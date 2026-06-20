//! Pipeline orchestrator - coordinates capture → OCR → matcher → audio flow.
//!
//! This module implements the async pipeline architecture using tokio tasks and channels.
//! The pipeline runs as separate workers connected by bounded channels with drop-oldest semantics.
//!
//! Architecture:
//! - Capture Worker: Grabs frames at configured interval, applies frame differencing
//! - OCR Worker: Preprocesses frames and runs OCR via FFI (in spawn_blocking)
//! - Matcher Worker: Matches OCR results against text file, manages deduplication
//! - Audio Worker: Plays audio files with queue management and dynamic speed
//!
//! All workers can be started/stopped gracefully via stop signal channels.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{info, debug, warn, error};

use crate::state::AppState;
use crate::capture::{Frame, CaptureRegion, CaptureSource};
use crate::capture_dxgi::DxgiCapture;
use crate::capture_wgc::WgcCapture;
use crate::frame_diff::FrameDiffer;
use crate::validation::Validator;
use crate::audio::AudioPlayer;
use crate::region::RegionManager;
use crate::preprocessing::Preprocessor;
use crate::text_grouping::{group_into_lines, cluster_into_paragraphs};
use crate::text_filters::{CenterLineFilter, CharacterNameFilter};
use crate::matcher::{FuzzyMatcher, MatchConfig, MatchState, Matcher};
use crate::dedup::{DedupState, is_garbage_text};

/// Message types flowing through the pipeline

/// Frame to be processed by OCR
#[derive(Debug, Clone)]
pub struct CaptureMessage {
    pub frame: Frame,
    pub region: CaptureRegion,
    /// When this frame was captured (for end-to-end latency tracking)
    pub captured_at: std::time::Instant,
}

/// OCR result to be matched
#[derive(Debug, Clone)]
pub struct OcrMessage {
    pub texts: Vec<String>,  // Paragraph texts or single text
    pub is_paragraph_mode: bool,
    /// Timestamp from the originating capture (propagated for latency tracking)
    pub captured_at: std::time::Instant,
}

/// Match result to play audio
#[derive(Debug, Clone)]
pub struct MatchMessage {
    pub line_index: usize,  // 1-based line index
    pub speed: f32,
    /// Timestamp from the originating capture (propagated for latency tracking)
    pub captured_at: std::time::Instant,
}

/// Saves the preprocessed (grayscale/enhanced) image that is fed to OCR as a PNG.
///
/// This is the exact black/white image the OCR engine receives, useful for
/// diagnosing recognition quality. Files are timestamped in `dir`.
fn save_ocr_debug_image(
    img: &crate::ocr::ImageBuffer,
    dir: &str,
    lines: crate::region_overlay::CenterLines,
) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    // Throttle: saving a PNG every OCR cycle hammers CPU/disk and starves the
    // audio worker. Cap to a few per second.
    static LAST_SHOT: Mutex<Option<Instant>> = Mutex::new(None);
    {
        let mut last = match LAST_SHOT.lock() {
            Ok(l) => l,
            Err(_) => return,
        };
        let now = Instant::now();
        if let Some(t) = *last {
            if now.duration_since(t) < Duration::from_millis(400) {
                return;
            }
        }
        *last = Some(now);
    }

    // img.data is BGR (3 bytes/pixel); the image crate wants RGB.
    let mut rgb = Vec::with_capacity(img.data.len());
    for px in img.data.chunks_exact(3) {
        rgb.push(px[2]); // R
        rgb.push(px[1]); // G
        rgb.push(px[0]); // B
    }
    let (w, h) = (img.width, img.height);

    // Draw the active center lines (blue vertical bands) so they're visible on
    // the screenshot, matching what the overlay shows.
    if lines.any() && w > 0 && h > 0 {
        let wi = w as i32;
        let mut bands: Vec<(i32, i32)> = Vec::new();
        if lines.l1 {
            let cx = wi / 2;
            bands.push((cx - lines.margin / 2, cx + lines.margin / 2));
        }
        if lines.l2 {
            bands.push((lines.l2_start, lines.l2_start + lines.margin));
        }
        if lines.l3 {
            let x = (wi as f32 * lines.l3_ratio) as i32;
            bands.push((x, x + lines.margin));
        }
        let mut vline = |x: i32| {
            if x < 0 || x >= wi {
                return;
            }
            for y in 0..h {
                let idx = ((y * w + x as u32) as usize) * 3;
                rgb[idx] = 0; // R
                rgb[idx + 1] = 128; // G
                rgb[idx + 2] = 255; // B (blue)
            }
        };
        for (x0, x1) in bands {
            vline(x0);
            vline(x1);
        }
    }

    let dir = dir.to_string();

    // Encode + write off the OCR thread so it never blocks recognition/audio.
    std::thread::spawn(move || match image::RgbImage::from_raw(w, h, rgb) {
        Some(buf) => {
            let ts = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
            let path = std::path::Path::new(&dir).join(format!("ocr_{}.png", ts));
            if let Err(e) = buf.save(&path) {
                warn!("Failed to save OCR debug screenshot to {:?}: {}", path, e);
            } else {
                debug!("Saved OCR debug screenshot: {:?}", path);
            }
        }
        None => warn!("Failed to build OCR debug image (size mismatch)"),
    });
}

/// Capture backend selected at runtime based on config `capture_mode`.
///
/// - `Gdi`: fast GDI BitBlt region capture (borderless/windowed)
/// - `Wgc`: Windows Graphics Capture of a specific window (works in fullscreen)
enum CaptureBackend {
    Gdi(DxgiCapture),
    Wgc(WgcCapture),
}

impl CaptureBackend {
    fn grab(&mut self, region: &CaptureRegion) -> Result<Frame, crate::capture::CaptureError> {
        match self {
            CaptureBackend::Gdi(c) => c.grab(region),
            CaptureBackend::Wgc(c) => c.grab(region),
        }
    }
}

/// Stacks two frames vertically (top over bottom) with a black gap between
/// them, into one tightly-packed BGRA frame. This lets a SINGLE OCR pass cover
/// both capture regions: detection cost is ~constant (the model scales to a
/// fixed side length), so reading two regions this way costs roughly the same
/// as one. The large gap guarantees the paragraph clusterer treats the two
/// regions as separate dialogues.
fn stack_frames(top: &Frame, bottom: &Frame, gap: u32) -> Frame {
    let w = top.width.max(bottom.width).max(1);
    let h = top.height + gap + bottom.height;
    let stride = (w as usize) * 4;
    let mut bgra = vec![0u8; stride * h as usize]; // black background

    for y in 0..top.height as usize {
        let src = y * top.stride;
        let dst = y * stride;
        let row = (top.width as usize) * 4;
        bgra[dst..dst + row].copy_from_slice(&top.bgra[src..src + row]);
    }
    let y_off = (top.height + gap) as usize;
    for y in 0..bottom.height as usize {
        let src = y * bottom.stride;
        let dst = (y_off + y) * stride;
        let row = (bottom.width as usize) * 4;
        bgra[dst..dst + row].copy_from_slice(&bottom.bgra[src..src + row]);
    }

    Frame::from_data(w, h, stride, bgra).unwrap_or_else(|_| top.clone())
}

/// Parses a resolution string like "1920x1080" into (width, height).
/// Falls back to 1920x1080 on parse failure.
fn parse_resolution(res: &str) -> (u32, u32) {
    let parts: Vec<&str> = res.split(['x', 'X']).collect();
    if parts.len() == 2 {
        if let (Ok(w), Ok(h)) = (parts[0].trim().parse::<u32>(), parts[1].trim().parse::<u32>()) {
            if w > 0 && h > 0 {
                return (w, h);
            }
        }
    }
    (1920, 1080)
}

/// Pipeline orchestrator manages all workers
pub struct Pipeline {
    state: Arc<AppState>,
    region_manager: Arc<parking_lot::Mutex<RegionManager>>,
    
    // Audio player configuration (actual player created in worker thread).
    // NOTE: kept for the constructor signature, but the audio worker now reads
    // these from the live config snapshot at start time (see spawn_audio_worker),
    // so these stored copies are not read directly.
    #[allow(dead_code)]
    audio_queue_capacity: usize,
    #[allow(dead_code)]
    enable_dynamic_speed: bool,
    #[allow(dead_code)]
    base_playback_speed: f32,
    #[allow(dead_code)]
    overlap_playback_speed: f32,
    #[allow(dead_code)]
    volume_reduction_level: f32,
    
    // Worker task handles
    capture_handle: Option<JoinHandle<()>>,
    ocr_handle: Option<JoinHandle<()>>,
    matcher_handle: Option<JoinHandle<()>>,
    audio_thread_handle: Option<std::thread::JoinHandle<()>>,
    
    // Stop signal broadcaster
    stop_tx: Option<broadcast::Sender<()>>,
    
    // Audio command channel
    audio_cmd_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::audio::AudioCommand>>,
}

impl Pipeline {
    /// Creates a new pipeline (not started)
    pub fn new(
        state: Arc<AppState>,
        region_manager: Arc<parking_lot::Mutex<RegionManager>>,
        audio_queue_capacity: usize,
        enable_dynamic_speed: bool,
        base_playback_speed: f32,
        overlap_playback_speed: f32,
        volume_reduction_level: f32,
    ) -> Self {
        Self {
            state,
            region_manager,
            audio_queue_capacity,
            enable_dynamic_speed,
            base_playback_speed,
            overlap_playback_speed,
            volume_reduction_level,
            capture_handle: None,
            ocr_handle: None,
            matcher_handle: None,
            audio_thread_handle: None,
            stop_tx: None,
            audio_cmd_tx: None,
        }
    }
    
    /// Starts the pipeline after validation
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_running() {
            return Err("Pipeline is already running".to_string());
        }
        
        // Validate configuration before starting
        let config = self.state.get_config_snapshot();
        info!("Validating config before start: audio_dir='{}', text_file_path='{}'", config.audio_dir, config.text_file_path);
        let validation_result = Validator::validate_before_reader_start(&config);
        
        if !validation_result.is_valid() {
            let error_msg = validation_result.first_error_message()
                .unwrap_or_else(|| "Validation failed".to_string());
            error!("Validation failed: {}", error_msg);
            crate::logging::user_log(format!("❌ Nie można uruchomić lektora: {}", error_msg));
            return Err(error_msg);
        }
        
        info!("Starting pipeline...");
        crate::logging::user_log("▶️ Uruchamiam lektora…");
        
        // Create stop signal channel
        let (stop_tx, _) = broadcast::channel(16);
        self.stop_tx = Some(stop_tx.clone());
        
        // Create pipeline channels with bounded capacity
        // capture->OCR uses a small buffer; the OCR worker drains it to the freshest
        // frame each cycle so we never process stale frames (low latency).
        let (capture_tx, capture_rx) = mpsc::channel::<CaptureMessage>(4);
        let (ocr_tx, ocr_rx) = mpsc::channel::<OcrMessage>(1);
        let audio_queue_size = config.audio_queue_size as usize;
        let (match_tx, match_rx) = mpsc::channel::<MatchMessage>(audio_queue_size);
        
        // Spawn workers
        self.capture_handle = Some(self.spawn_capture_worker(
            stop_tx.subscribe(),
            capture_tx,
        ));
        
        self.ocr_handle = Some(self.spawn_ocr_worker(
            stop_tx.subscribe(),
            capture_rx,
            ocr_tx,
        ));
        
        self.matcher_handle = Some(self.spawn_matcher_worker(
            stop_tx.subscribe(),
            ocr_rx,
            match_tx,
        ));
        
        // Create audio command channel
        let (audio_cmd_tx, audio_cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        self.audio_cmd_tx = Some(audio_cmd_tx);
        
        // Audio worker runs in a dedicated thread (rodio::OutputStream is not Send)
        let audio_handle = self.spawn_audio_worker(stop_tx.subscribe(), match_rx, audio_cmd_rx);
        self.audio_thread_handle = Some(audio_handle);
        
        // Update runtime state
        self.state.update_runtime(|runtime| {
            runtime.capture_enabled = true;
        });
        
        info!("Pipeline started successfully");
        crate::logging::user_log("✅ Lektor działa – obserwuję ekran gry");
        crate::system_sounds::play("on");
        Ok(())
    }
    
    /// Stops the pipeline gracefully
    pub async fn stop(&mut self) {
        if !self.is_running() {
            return;
        }
        
        info!("Stopping pipeline...");
        
        // Send stop signal to all workers
        if let Some(stop_tx) = &self.stop_tx {
            let _ = stop_tx.send(());
        }
        
        // Wait for async workers to finish
        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.ocr_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.matcher_handle.take() {
            let _ = handle.await;
        }
        
        // Wait for audio thread to finish (non-async)
        if let Some(handle) = self.audio_thread_handle.take() {
            let _ = handle.join();
        }
        
        // Update runtime state
        self.state.update_runtime(|runtime| {
            runtime.capture_enabled = false;
        });
        
        info!("Pipeline stopped");
        crate::logging::user_log("⏹️ Lektor zatrzymany");
        crate::system_sounds::play("off");
    }
    
    /// Sends a command to the audio player.
    ///
    /// Returns Ok(()) if command was sent successfully, or Err if pipeline is not running.
    pub fn send_audio_command(&self, command: crate::audio::AudioCommand) -> Result<(), String> {
        if let Some(tx) = &self.audio_cmd_tx {
            tx.send(command)
                .map_err(|e| format!("Failed to send audio command: {}", e))
        } else {
            Err("Pipeline not running".to_string())
        }
    }
    
    /// Checks if pipeline is currently running
    pub fn is_running(&self) -> bool {
        self.capture_handle.is_some()
    }
    
    // Worker spawning methods
    
    fn spawn_capture_worker(
        &self,
        mut stop_rx: broadcast::Receiver<()>,
        tx: mpsc::Sender<CaptureMessage>,
    ) -> JoinHandle<()> {
        let state = Arc::clone(&self.state);
        let region_manager = Arc::clone(&self.region_manager);
        
        tokio::spawn(async move {
            info!("Capture worker started");
            
            // Initialize frame differ (no downscale, always use full resolution)
            let config = state.get_config_snapshot();
            let mut frame_differ = FrameDiffer::new(1.0);
            
            // Initialize capture backend based on configured mode.
            // "window" -> WGC (captures a specific window, works in fullscreen)
            // "region" -> GDI BitBlt region capture (default)
            let mut capture_backend: Option<CaptureBackend> = if config.capture_mode == "window" {
                let mut wgc = WgcCapture::new();
                // Parse base resolution (e.g. "1920x1080") for windowed region scaling
                let (base_w, base_h) = parse_resolution(&config.resolution);
                match wgc.start_for_window(&config.capture_window_query, base_w, base_h) {
                    Ok(()) => {
                        info!("WGC window capture started for query '{}'", config.capture_window_query);
                        crate::logging::user_log(format!("🎯 Podłączono do okna gry: „{}”", config.capture_window_query));
                        Some(CaptureBackend::Wgc(wgc))
                    }
                    Err(e) => {
                        error!("Failed to start WGC window capture ('{}'): {}. Capture disabled.",
                            config.capture_window_query, e);
                        crate::logging::user_log(format!("❌ Nie udało się podłączyć do okna gry „{}”", config.capture_window_query));
                        None
                    }
                }
            } else {
                match DxgiCapture::new() {
                    Ok(mut backend) => {
                        match backend.bind_monitor(0) {
                            Ok(()) => {
                                info!("GDI region capture bound to monitor 0");
                                Some(CaptureBackend::Gdi(backend))
                            }
                            Err(e) => {
                                error!("Failed to bind monitor 0: {}. Capture disabled.", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to create GDI capture: {}. Capture disabled.", e);
                        None
                    }
                }
            };
            
            let mut last_capture_time = std::time::Instant::now();
            
            loop {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        info!("Capture worker stopping");
                        crate::capture_wgc::set_overlay_rects(Vec::new());
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Get current config and check interval
                        let config = state.get_config_snapshot();
                        let interval = Duration::from_secs_f32(config.capture_interval);
                        
                        if last_capture_time.elapsed() < interval {
                            continue; // Not time yet
                        }
                        last_capture_time = std::time::Instant::now();
                        
                        // Get active region from region manager
                        let region_mgr = region_manager.lock();
                        let active_rect = region_mgr.get_active_rect(&config);
                        drop(region_mgr);

                        let backend = match capture_backend.as_mut() {
                            Some(b) => b,
                            None => continue, // No capture backend available
                        };

                        // With a second region enabled, grab BOTH regions and stack
                        // them into one frame so a SINGLE OCR pass reads both
                        // (cheap: detection scales to a fixed size). Otherwise grab
                        // just the active region as before.
                        let (frame, region) = if config.monitor2_enabled {
                            let reg1 = CaptureRegion::new(
                                config.monitor.left as u32,
                                config.monitor.top as u32,
                                config.monitor.width,
                                config.monitor.height,
                            );
                            let reg2 = CaptureRegion::new(
                                config.monitor2_left as u32,
                                config.monitor2_top as u32,
                                config.monitor2_width,
                                config.monitor2_height,
                            );
                            let f1 = match backend.grab(&reg1) {
                                Ok(f) => f,
                                Err(crate::capture::CaptureError::DeviceLost) => {
                                    error!("Capture device lost");
                                    continue;
                                }
                                Err(e) => {
                                    debug!("Capture (region 1) failed: {}", e);
                                    continue;
                                }
                            };
                            // grab() published region 1's exact screen rect; capture
                            // it now before the region 2 grab overwrites it.
                            let r1 = crate::capture_wgc::last_ocr_rect();
                            match backend.grab(&reg2) {
                                Ok(f2) => {
                                    // Big gap = guaranteed separate dialogues.
                                    let gap = f1.height.max(f2.height).max(40);
                                    let stacked = stack_frames(&f1, &f2, gap);
                                    let sreg = CaptureRegion::new(0, 0, stacked.width, stacked.height);
                                    // Publish both regions' screen rects for the overlay.
                                    let r2 = crate::capture_wgc::last_ocr_rect();
                                    let mut rects = Vec::new();
                                    if let Some(r) = r1 { rects.push(r); }
                                    if let Some(r) = r2 { rects.push(r); }
                                    crate::capture_wgc::set_overlay_rects(rects);
                                    (stacked, sreg)
                                }
                                Err(e) => {
                                    debug!("Capture (region 2) failed: {} - using region 1 only", e);
                                    if let Some(r) = r1 {
                                        crate::capture_wgc::set_overlay_rects(vec![r]);
                                    }
                                    (f1, reg1)
                                }
                            }
                        } else {
                            let region = CaptureRegion::new(
                                active_rect.left as u32,
                                active_rect.top as u32,
                                active_rect.width,
                                active_rect.height,
                            );
                            match backend.grab(&region) {
                                Ok(f) => {
                                    if let Some(r) = crate::capture_wgc::last_ocr_rect() {
                                        crate::capture_wgc::set_overlay_rects(vec![r]);
                                    }
                                    (f, region)
                                }
                                Err(crate::capture::CaptureError::DeviceLost) => {
                                    error!("Capture device lost");
                                    continue;
                                }
                                Err(e) => {
                                    debug!("Capture failed: {}", e);
                                    continue;
                                }
                            }
                        };
                        
                        // Skip OCR when the captured area hasn't changed (static
                        // scene / unchanged subtitle) — the biggest CPU saver.
                        let diff_score = frame_differ.compute_difference(&frame, &region);
                        if diff_score < crate::constants::FRAME_DIFFERENCE_THRESHOLD {
                            continue;
                        }

                        let captured_at = std::time::Instant::now();
                        info!("[T0] Captured region ({},{},{}x{}) diff={:.2} -> sending to OCR",
                            region.left, region.top, region.width, region.height, diff_score);
                        
                        // Send frame to OCR worker (drop if channel full - drop-oldest)
                        let msg = CaptureMessage { frame, region, captured_at };
                        if tx.try_send(msg).is_err() {
                            debug!("OCR channel full, dropping oldest frame");
                        }
                    }
                }
            }
            
            info!("Capture worker stopped");
        })
    }
    
    fn spawn_ocr_worker(
        &self,
        mut stop_rx: broadcast::Receiver<()>,
        mut rx: mpsc::Receiver<CaptureMessage>,
        tx: mpsc::Sender<OcrMessage>,
    ) -> JoinHandle<()> {
        let state = Arc::clone(&self.state);
        
        tokio::spawn(async move {
            info!("OCR worker started");
            
            // Initialize OCR engine wrapped in Arc<Mutex<>> for sharing across spawn_blocking calls
            let ocr_engine = Arc::new(parking_lot::Mutex::new(crate::ocr_engine::create_ocr_engine()));
            
            // Initialize the OCR engine with model files from the models directory.
            // Tries common PP-OCRv5 filenames, then generic names.
            {
                let models_dir = std::path::PathBuf::from("models");
                // Prefer FP16 models: ~9% faster inference, ~8% less memory, half the
                // size, with no accuracy loss. Fall back to the standard models if the
                // FP16 variants aren't present.
                let det_candidates = ["PP-OCRv5_mobile_det_fp16.mnn", "PP-OCRv5_mobile_det.mnn", "det.mnn"];
                let rec_candidates = ["PP-OCRv5_mobile_rec_fp16.mnn", "PP-OCRv5_mobile_rec.mnn", "rec.mnn"];
                let keys_candidates = ["ppocr_keys_v5.txt", "keys.txt"];
                
                let find = |candidates: &[&str]| -> Option<std::path::PathBuf> {
                    candidates.iter()
                        .map(|name| models_dir.join(name))
                        .find(|p| p.exists())
                };
                
                match (find(&det_candidates), find(&rec_candidates), find(&keys_candidates)) {
                    (Some(det), Some(rec), Some(keys)) => {
                        let mut engine = ocr_engine.lock();
                        match engine.init(&det, &rec, &keys) {
                            Ok(()) => info!("OCR engine initialized with models from {}", models_dir.display()),
                            Err(e) => error!("Failed to initialize OCR engine: {}. OCR will return no text.", e),
                        }
                    }
                    _ => {
                        error!("OCR models not found in '{}'. Place PP-OCRv5 det/rec .mnn and keys file there. OCR will return no text.", models_dir.display());
                    }
                }
            }
            
            loop {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        info!("OCR worker stopping");
                        break;
                    }
                    msg = rx.recv() => {
                        if let Some(mut capture_msg) = msg {
                            // Drain the channel to the FRESHEST frame to avoid processing
                            // stale frames (low latency). Older queued frames are discarded.
                            let mut dropped = 0;
                            while let Ok(newer) = rx.try_recv() {
                                capture_msg = newer;
                                dropped += 1;
                            }
                            if dropped > 0 {
                                debug!("Dropped {} stale frame(s), processing freshest", dropped);
                            }
                            
                            // Get current config (may have changed)
                            let mut config = state.get_config_snapshot();
                            // With two regions stacked into one frame, force paragraph
                            // clustering so the two regions become separate dialogues.
                            if config.monitor2_enabled {
                                config.enable_paragraph_ocr = true;
                            }
                            
                            let captured_at = capture_msg.captured_at;
                            // Latency: how long this frame waited before OCR started processing it
                            let queue_wait = captured_at.elapsed();
                            let ocr_start = std::time::Instant::now();
                            
                            // Create preprocessor (automatic CLAHE colour-contrast
                            // enhancement; no downscaling).
                            let preprocessor = Preprocessor::new();
                            
                            // Preprocess frame for OCR
                            let img_buffer = preprocessor.for_ocr(&capture_msg.frame);
                            
                            // Optionally save the exact preprocessed image fed to OCR
                            // (debug aid) when screenshots are enabled.
                            if config.enable_screenshots && !config.screenshot_dir.is_empty() {
                                let lines = crate::region_overlay::CenterLines {
                                    l1: config.use_center_line_1,
                                    l2: config.use_center_line_2,
                                    l3: config.use_center_line_3,
                                    margin: config.center_line_margin,
                                    l2_start: config.center_line_2_start,
                                    l3_ratio: config.center_line_3_start_ratio,
                                };
                                save_ocr_debug_image(&img_buffer, &config.screenshot_dir, lines);
                            }
                            
                            // Run OCR in spawn_blocking (CPU-intensive, blocking operation)
                            let ocr_engine_clone = Arc::clone(&ocr_engine);
                            let ocr_result = tokio::task::spawn_blocking(move || {
                                let mut engine = ocr_engine_clone.lock();
                                engine.run(&img_buffer)
                            }).await;
                            
                            let boxes = match ocr_result {
                                Ok(Ok(boxes)) => boxes,
                                Ok(Err(e)) => {
                                    debug!("OCR failed: {}", e);
                                    continue;
                                }
                                Err(e) => {
                                    debug!("OCR task panicked: {}", e);
                                    continue;
                                }
                            };
                            
                            if boxes.is_empty() {
                                // Send empty result
                                let msg = OcrMessage {
                                    texts: Vec::new(),
                                    is_paragraph_mode: config.enable_paragraph_ocr,
                                    captured_at,
                                };
                                let _ = tx.send(msg).await;
                                continue;
                            }
                            
                            // Group boxes into lines
                            let boxes_count = boxes.len();
                            let mut lines = group_into_lines(
                                boxes,
                                &config,
                                config.resolution_downscale
                            );
                            
                            // Apply center line filters
                            if config.use_center_line_1 || config.use_center_line_2 || config.use_center_line_3 {
                                lines = CenterLineFilter::filter_lines(
                                    lines,
                                    &config,
                                    capture_msg.frame.width,
                                    capture_msg.frame.height,
                                );
                            }
                            
                            // Cluster into paragraphs
                            let texts = cluster_into_paragraphs(
                                lines,
                                &config,
                                config.resolution_downscale
                            );
                            
                            // Diagnostic: log what OCR actually read + timing
                            let ocr_ms = ocr_start.elapsed().as_millis();
                            info!("[T1] OCR read {} box(es) in {}ms (waited {}ms in queue) -> texts: {:?}",
                                boxes_count, ocr_ms, queue_wait.as_millis(), texts);

                            if texts.is_empty() {
                                crate::logging::user_log("🔍 OCR: nie wykryto tekstu w obszarze");
                            } else {
                                let preview = texts.join(" ").replace('\n', " ");
                                let preview = if preview.chars().count() > 80 {
                                    let short: String = preview.chars().take(80).collect();
                                    format!("{}…", short)
                                } else {
                                    preview
                                };
                                crate::logging::user_log(format!("🔍 OCR odczytał tekst: „{}”", preview));
                            }
                            
                            // Send OCR result to matcher
                            let msg = OcrMessage {
                                texts,
                                is_paragraph_mode: config.enable_paragraph_ocr,
                                captured_at,
                            };
                            
                            if tx.send(msg).await.is_err() {
                                debug!("Failed to send OCR result - matcher channel closed");
                                break;
                            }
                        } else {
                            // Channel closed
                            break;
                        }
                    }
                }
            }
            
            info!("OCR worker stopped");
        })
    }
    
    fn spawn_matcher_worker(
        &self,
        mut stop_rx: broadcast::Receiver<()>,
        mut rx: mpsc::Receiver<OcrMessage>,
        tx: mpsc::Sender<MatchMessage>,
    ) -> JoinHandle<()> {
        let state = Arc::clone(&self.state);
        
        tokio::spawn(async move {
            info!("Matcher worker started");
            
            // Initialize matcher components
            let config = state.get_config_snapshot();
            let match_config = MatchConfig::default();
            let matcher = FuzzyMatcher::new(match_config);
            let mut match_state = MatchState::new();
            let mut dedup_state = DedupState::new();
            
            // Load dialog lines from text file
            let dialog_lines = match std::fs::read_to_string(&config.text_file_path) {
                Ok(content) => {
                    let lines: Vec<String> = content.lines()
                        .map(|l| l.to_string())
                        .collect();
                    info!("Loaded {} dialogue lines from {}", lines.len(), config.text_file_path);
                    crate::logging::user_log(format!("📖 Wczytano bazę dialogów: {} linii", lines.len()));
                    lines
                }
                Err(e) => {
                    error!("Failed to load dialogue lines from {}: {}", config.text_file_path, e);
                    crate::logging::user_log(format!("❌ Nie udało się wczytać pliku z dialogami: {}", config.text_file_path));
                    Vec::new()
                }
            };
            
            // Load character names if enabled
            let character_filter = if config.enable_remove_character_name {
                let names = if !config.names_file_path.is_empty() {
                    match std::fs::read_to_string(&config.names_file_path) {
                        Ok(content) => {
                            let names: Vec<String> = content.lines()
                                .map(|l| l.trim().to_string())
                                .filter(|l| !l.is_empty())
                                .collect();
                            info!("Loaded {} character names", names.len());
                            crate::logging::user_log(format!("👤 Wczytano {} imion postaci", names.len()));
                            names
                        }
                        Err(e) => {
                            debug!("Failed to load character names from {}: {}", config.names_file_path, e);
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                };
                Some(CharacterNameFilter::new(names))
            } else {
                None
            };
            
            if dialog_lines.is_empty() {
                error!("No dialogue lines loaded - matcher worker cannot function");
                crate::logging::user_log("❌ Brak dialogów do dopasowania – sprawdź plik tekstowy");
                return;
            }
            
            loop {
                tokio::select! {
                    _ = stop_rx.recv() => {
                        info!("Matcher worker stopping");
                        break;
                    }
                    msg = rx.recv() => {
                        if let Some(ocr_msg) = msg {
                            // Get current config (may have changed)
                            let config = state.get_config_snapshot();
                            
                            let captured_at = ocr_msg.captured_at;
                            let match_start = std::time::Instant::now();
                            
                            // Handle empty OCR results
                            if ocr_msg.texts.is_empty() {
                                dedup_state.increment_empty_reads();
                                
                                // Clear typewriter cache in typewriter mode
                                if config.enable_typewriter_wait {
                                    dedup_state.clear_typewriter_cache();
                                }
                                
                                // Check for idle mode (using constants)
                                let empty_reads_to_idle = crate::constants::EMPTY_READS_TO_IDLE as u32;
                                if dedup_state.empty_reads() >= empty_reads_to_idle {
                                    debug!("Entering idle mode - sleeping");
                                    tokio::time::sleep(Duration::from_secs_f32(
                                        crate::constants::IDLE_SLEEP_SECONDS
                                    )).await;
                                }
                                continue;
                            }
                            
                            // Reset empty reads counter
                            dedup_state.reset_empty_reads();
                            
                            // Create text fingerprint for stable frame detection
                            let fingerprint = ocr_msg.texts.join("\n\n");
                            let frame_stable = dedup_state.update_stable_detection(
                                &fingerprint,
                                crate::constants::TYPEWRITER_STABLE_READS as u32
                            );
                            
                            // Each entry in `ocr_msg.texts` is ONE dialogue:
                            //  - paragraph mode OFF: a single entry (all lines).
                            //  - paragraph mode ON: one entry per vertically-separated
                            //    block (handled by cluster_into_paragraphs).
                            // Within an entry, '\n' is just word-wrap, so we always
                            // join it into one sentence and match the whole thing.
                            // The character-name filter runs on the full multi-line
                            // text first so a name on its own line is stripped.
                            let mut line_items: Vec<(usize, String)> = Vec::new();
                            for (para_idx, text) in ocr_msg.texts.iter().enumerate() {
                                let cleaned = match character_filter {
                                    Some(ref filter) => filter.remove_name(text),
                                    None => text.clone(),
                                };
                                let joined = cleaned
                                    .split('\n')
                                    .map(|l| l.trim())
                                    .filter(|l| !l.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                if !joined.is_empty() {
                                    line_items.push((para_idx, joined));
                                }
                            }

                            for (para_idx, raw_line) in line_items {
                                let processed_text = raw_line;

                                // (Character name already stripped from the full text above.)

                                // Garbage text filtering
                                if is_garbage_text(&processed_text, 0.30, 3) {
                                    debug!("Rejected garbage text: '{}'", processed_text);
                                    continue;
                                }
                                
                                // Minimum length check
                                if processed_text.len() < 3 {
                                    continue;
                                }
                                
                                // Check typewriter deduplication (per-paragraph)
                                if config.enable_typewriter_wait && ocr_msg.is_paragraph_mode {
                                    // Note: is_typewriter_duplicate checks if THIS line_idx was matched for THIS paragraph
                                    // We don't know line_idx yet, so we skip this check here
                                    // The actual check happens after matching
                                }
                                
                                // Find best match
                                let match_result = matcher.find_best_match(
                                    &processed_text,
                                    &dialog_lines,
                                    &match_state,
                                    config.enable_typewriter_wait,
                                    frame_stable,
                                );
                                
                                let Some(result) = match_result else {
                                    debug!("No match found for text: '{}'", processed_text);
                                    crate::logging::user_log(format!("🤔 Brak pasującego dialogu dla: „{}”", processed_text));
                                    continue;
                                };
                                
                                // Check if this line was already matched in this paragraph (typewriter dedup)
                                if config.enable_typewriter_wait && ocr_msg.is_paragraph_mode {
                                    if dedup_state.is_typewriter_duplicate(para_idx, result.index) {
                                        debug!("Skipping typewriter duplicate: paragraph {} already matched line {}", 
                                            para_idx, result.index);
                                        continue;
                                    }
                                }
                                
                                // Check index deduplication
                                if dedup_state.is_duplicate_index(result.index) {
                                    debug!("Skipping duplicate index: {}", result.index);
                                    continue;
                                }
                                
                                // Check text similarity deduplication.
                                // Match Python: compare the matched LINE text against
                                // the previously matched LINE text (both clean), not the
                                // raw OCR text (which has recognition errors).
                                let matched_line_text = &dialog_lines[result.index - 1];
                                if dedup_state.is_duplicate_text(matched_line_text, 92) {
                                    debug!("Skipping similar line content (>= 92% match)");
                                    continue;
                                }
                                
                                // Record match in deduplication state
                                dedup_state.record_match(result.index, dialog_lines[result.index - 1].clone());
                                
                                // Update matcher state
                                match_state.update(result.index);
                                
                                // Record typewriter match if in paragraph mode
                                if config.enable_typewriter_wait && ocr_msg.is_paragraph_mode {
                                    dedup_state.record_typewriter_match(para_idx, result.index);
                                }
                                
                                // Calculate audio speed (placeholder - dynamic speed logic)
                                let speed = if config.enable_dynamic_speed {
                                    // TODO: Implement dynamic speed calculation based on queue state
                                    // For now, use overlap speed as placeholder
                                    config.overlap_playback_speed
                                } else {
                                    1.0
                                };
                                
                                info!("[T2] Matched line {} (score: {}) in {}ms | TOTAL capture->match: {}ms | text: '{}'", 
                                    result.index, result.score,
                                    match_start.elapsed().as_millis(),
                                    captured_at.elapsed().as_millis(),
                                    processed_text);
                                crate::logging::user_log(format!(
                                    "✅ Dopasowano dialog #{} (zgodność {}%): „{}”",
                                    result.index, result.score, dialog_lines[result.index - 1]
                                ));
                                
                                // Send match to audio worker
                                let match_msg = MatchMessage {
                                    line_index: result.index,
                                    speed,
                                    captured_at,
                                };
                                
                                if tx.send(match_msg).await.is_err() {
                                    debug!("Failed to send match - audio channel closed");
                                    break;
                                }
                            }
                        } else {
                            // Channel closed
                            break;
                        }
                    }
                }
            }
            
            info!("Matcher worker stopped");
        })
    }
    
    fn spawn_audio_worker(
        &self,
        mut stop_rx: broadcast::Receiver<()>,
        mut rx: mpsc::Receiver<MatchMessage>,
        mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<crate::audio::AudioCommand>,
    ) -> std::thread::JoinHandle<()> {
        let state = Arc::clone(&self.state);

        // Read audio settings from the CURRENT config snapshot (not the stale
        // values captured when Pipeline was constructed at startup). This is
        // what makes the playback-speed slider actually take effect on start.
        let config = state.get_config_snapshot();
        let audio_queue_capacity = config.audio_queue_size as usize;
        let enable_dynamic_speed = config.enable_dynamic_speed;
        let base_playback_speed = config.base_playback_speed;
        let overlap_playback_speed = config.overlap_playback_speed;
        let volume_reduction_level = config.volume_reduction_level;

        // Get ducking target from config (use capture_window_query if ducking_target_process is empty)
        let ducking_target = if !config.ducking_target_process.is_empty() {
            Some(config.ducking_target_process.clone())
        } else if !config.capture_window_query.is_empty() {
            Some(config.capture_window_query.clone())
        } else {
            None
        };
        
        // Audio worker runs in a dedicated thread because rodio's OutputStream is not Send
        std::thread::spawn(move || {
            info!("Audio worker started");

            // Audio decoding + time-stretch must keep up even while OCR
            // saturates the CPU, so give this thread a high priority.
            crate::process_priority::set_current_thread_high();
            
            // Create audio player in this thread
            let mut audio_player = match AudioPlayer::new(
                audio_queue_capacity,
                enable_dynamic_speed,
                base_playback_speed,
                overlap_playback_speed,
                volume_reduction_level,
                ducking_target,
            ) {
                Ok(player) => player,
                Err(e) => {
                    error!("Failed to create audio player: {}", e);
                    return;
                }
            };
            
            // Create a blocking runtime for receiving messages
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create audio worker runtime");
            
            rt.block_on(async move {
                // Playback update interval (check if current audio finished)
                let mut update_interval = tokio::time::interval(Duration::from_millis(50));
                
                loop {
                    tokio::select! {
                        _ = stop_rx.recv() => {
                            info!("Audio worker stopping");
                            break;
                        }
                        Some(cmd) = cmd_rx.recv() => {
                            // Handle audio commands
                            match cmd {
                                crate::audio::AudioCommand::SkipToNextWithBoost => {
                                    debug!("Processing skip to next with boost command");
                                    if let Err(e) = audio_player.skip_to_next_with_speed_boost() {
                                        warn!("Failed to skip to next: {}", e);
                                    }
                                }
                                crate::audio::AudioCommand::StopAll => {
                                    debug!("Processing stop all command");
                                    audio_player.stop();
                                }
                            }
                        }
                        msg = rx.recv() => {
                            if let Some(match_msg) = msg {
                                let config = state.get_config_snapshot();
                                
                                // End-to-end latency: from screen capture to audio dispatch
                                info!("[T3] Audio dispatch for line {} | TOTAL capture->audio: {}ms",
                                    match_msg.line_index, match_msg.captured_at.elapsed().as_millis());
                                
                                // Find audio file for line index
                                let audio_path = crate::audio::find_audio_file(
                                    std::path::Path::new(&config.audio_dir),
                                    match_msg.line_index,
                                    config.enable_output2_system,
                                    config.enable_dynamic_speed,
                                );
                                
                                if let Some(path) = audio_path {
                                    debug!("Enqueueing audio for line {} at speed {} from {:?}",
                                        match_msg.line_index, match_msg.speed, path);

                                    // Apply the latest speed sliders live, so changes take
                                    // effect on the next line without restarting the reader.
                                    audio_player.set_speeds(
                                        config.base_playback_speed,
                                        config.overlap_playback_speed,
                                        config.enable_dynamic_speed,
                                    );
                                    // Apply the latest audio queue size live too.
                                    audio_player.set_queue_capacity(config.audio_queue_size as usize);
                                    // Apply the latest game-ducking level live.
                                    audio_player.set_volume_reduction(config.volume_reduction_level);
                                    // Apply the latest reader (TTS) volume live.
                                    audio_player.set_reader_volume(config.reader_volume);

                                    // Capture busy state BEFORE enqueue (enqueue makes queue non-empty)
                                    let was_busy = audio_player.is_busy();
                                    
                                    // Enqueue line with auto-decided speed:
                                    // - If busy (something playing) -> overlap speed (faster)
                                    // - If idle -> base speed (normal)
                                    // The speed is locked in NOW, so even after the queue drains
                                    // the line will still play sped-up when its turn comes.
                                    audio_player.enqueue_line_auto_speed(path);
                                    
                                    // Only start playing if nothing was playing before.
                                    // Otherwise current audio plays to completion, then update()
                                    // picks up this queued (sped-up) line.
                                    if !was_busy {
                                        let _ = audio_player.play_next();
                                    }
                                } else {
                                    debug!("No audio file found for line {}", match_msg.line_index);
                                    crate::logging::user_log(format!(
                                        "⚠️ Brak pliku audio dla dialogu #{} – pomijam",
                                        match_msg.line_index
                                    ));
                                }
                            } else {
                                // Channel closed
                                break;
                            }
                        }
                        _ = update_interval.tick() => {
                            // Periodically check if current audio finished and start next
                            let _ = audio_player.update();
                        }
                    }
                }
            });
            
            info!("Audio worker stopped");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    
    #[test]
    fn test_pipeline_creation() {
        let state = Arc::new(AppState::new());
        let region_manager = Arc::new(parking_lot::Mutex::new(RegionManager::new(false)));
        
        let pipeline = Pipeline::new(state, region_manager, 1, false, 1.0, 1.2, 0.2);
        assert!(!pipeline.is_running());
    }
    
    #[tokio::test]
    async fn test_pipeline_start_stop() {
        let state = Arc::new(AppState::with_config(AppConfig::test_valid()));
        let region_manager = Arc::new(parking_lot::Mutex::new(RegionManager::new(false)));
        
        let mut pipeline = Pipeline::new(state, region_manager, 1, false, 1.0, 1.2, 0.2);
        
        // Start pipeline
        pipeline.start().expect("Failed to start pipeline");
        assert!(pipeline.is_running());
        
        // Stop pipeline
        pipeline.stop().await;
        assert!(!pipeline.is_running());
    }
    
    #[tokio::test]
    async fn test_pipeline_validation_error() {
        let state = Arc::new(AppState::new());  // Default config may have validation issues
        let region_manager = Arc::new(parking_lot::Mutex::new(RegionManager::new(false)));
        
        let mut config = AppConfig::default();
        config.resolution_downscale = 0.0;  // Invalid value
        state.replace_config(config);
        
        let mut pipeline = Pipeline::new(state, region_manager, 1, false, 1.0, 1.2, 0.2);
        
        // Start should fail validation
        let result = pipeline.start();
        assert!(result.is_err());
        assert!(!pipeline.is_running());
    }
    
    #[tokio::test]
    async fn test_pipeline_double_start() {
        let state = Arc::new(AppState::with_config(AppConfig::test_valid()));
        let region_manager = Arc::new(parking_lot::Mutex::new(RegionManager::new(false)));
        
        let mut pipeline = Pipeline::new(state, region_manager, 1, false, 1.0, 1.2, 0.2);
        
        pipeline.start().expect("First start failed");
        let result = pipeline.start();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Pipeline is already running");
        
        pipeline.stop().await;
    }
}
