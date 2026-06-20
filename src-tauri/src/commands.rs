     //! Tauri command handlers for IPC communication.
//!
//! This module provides the command handlers that are invoked from the frontend
//! via Tauri's IPC mechanism.

use std::sync::Arc;
use tauri::{State, Emitter, Manager};
use parking_lot::Mutex;
use tokio::sync::Mutex as TokioMutex;
use serde::Serialize;

use crate::state::AppState;
use crate::pipeline::Pipeline;
use crate::region::RegionManager;
use crate::preset::PresetManager;
use crate::config::AppConfig;
use crate::ocr::OcrEngine;

/// Event payloads for frontend communication

#[derive(Debug, Clone, Serialize)]
struct ReaderStateEvent {
    enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ValidationErrorEvent {
    error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PresetLoadedEvent {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
struct DebugLogEvent {
    level: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct NoticeEvent {
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct ConfigChangedEvent {
    volume_reduction_level: f64,
    reader_volume: f64,
    base_playback_speed: f64,
    overlap_playback_speed: f64,
}

/// Helper function to emit reader_state event
fn emit_reader_state(app_handle: &tauri::AppHandle, enabled: bool) {
    let payload = ReaderStateEvent { enabled };
    if let Err(e) = app_handle.emit("reader_state", payload) {
        tracing::warn!("Failed to emit reader_state event: {}", e);
    }

    // Optionally hide the main window to the tray while the reader runs.
    if let Some(state) = app_handle.try_state::<Arc<crate::state::AppState>>() {
        if state.get_config_snapshot().minimize_to_tray_on_reader_start {
            if let Some(win) = app_handle.get_webview_window("main") {
                if enabled {
                    let _ = win.hide();
                } else {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        }
    }
}

/// Helper function to emit validation_error event
fn emit_validation_error(app_handle: &tauri::AppHandle, error: String) {
    let payload = ValidationErrorEvent { error };
    if let Err(e) = app_handle.emit("validation_error", payload) {
        tracing::warn!("Failed to emit validation_error event: {}", e);
    }
}

/// Helper function to emit preset_loaded event
fn emit_preset_loaded(app_handle: &tauri::AppHandle, path: String, name: String) {
    let payload = PresetLoadedEvent { path, name };
    if let Err(e) = app_handle.emit("preset_loaded", payload) {
        tracing::warn!("Failed to emit preset_loaded event: {}", e);
    }
}

/// Helper function to emit debug_log event (optional, for key operations)
fn emit_debug_log(app_handle: &tauri::AppHandle, level: String, message: String) {
    let payload = DebugLogEvent { level, message };
    if let Err(e) = app_handle.emit("debug_log", payload) {
        tracing::warn!("Failed to emit debug_log event: {}", e);
    }
}

/// Helper function to emit a user-facing notice (shown as a warning toast)
fn emit_notice(app_handle: &tauri::AppHandle, message: String) {
    let payload = NoticeEvent { message };
    if let Err(e) = app_handle.emit("notice", payload) {
        tracing::warn!("Failed to emit notice event: {}", e);
    }
}

/// Show the on-screen HUD overlay toast (key + action text) via the native
/// Win32 layered window, replacing the old HTML webview HUD.
fn show_hud(context: &Arc<AppContext>, key: &str, text: impl Into<String>) {
    let region = active_region_rect(context);
    crate::hud_overlay::show(key, &text.into(), 1500, region);
}

/// Show the HUD toast for a hotkey action, using that action's bound key.
fn hud_for(context: &Arc<AppContext>, action: &str, text: impl Into<String>) {
    let key = context
        .state
        .get_config_snapshot()
        .key_bindings
        .get(action)
        .cloned()
        .unwrap_or_default()
        .to_uppercase();
    show_hud(context, &key, text);
}

/// Get the screen rect of the currently active OCR capture region.
fn active_region_rect(context: &Arc<AppContext>) -> crate::hud_overlay::MonitorRect {
    use crate::region::ActiveRegion;
    let config = context.state.get_config_snapshot();
    let active = context.region_manager.lock().active_region();
    let mr = match active {
        ActiveRegion::Monitor1 => config.monitor,
        ActiveRegion::Monitor2 => crate::config::MonitorRect {
            top: config.monitor2_top,
            left: config.monitor2_left,
            width: config.monitor2_width,
            height: config.monitor2_height,
        },
    };
    // Map region coords to screen (handles window-relative scaling).
    let (sx, sy, sw, sh) = map_region_to_screen(&config, &mr);
    crate::hud_overlay::MonitorRect {
        top: sy,
        left: sx,
        width: sw as u32,
        height: sh as u32,
    }
}

/// Persist only the audio prefs that hotkeys/sliders tweak (reader volume, game
/// ducking, playback speeds) to a small file, so they survive restarts without
/// the user having to save a full preset.
fn save_audio_prefs(context: &Arc<AppContext>) {
    let c = context.state.get_config_snapshot();
    let prefs = serde_json::json!({
        "READER_VOLUME": c.reader_volume,
        "VOLUME_REDUCTION_LEVEL": c.volume_reduction_level,
        "BASE_PLAYBACK_SPEED": c.base_playback_speed,
        "OVERLAP_PLAYBACK_SPEED": c.overlap_playback_speed,
    });
    if let Ok(dir) = context.app_handle.path().app_data_dir() {
        let path = dir.join("audio_prefs.json");
        if let Ok(s) = serde_json::to_string_pretty(&prefs) {
            if let Err(e) = std::fs::write(&path, s) {
                tracing::warn!("Failed to save audio prefs: {}", e);
            }
        }
    }
}

/// Tell the settings UI that the config changed (e.g. a hotkey adjusted volume
/// or speed) so it can reload the sliders/values.
fn notify_config_changed(context: &Arc<AppContext>) {
    let c = context.state.get_config_snapshot();
    let payload = ConfigChangedEvent {
        volume_reduction_level: c.volume_reduction_level as f64,
        reader_volume: c.reader_volume as f64,
        base_playback_speed: c.base_playback_speed as f64,
        overlap_playback_speed: c.overlap_playback_speed as f64,
    };
    if let Err(e) = context.app_handle.emit("config_changed", payload) {
        tracing::warn!("Failed to emit config_changed event: {}", e);
    }
    // Hotkeys changed an audio pref -> persist it.
    save_audio_prefs(context);
}

/// Application context shared across commands
pub struct AppContext {
    pub state: Arc<AppState>,
    pub pipeline: Arc<TokioMutex<Pipeline>>,
    pub region_manager: Arc<Mutex<RegionManager>>,
    pub ocr_engine: Arc<Mutex<Box<dyn OcrEngine>>>,
    pub app_handle: tauri::AppHandle,
    pub tray_handle: Arc<Mutex<Option<tauri::tray::TrayIcon>>>,
    pub hotkey_manager: Option<Arc<crate::hotkeys::HotkeyManager>>,
}

/// Parses a "WIDTHxHEIGHT" string into (w, h), falling back to 1920x1080.
fn parse_base_resolution(res: &str) -> (u32, u32) {
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

/// Maps an OCR region (in base-resolution coordinates) to the actual on-screen
/// rectangle over the game window's client area.
///
/// The capture region is defined relative to the base resolution and scaled to
/// the live window client area, so the overlay must be mapped the same way:
/// `screen = client_origin + region * (client_size / base_resolution)`.
/// Falls back to the raw region rect if the game window can't be located
/// (e.g. fullscreen at the base resolution, where the mapping is identity).
#[allow(dead_code)]
fn overlay_screen_rect(
    config: &crate::config::AppConfig,
    region: &crate::config::MonitorRect,
) -> crate::config::MonitorRect {
    // Prefer the EXACT rectangle the capture is reading right now (published by
    // grab()). This guarantees the overlay matches the OCR area precisely.
    if let Some((x, y, w, h)) = crate::capture_wgc::last_ocr_rect() {
        tracing::info!("overlay: using live OCR capture rect ({},{},{}x{})", x, y, w, h);
        return crate::config::MonitorRect {
            left: x,
            top: y,
            width: w.max(1) as u32,
            height: h.max(1) as u32,
        };
    }

    // Fallback (reader not running): approximate by mapping the region into the
    // game window's client area on screen.
    if let Some((cx, cy, cw, ch)) =
        crate::capture_wgc::window_client_screen_rect(&config.capture_window_query)
    {
        let (base_w, base_h) = parse_base_resolution(&config.resolution);
        let sx = cw as f32 / base_w as f32;
        let sy = ch as f32 / base_h as f32;
        crate::config::MonitorRect {
            left: cx + (region.left as f32 * sx).round() as i32,
            top: cy + (region.top as f32 * sy).round() as i32,
            width: (region.width as f32 * sx).round().max(1.0) as u32,
            height: (region.height as f32 * sy).round().max(1.0) as u32,
        }
    } else {
        tracing::warn!(
            "overlay map: window '{}' NOT found and no live capture -> raw region coords",
            config.capture_window_query
        );
        region.clone()
    }
}

/// Reposition the overlay window to the active capture region, if visible.
///
/// Called whenever the configuration or active region changes so the frame
/// keeps tracking the real OCR capture area.
pub fn refresh_region_overlay(context: &AppContext) {
    let config = context.state.get_config_snapshot();
    if !config.enable_region_overlay {
        return;
    }
    crate::region_overlay::show_regions(overlay_region_rects(&config), center_lines(&config));
}

/// Build the center-line parameters for the overlay from the config.
fn center_lines(config: &crate::config::AppConfig) -> crate::region_overlay::CenterLines {
    crate::region_overlay::CenterLines {
        l1: config.use_center_line_1,
        l2: config.use_center_line_2,
        l3: config.use_center_line_3,
        margin: config.center_line_margin,
        l2_start: config.center_line_2_start,
        l3_ratio: config.center_line_3_start_ratio,
    }
}

/// Static mapping of one region (base-resolution coords) to screen, via the
/// game window's client area. Used as the overlay fallback when the reader
/// isn't running (no live capture rects).
fn map_region_to_screen(
    config: &crate::config::AppConfig,
    region: &crate::config::MonitorRect,
) -> (i32, i32, i32, i32) {
    if let Some((cx, cy, cw, ch)) =
        crate::capture_wgc::window_client_screen_rect(&config.capture_window_query)
    {
        let (base_w, base_h) = parse_base_resolution(&config.resolution);
        let sx = cw as f32 / base_w as f32;
        let sy = ch as f32 / base_h as f32;
        (
            cx + (region.left as f32 * sx).round() as i32,
            cy + (region.top as f32 * sy).round() as i32,
            (region.width as f32 * sx).round().max(1.0) as i32,
            (region.height as f32 * sy).round().max(1.0) as i32,
        )
    } else {
        (region.left, region.top, region.width.max(1) as i32, region.height.max(1) as i32)
    }
}

/// Screen rectangles for ALL enabled regions (region 1, and region 2 if on).
fn overlay_region_rects(config: &crate::config::AppConfig) -> Vec<(i32, i32, i32, i32)> {
    let mut rects = vec![map_region_to_screen(config, &config.monitor)];
    if config.monitor2_enabled {
        let r2 = crate::config::MonitorRect {
            top: config.monitor2_top,
            left: config.monitor2_left,
            width: config.monitor2_width,
            height: config.monitor2_height,
        };
        rects.push(map_region_to_screen(config, &r2));
    }
    rects
}

/// Show or hide the on-screen frame marking the OCR capture region.
///
/// When shown, a transparent, click-through, always-on-top window is placed
/// exactly over the active capture region so the user can see what the OCR
/// engine is looking at.
#[tauri::command]
pub async fn set_region_overlay(visible: bool, context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    // Persist the preference so it survives restarts / presets.
    context.state.update_config(|c| c.enable_region_overlay = visible);

    tracing::info!("set_region_overlay called: visible={}", visible);

    if visible {
        let config = context.state.get_config_snapshot();
        let rects = overlay_region_rects(&config);
        tracing::info!("Overlay (native) showing {} region frame(s): {:?}", rects.len(), rects);
        crate::region_overlay::show_regions(rects, center_lines(&config));
    } else {
        crate::region_overlay::hide_region();
        tracing::info!("Native overlay hidden");
    }

    Ok(())
}

/// Enable the reader (start capture and OCR pipeline).
///
/// Validates configuration before starting.
/// Returns Ok(()) on success, or Err(message) if validation fails or pipeline start fails.
#[tauri::command]
pub async fn enable_reader(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    let mut pipeline = context.pipeline.lock().await;
    let result = pipeline.start();
    
    match result {
        Ok(()) => {
            emit_reader_state(&context.app_handle, true);

            
            // Update hotkey manager reader state
            if let Some(hotkey_mgr) = &context.hotkey_manager {
                hotkey_mgr.set_reader_enabled(true);
            }
            
            Ok(())
        }
        Err(error) => {
            emit_validation_error(&context.app_handle, error.clone());
            Err(error)
        }
    }
}

/// Disable the reader (stop capture and OCR pipeline).
#[tauri::command]
pub async fn disable_reader(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    let mut pipeline = context.pipeline.lock().await;
    pipeline.stop().await;
    emit_reader_state(&context.app_handle, false);

    
    // Update hotkey manager reader state
    if let Some(hotkey_mgr) = &context.hotkey_manager {
        hotkey_mgr.set_reader_enabled(false);
    }
    
    Ok(())
}

/// Toggle the reader on/off.
#[tauri::command]
pub async fn toggle_reader(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    let is_running = {
        let pipeline = context.pipeline.lock().await;
        pipeline.is_running()
    };
    
    if is_running {
        let mut pipeline = context.pipeline.lock().await;
        pipeline.stop().await;
        emit_reader_state(&context.app_handle, false);
    
        
        // Update hotkey manager reader state
        if let Some(hotkey_mgr) = &context.hotkey_manager {
            hotkey_mgr.set_reader_enabled(false);
        }
        
        Ok(())
    } else {
        let mut pipeline = context.pipeline.lock().await;
        let result = pipeline.start();
        
        match result {
            Ok(()) => {
                emit_reader_state(&context.app_handle, true);
    
                
                // Update hotkey manager reader state
                if let Some(hotkey_mgr) = &context.hotkey_manager {
                    hotkey_mgr.set_reader_enabled(true);
                }
                
                // Minimize to tray if enabled in config
                let config = context.state.get_config_snapshot();
                if config.minimize_to_tray_on_reader_start {
                    if let Some(window) = context.app_handle.get_webview_window("main") {
                        let _ = window.hide();
                        crate::logging::user_log("🔽 Okno zminimalizowane do zasobnika systemowego");
                    }
                }
                
                Ok(())
            }
            Err(error) => {
                emit_validation_error(&context.app_handle, error.clone());
                Err(error)
            }
        }
    }
}

/// Interrupt currently playing audio and skip to next item.
#[tauri::command]
pub fn interrupt_audio(_context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    // TODO: Implement audio interrupt via pipeline or audio player
    // For now, this is a placeholder
    tracing::info!("Audio interrupt requested");
    Ok(())
}

/// Skip to next line with +10% speed boost.
#[tauri::command]
pub async fn skip_next_line(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    let pipeline = context.pipeline.lock().await;
    pipeline.send_audio_command(crate::audio::AudioCommand::SkipToNextWithBoost)
}

/// Trigger a hotkey action from the frontend (used so shortcuts also work while/// the GameReader window itself is focused). Respects the same access policy as
/// the global keyboard hook: when the reader is off, only a few actions are
/// allowed.
#[tauri::command]
pub fn trigger_hotkey(action: String, context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    let reader_on = context.state.get_runtime_snapshot().capture_enabled;
    let allowed = reader_on
        || crate::constants::ALLOWED_HOTKEYS_WHEN_READER_OFF.contains(&action.as_str());
    if !allowed {
        return Ok(());
    }
    handle_hotkey_action(context.inner(), &action);
    Ok(())
}

/// Return recent log lines (for populating the debug console on open).
#[tauri::command]
pub fn get_recent_logs() -> Vec<String> {
    crate::logging::recent_logs()
}

/// Return recent user-friendly log lines (simple view).
#[tauri::command]
pub fn get_recent_user_logs() -> Vec<String> {
    crate::logging::recent_user_logs()
}

/// Play a bundled system sound by name (e.g. "announcement", "ping").
#[tauri::command]
pub fn play_sound(name: String) {
    crate::system_sounds::play(&name);
}

/// Export the captured log history to a user-chosen text file. A bare filename
/// (no directory, used as a fallback when the save dialog is unavailable) is
/// placed in the app data directory. Returns the final path.
#[tauri::command]
pub fn export_logs(path: String, context: State<'_, Arc<AppContext>>) -> Result<String, String> {
    let mut target = std::path::PathBuf::from(&path);
    let has_dir = target.parent().map(|p| !p.as_os_str().is_empty()).unwrap_or(false);
    if !has_dir {
        if let Ok(dir) = context.app_handle.path().app_data_dir() {
            target = dir.join(&path);
        }
    }
    let content = crate::logging::recent_logs().join("\r\n");
    std::fs::write(&target, content)
        .map_err(|e| format!("Nie udało się zapisać logów: {}", e))?;
    let final_path = target.display().to_string();
    tracing::info!("Logs exported to: {}", final_path);
    Ok(final_path)
}

/// Hide the debug console window (called from the debug window itself, since the
/// global hotkey hook is suppressed while our own windows are focused).
#[tauri::command]
pub fn close_debug_console(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    if let Some(window) = context.app_handle.get_webview_window("debug") {
        let _ = window.hide();
    }
    context.state.update_runtime(|runtime| runtime.debug_enabled = false);
    Ok(())
}

/// Fully quit the application (used by the window close button).
/// Stops the reader pipeline first so audio/OCR don't linger, then exits.
#[tauri::command]
pub async fn exit_app(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    {
        let mut pipeline = context.pipeline.lock().await;
        if pipeline.is_running() {
            pipeline.stop().await;
        }
    }
    tracing::info!("Exiting application (user requested via window close)");
    context.app_handle.exit(0);
    Ok(())
}
#[tauri::command]
pub fn switch_active_region(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    let mut region_mgr = context.region_manager.lock();
    
    let active_region = region_mgr.toggle()?;
    drop(region_mgr);
    
    // Move the OCR overlay to the newly active region.
    refresh_region_overlay(&context);
    
    // TODO: Play system sound (area1.ogg or area2.ogg)
    tracing::info!("Switched to {:?}", active_region);
    
    Ok(())
}

/// Load a preset from the specified file path.
///
/// Auto-discover resource files in the preset's own folder for any fields that
/// the preset JSON left empty. Only looks inside the directory containing the
/// preset file. For subtitles, if BOTH subtitlesPL.txt and subtitlesEN.txt
/// exist it leaves the path empty and emits a `subtitle_choice` event so the
/// UI can ask the user which language to use.
fn discover_preset_resources(config: &mut AppConfig, preset_path: &str, app_handle: &tauri::AppHandle) {
    let dir = match std::path::Path::new(preset_path).parent() {
        Some(d) => d.to_path_buf(),
        None => return,
    };

    // audio/ folder
    if config.audio_dir.trim().is_empty() {
        let p = dir.join("audio");
        if p.is_dir() {
            config.audio_dir = p.to_string_lossy().to_string();
            crate::logging::user_log(format!("🔍 Znaleziono folder audio: {}", p.display()));
        }
    }
    // Candidate directories to search for text resources: the preset folder
    // itself and a "subtitles" subfolder (some games keep them there).
    let search_dirs: Vec<std::path::PathBuf> = vec![dir.clone(), dir.join("subtitles")];
    let find = |name: &str| -> Option<std::path::PathBuf> {
        search_dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
    };

    // names.txt (preset folder or subtitles/ subfolder)
    if config.names_file_path.trim().is_empty() {
        if let Some(p) = find("names.txt") {
            crate::logging::user_log(format!("🔍 Znaleziono plik imion: {}", p.display()));
            config.names_file_path = p.to_string_lossy().to_string();
        }
    }
    // screenshots/ folder
    if config.screenshot_dir.trim().is_empty() {
        let p = dir.join("screenshots");
        if p.is_dir() {
            config.screenshot_dir = p.to_string_lossy().to_string();
        }
    }
    // subtitles: subtitlesPL.txt / subtitlesEN.txt (preset folder or subtitles/)
    if config.text_file_path.trim().is_empty() {
        let pl = find("subtitlesPL.txt");
        let en = find("subtitlesEN.txt");
        match (pl, en) {
            (Some(pl), Some(en)) => {
                // Ambiguous — let the UI ask which language.
                crate::logging::user_log("🔍 Znaleziono napisy PL i EN — pytam o wybór".to_string());
                let _ = app_handle.emit(
                    "subtitle_choice",
                    serde_json::json!({
                        "pl": pl.to_string_lossy().to_string(),
                        "en": en.to_string_lossy().to_string(),
                    }),
                );
            }
            (Some(pl), None) => {
                crate::logging::user_log(format!("🔍 Znaleziono napisy: {}", pl.display()));
                config.text_file_path = pl.to_string_lossy().to_string();
            }
            (None, Some(en)) => {
                crate::logging::user_log(format!("🔍 Znaleziono napisy: {}", en.display()));
                config.text_file_path = en.to_string_lossy().to_string();
            }
            (None, None) => {}
        }
    }
}

/// Updates the application state with the loaded configuration.
#[tauri::command]
pub async fn load_preset(
    path: String,
    context: State<'_, Arc<AppContext>>,
    preset_manager: State<'_, PresetManager>,
) -> Result<AppConfig, String> {
    // Stop the reader before switching presets so OCR/audio don't keep running
    // with the old configuration.
    {
        let mut pipeline = context.pipeline.lock().await;
        if pipeline.is_running() {
            pipeline.stop().await;
            tracing::info!("Reader stopped due to preset load");
        }
    }
    emit_reader_state(&context.app_handle, false);
    if let Some(hotkey_mgr) = &context.hotkey_manager {
        hotkey_mgr.set_reader_enabled(false);
    }

    let (config, hotkey_issues) = preset_manager.load_with_report(std::path::Path::new(&path))
        .map_err(|e| e.to_string())?;

    // If the preset's shortcuts had problems we auto-fixed, tell the user.
    if !hotkey_issues.is_empty() {
        let msg = format!("Poprawiono skróty w presecie: {}", hotkey_issues.join("; "));
        emit_notice(&context.app_handle, msg);
    }

    // Auto-discover resource files (audio/, subtitlesPL/EN.txt, names.txt,
    // screenshots/) from the preset's own folder for any fields left empty.
    let mut config = config;
    discover_preset_resources(&mut config, &path, &context.app_handle);

    // Update application state
    context.state.replace_config(config.clone());
    
    // Add to recent presets (use path filename as name)
    let preset_name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let _ = preset_manager.add_to_recent(std::path::Path::new(&path), preset_name.clone());
    
    // Update runtime state with preset info
    context.state.update_runtime(|state| {
        state.preset_filename = preset_name.clone();
        state.preset_path = path.clone();
    });
    
    tracing::info!("Loaded preset from: {}", path);
    crate::logging::user_log(format!("📁 Wczytano preset: {}", preset_name));
    
    // Emit preset_loaded event
    emit_preset_loaded(&context.app_handle, path.clone(), preset_name);
    
    // Update tray tooltip with new preset name

    
    // Update hotkey manager with new key bindings
    if let Some(hotkey_mgr) = &context.hotkey_manager {
        if let Err(e) = hotkey_mgr.update_bindings(config.key_bindings.clone()) {
            tracing::warn!("Failed to update hotkey bindings: {}", e);
        } else {
            tracing::info!("Hotkey bindings updated from preset");
        }
    }
    
    Ok(config)
}

/// Save the current configuration to the specified file path.
#[tauri::command]
pub fn save_preset(
    path: String,
    context: State<'_, Arc<AppContext>>,
    preset_manager: State<'_, PresetManager>,
) -> Result<(), String> {
    let config = context.state.get_config_snapshot();
    preset_manager.save(std::path::Path::new(&path), &config)
        .map_err(|e| e.to_string())?;
    
    // Add to recent presets (use path filename as name)
    let preset_name = std::path::Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let _ = preset_manager.add_to_recent(std::path::Path::new(&path), preset_name);
    
    tracing::info!("Saved preset to: {}", path);
    Ok(())
}

/// Get the list of recent presets.
#[tauri::command]
pub fn get_recent_presets(
    preset_manager: State<'_, PresetManager>,
) -> Result<Vec<crate::config::RecentPreset>, String> {
    preset_manager.get_recent()
        .map_err(|e| e.to_string())
}

/// Remove a single preset from the recent list.
#[tauri::command]
pub fn remove_recent_preset(
    path: String,
    preset_manager: State<'_, PresetManager>,
) -> Result<(), String> {
    preset_manager.remove_from_recent(&path).map_err(|e| e.to_string())
}

/// Clear the whole recent presets list.
#[tauri::command]
pub fn clear_recent_presets(
    preset_manager: State<'_, PresetManager>,
) -> Result<(), String> {
    preset_manager.clear_recent().map_err(|e| e.to_string())
}

/// Get the currently loaded preset name.
#[tauri::command]
pub fn get_current_preset(
    context: State<'_, Arc<AppContext>>,
) -> Result<String, String> {
    let state = context.state.get_runtime_snapshot();
    Ok(state.preset_filename)
}

/// Handle tray menu actions.
#[tauri::command]
pub async fn tray_action(
    action: String,
    context: State<'_, Arc<AppContext>>,
) -> Result<(), String> {
    match action.as_str() {
        "pokaz_ustawienia" => {
            if let Some(window) = context.app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        "konsola_debug" => {
            if let Some(window) = context.app_handle.get_webview_window("debug") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "toggle_lektor" => {
            let is_running = {
                let pipeline = context.pipeline.lock().await;
                pipeline.is_running()
            };
            if is_running {
                let mut pipeline = context.pipeline.lock().await;
                let _ = pipeline.stop().await;
                emit_reader_state(&context.app_handle, false);
                if let Some(hotkey_mgr) = &context.hotkey_manager {
                    hotkey_mgr.set_reader_enabled(false);
                }
            } else {
                let mut pipeline = context.pipeline.lock().await;
                let result = pipeline.start();
                if result.is_ok() {
                    emit_reader_state(&context.app_handle, true);
                    if let Some(hotkey_mgr) = &context.hotkey_manager {
                        hotkey_mgr.set_reader_enabled(true);
                    }
                }
            }
        }
        "zamknij_aplikacje" => {
            context.app_handle.exit(0);
        }
        _ => {
            tracing::warn!("Unknown tray action: {}", action);
        }
    }
    Ok(())
}

/// Called by the launcher window to finish startup: show the main window and
/// close the launcher.
#[tauri::command]
pub fn finish_launch(context: State<'_, Arc<AppContext>>) -> Result<(), String> {
    if let Some(main) = context.app_handle.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    if let Some(launcher) = context.app_handle.get_webview_window("launcher") {
        let _ = launcher.close();
    }
    Ok(())
}

/// Update configuration with partial updates.
///
/// This allows updating specific fields without replacing the entire config.
/// The new_config parameter should contain only the fields to update.
#[tauri::command]
pub fn update_config(
    new_config: serde_json::Value,
    context: State<'_, Arc<AppContext>>,
) -> Result<(), String> {
    // Partial merge: serialize current config to JSON, overlay incoming keys, deserialize back.
    // This tolerates partial updates and ignores unknown/extra fields from the frontend.
    let current = context.state.get_config_snapshot();
    let mut current_value = serde_json::to_value(&current)
        .map_err(|e| format!("Failed to serialize current config: {}", e))?;
    
    if let (Some(current_obj), Some(new_obj)) = (current_value.as_object_mut(), new_config.as_object()) {
        for (key, value) in new_obj {
            // Skip null values (frontend may send undefined as null)
            if !value.is_null() {
                current_obj.insert(key.clone(), value.clone());
            }
        }
    } else {
        return Err("Invalid configuration format".to_string());
    }
    
    let merged: AppConfig = serde_json::from_value(current_value)
        .map_err(|e| {
            tracing::error!("update_config: failed to deserialize merged config: {}", e);
            format!("Failed to deserialize merged config: {}", e)
        })?;
    
    tracing::info!("update_config: audio_dir='{}', text_file_path='{}'", merged.audio_dir, merged.text_file_path);
    context.state.replace_config(merged);
    tracing::info!("Configuration updated (partial merge)");

    // Persist audio prefs (reader volume, game ducking, playback speeds) so the
    // slider changes also survive a restart.
    save_audio_prefs(&context);

    // Keep the OCR region overlay aligned with the (possibly changed) capture region.
    refresh_region_overlay(&context);

    Ok(())
}

/// Get the current configuration.
#[tauri::command]
pub fn get_config(context: State<'_, Arc<AppContext>>) -> Result<AppConfig, String> {
    Ok(context.state.get_config_snapshot())
}

/// Get the current runtime state.
#[tauri::command]
pub fn get_runtime_state(
    context: State<'_, Arc<AppContext>>,
) -> Result<crate::config::RuntimeState, String> {
    Ok(context.state.get_runtime_snapshot())
}

/// List capturable windows for the "window" capture mode picker.
///
/// Returns visible top-level windows with their title and owning process name.
#[tauri::command]
pub fn list_windows() -> Result<Vec<crate::capture_wgc::WindowInfo>, String> {
    Ok(crate::capture_wgc::enumerate_windows())
}

/// List the physical monitors for the multi-monitor picker.
#[tauri::command]
pub fn list_monitors() -> Result<Vec<crate::capture_wgc::MonitorInfo>, String> {
    Ok(crate::capture_wgc::enumerate_monitors())
}

/// Show the native alt+tab-style window picker (live DWM thumbnails) and return
/// the chosen window's capture query (process name or title), or None.
///
/// Used when starting the reader with no game window specified.
#[derive(Debug, Clone, Serialize)]
pub struct PickedWindow {
    /// String used for matching/persisting (process name, or title fallback).
    pub query: String,
    /// Human-readable window title for display ("which window am I reading").
    pub title: String,
}

#[tauri::command]
pub async fn pick_window(context: State<'_, Arc<AppContext>>) -> Result<Option<PickedWindow>, String> {
    // Hide our window so it isn't a candidate and doesn't cover the picker.
    let main = context.app_handle.get_webview_window("main");
    if let Some(w) = &main {
        let _ = w.hide();
    }
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let chosen = tauri::async_runtime::spawn_blocking(crate::window_picker::pick_window)
        .await
        .map_err(|e| format!("Window picker task failed: {e}"))?;

    if let Some(w) = &main {
        let _ = w.show();
        let _ = w.set_focus();
    }

    // Resolve the chosen HWND to a query string AND pin the exact window, so
    // capture targets precisely this window even if several share a process
    // name (e.g. multiple brave.exe instances).
    let Some(hwnd_raw) = chosen else {
        return Ok(None);
    };
    match crate::capture_wgc::window_info_for_hwnd(hwnd_raw) {
        Some((query, title)) => {
            crate::capture_wgc::set_pinned_target(hwnd_raw, query.clone());
            Ok(Some(PickedWindow { query, title }))
        }
        None => Ok(None),
    }
}

/// Forget the exact pinned window (called when the user edits the query field
/// by hand, so name-based matching takes over again).
#[tauri::command]
pub fn clear_pinned_window() -> Result<(), String> {
    crate::capture_wgc::clear_pinned_target();
    Ok(())
}

/// Detect the client-area resolution of the window matching `query`.
///
/// Returns Some((width, height)) if a matching window is found, or None.
/// Used by the UI to show the detected game resolution and to set it as the
/// region base resolution.
#[tauri::command]
pub fn detect_window_resolution(query: String) -> Result<Option<(u32, u32)>, String> {
    Ok(crate::capture_wgc::window_resolution_for_query(&query))
}

/// Result of an interactive screen-region selection, in base-resolution
/// region coordinates ready to drop into the OCR region fields.
#[derive(Debug, Clone, Serialize)]
pub struct SelectedRegion {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

/// Interactive "Win+Shift+S"-style screen-region selector.
///
/// Hides the main window, shows a full-screen snipping overlay, and converts
/// the chosen screen rectangle into region coordinates relative to the game
/// window's client area at the base resolution (the inverse of the scaling
/// `grab()` applies). The game window is taken from `capture_window_query`, or
/// the current foreground window when that is empty.
///
/// Returns `Ok(None)` if the user cancels (ESC / right-click / tiny selection).
#[tauri::command]
pub async fn select_screen_region(
    context: State<'_, Arc<AppContext>>,
) -> Result<Option<SelectedRegion>, String> {
    let config = context.state.get_config_snapshot();

    // Resolve the capture target BEFORE touching window focus. Right now OUR
    // settings window is the foreground one, so the auto-detector correctly
    // returns the last game window (tracked), not us.
    let target = crate::capture_wgc::resolve_target_hwnd(&config.capture_window_query);

    // Hide the main window so it doesn't cover the game while selecting.
    let main = context.app_handle.get_webview_window("main");
    if let Some(w) = &main {
        let _ = w.hide();
    }
    // Give the compositor a moment to actually remove our window.
    tokio::time::sleep(std::time::Duration::from_millis(180)).await;

    // Raise the game window to the top so the user selects over the GAME, not
    // over whatever else (YouTube, browser, ...) happened to be behind us.
    if let Some(h) = target {
        crate::capture_wgc::bring_to_foreground(h);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    // Client rect of the target window (screen px). Passed to the selector so
    // everything outside the game is blacked out, and reused for conversion.
    let client = target.and_then(crate::capture_wgc::client_screen_rect_for);
    // Reveal the game window if found, otherwise the chosen monitor, otherwise
    // the whole virtual screen.
    let game_rect = client
        .map(|(x, y, w, h)| (x, y, w as i32, h as i32))
        .or_else(|| crate::capture_wgc::monitor_rect_by_id(&config.capture_monitor));

    // Run the blocking Win32 selector off the async executor.
    let screen = tauri::async_runtime::spawn_blocking(move || {
        crate::region_selector::select_region(game_rect)
    })
    .await
    .map_err(|e| format!("Region selector task failed: {e}"))?;

    // Restore the main window.
    if let Some(w) = &main {
        let _ = w.show();
        let _ = w.set_focus();
    }

    let Some((sx, sy, sw, sh)) = screen else {
        return Ok(None); // cancelled
    };

    let Some((cx, cy, cw, ch)) = client else {
        return Err(
            "Nie znaleziono okna gry. Podaj nazwę procesu w „Szybki start\" lub ustaw grę na pierwszym planie.".to_string(),
        );
    };

    let (base_w, base_h) = parse_base_resolution(&config.resolution);
    // Inverse of grab() scaling: screen px -> client-relative -> base coords.
    let to_base_x = |v: i32| ((v as f32) * base_w as f32 / cw as f32).round();
    let to_base_y = |v: i32| ((v as f32) * base_h as f32 / ch as f32).round();

    let left = to_base_x(sx - cx).max(0.0) as i32;
    let top = to_base_y(sy - cy).max(0.0) as i32;
    let width = to_base_x(sw).max(1.0) as u32;
    let height = to_base_y(sh).max(1.0) as u32;

    tracing::info!(
        "select_screen_region: screen=({},{},{}x{}) client=({},{},{}x{}) base={}x{} -> region=({},{},{}x{})",
        sx, sy, sw, sh, cx, cy, cw, ch, base_w, base_h, left, top, width, height
    );

    Ok(Some(SelectedRegion { left, top, width, height }))
}

/// Handle hotkey actions (called from hotkey manager)
/// This is not a Tauri command - it's called internally by the hotkey system
/// Show or hide the debug console window. The window itself is defined in
/// tauri.conf.json (hidden at startup) so its URL resolves correctly in both
/// dev (dev server) and release (embedded assets). MUST run on the main thread.
fn toggle_debug_window(context: &Arc<AppContext>) {
    use tauri::Manager;

    if let Some(window) = context.app_handle.get_webview_window("debug") {
        let visible = window.is_visible().unwrap_or(false);
        if visible {
            let _ = window.hide();
            context.state.update_runtime(|runtime| runtime.debug_enabled = false);
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            // Ask the page to reload the full history (it may have accumulated
            // while the window was hidden and not receiving live lines).
            let _ = window.emit("debug_show", ());
            context.state.update_runtime(|runtime| runtime.debug_enabled = true);
        }
    } else {
        tracing::warn!("Debug window not found (should be defined in tauri.conf.json)");
    }

    let runtime = context.state.get_runtime_snapshot();
    tracing::info!("Debug console: {}", if runtime.debug_enabled { "enabled" } else { "disabled" });
}

pub fn handle_hotkey_action(context: &Arc<AppContext>, action: &str) {
    tracing::info!("Handling hotkey action: {}", action);
    
    // Send debug log if debug is enabled
    let runtime = context.state.get_runtime_snapshot();
    if runtime.debug_enabled {
        emit_debug_log(&context.app_handle, "INFO".to_string(), format!("Hotkey action: {}", action));
    }
    
    match action {
        "toggle_reader" | "enable_reader" | "disable_reader" => {
            // Use async runtime to call async command
            let context_clone = Arc::clone(context);
            let action = action.to_string();
            tauri::async_runtime::spawn(async move {
                match action.as_str() {
                    "enable_reader" => {
                        let mut pipeline = context_clone.pipeline.lock().await;
                        if let Ok(()) = pipeline.start() {
                            emit_reader_state(&context_clone.app_handle, true);

                            hud_for(&context_clone, "enable_reader", "Uruchomiono lektora");
                            
                            let runtime = context_clone.state.get_runtime_snapshot();
                            if runtime.debug_enabled {
                                emit_debug_log(&context_clone.app_handle, "INFO".to_string(), "Reader started successfully".to_string());
                            }
                        }
                    }
                    "disable_reader" => {
                        let mut pipeline = context_clone.pipeline.lock().await;
                        pipeline.stop().await;
                        emit_reader_state(&context_clone.app_handle, false);

                        hud_for(&context_clone, "disable_reader", "Zatrzymano lektora");
                        
                        let runtime = context_clone.state.get_runtime_snapshot();
                        if runtime.debug_enabled {
                            emit_debug_log(&context_clone.app_handle, "INFO".to_string(), "Reader stopped".to_string());
                        }
                    }
                    "toggle_reader" => {
                        let is_running = {
                            let pipeline = context_clone.pipeline.lock().await;
                            pipeline.is_running()
                        };
                        
                        if is_running {
                            let mut pipeline = context_clone.pipeline.lock().await;
                            pipeline.stop().await;
                            emit_reader_state(&context_clone.app_handle, false);

                            hud_for(&context_clone, "toggle_reader", "Zatrzymano lektora");
                        } else {
                            let mut pipeline = context_clone.pipeline.lock().await;
                            if let Ok(()) = pipeline.start() {
                                emit_reader_state(&context_clone.app_handle, true);
    
                                hud_for(&context_clone, "toggle_reader", "Uruchomiono lektora");
                            }
                        }
                    }
                    _ => {}
                }
                
                // Update hotkey manager reader state
                if let Some(hotkey_mgr) = &context_clone.hotkey_manager {
                    let runtime = context_clone.state.get_runtime_snapshot();
                    hotkey_mgr.set_reader_enabled(runtime.capture_enabled);
                }
            });
        }
        
        "interrupt_audio" => {
            // Send skip command to pipeline
            let context_clone = Arc::clone(context);
            tauri::async_runtime::spawn(async move {
                let pipeline = context_clone.pipeline.lock().await;
                if let Err(e) = pipeline.send_audio_command(crate::audio::AudioCommand::SkipToNextWithBoost) {
                    tracing::warn!("Failed to send skip audio command: {}", e);
                } else {
                    tracing::info!("Skip audio command sent via hotkey");
                }
                drop(pipeline);
                hud_for(&context_clone, "interrupt_audio", "Przerwano kwestię lektora");
            });
        }
        
        "skip_next_line" => {
            // Send skip command to pipeline with speed boost
            let context_clone = Arc::clone(context);
            tauri::async_runtime::spawn(async move {
                let pipeline = context_clone.pipeline.lock().await;
                if let Err(e) = pipeline.send_audio_command(crate::audio::AudioCommand::SkipToNextWithBoost) {
                    tracing::warn!("Failed to send skip next line command: {}", e);
                } else {
                    tracing::info!("Skip next line (+10% speed) command sent via hotkey");
                }
                hud_for(&context_clone, "skip_next_line", "Pominięto kwestię");
            });
        }
        
        "switch_monitor_toggle" => {
            let mut region_mgr = context.region_manager.lock();
            if let Ok(active_region) = region_mgr.toggle() {
                tracing::info!("Switched to {:?} via hotkey", active_region);
                let text = match active_region {
                    crate::region::ActiveRegion::Monitor1 => {
                        crate::system_sounds::play("area1");
                        "Przełączono na region 1"
                    }
                    crate::region::ActiveRegion::Monitor2 => {
                        crate::system_sounds::play("area2");
                        "Przełączono na region 2"
                    }
                };
                hud_for(context, "switch_monitor_toggle", text);
            }
        }
        
        "toggle_areas" => {
            // Show / hide the on-screen OCR region overlay frame.
            let now_visible = !context.state.get_config_snapshot().enable_region_overlay;
            context.state.update_config(|config| {
                config.enable_region_overlay = now_visible;
            });
            let config = context.state.get_config_snapshot();
            if now_visible {
                let rects = overlay_region_rects(&config);
                crate::region_overlay::show_regions(rects, center_lines(&config));
            } else {
                crate::region_overlay::hide_region();
            }
            // Reflect the new state in the settings UI checkbox.
            let _ = context.app_handle.emit("region_overlay_changed", now_visible);
            hud_for(
                context,
                "toggle_areas",
                if now_visible { "Pokazano obszar wykrywania" } else { "Ukryto obszar wykrywania" },
            );
            tracing::info!("Toggled OCR region overlay via hotkey: {}", now_visible);
        }
        
        "open_settings" => {
            // Toggle the main settings window (show if hidden/minimized, hide
            // if visible). Run on the main thread for reliable window ops.
            let key = context.state.get_config_snapshot().key_bindings
                .get("open_settings").cloned().unwrap_or_default().to_uppercase();
            let context_clone = Arc::clone(context);
            let _ = context.app_handle.run_on_main_thread(move || {
                use tauri::Manager;
                if let Some(window) = context_clone.app_handle.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    let minimized = window.is_minimized().unwrap_or(false);
                    if visible && !minimized {
                        let _ = window.hide();
                        show_hud(&context_clone, &key, "Ukryto ustawienia");
                    } else {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                        show_hud(&context_clone, &key, "Pokazano ustawienia");
                    }
                }
            });
        }
        
        "volume_up" => {
            context.state.update_config(|config| {
                config.reader_volume = (config.reader_volume + 0.05).min(1.0);
            });
            let config = context.state.get_config_snapshot();
            tracing::info!("Reader volume: {:.2}", config.reader_volume);
            hud_for(context, "volume_up", format!("Głośność lektora ({}%)", (config.reader_volume * 100.0).round() as i32));
            notify_config_changed(context);
        }
        
        "volume_down" => {
            context.state.update_config(|config| {
                config.reader_volume = (config.reader_volume - 0.05).max(0.0);
            });
            let config = context.state.get_config_snapshot();
            tracing::info!("Reader volume: {:.2}", config.reader_volume);
            hud_for(context, "volume_down", format!("Głośność lektora ({}%)", (config.reader_volume * 100.0).round() as i32));
            notify_config_changed(context);
        }
        
        "base_speed_up" => {
            context.state.update_config(|config| {
                config.base_playback_speed = (config.base_playback_speed + crate::constants::SPEED_STEP)
                    .min(crate::constants::BASE_PLAYBACK_SPEED_MAX);
            });
            let config = context.state.get_config_snapshot();
            tracing::info!("Base playback speed: {:.2}", config.base_playback_speed);
            hud_for(context, "base_speed_up", format!("Prędkość lektora zwiększona ({:.2}x)", config.base_playback_speed));
            notify_config_changed(context);
        }
        
        "base_speed_down" => {
            context.state.update_config(|config| {
                config.base_playback_speed = (config.base_playback_speed - crate::constants::SPEED_STEP)
                    .max(crate::constants::BASE_PLAYBACK_SPEED_MIN);
            });
            let config = context.state.get_config_snapshot();
            tracing::info!("Base playback speed: {:.2}", config.base_playback_speed);
            hud_for(context, "base_speed_down", format!("Prędkość lektora zmniejszona ({:.2}x)", config.base_playback_speed));
            notify_config_changed(context);
        }
        
        "overlap_speed_up" => {
            context.state.update_config(|config| {
                config.overlap_playback_speed = (config.overlap_playback_speed + crate::constants::SPEED_STEP)
                    .min(crate::constants::OVERLAP_PLAYBACK_SPEED_MAX);
            });
            let config = context.state.get_config_snapshot();
            tracing::info!("Overlap playback speed: {:.2}", config.overlap_playback_speed);
            hud_for(context, "overlap_speed_up", format!("Prędkość doganiania zwiększona ({:.2}x)", config.overlap_playback_speed));
            notify_config_changed(context);
        }
        
        "overlap_speed_down" => {
            context.state.update_config(|config| {
                config.overlap_playback_speed = (config.overlap_playback_speed - crate::constants::SPEED_STEP)
                    .max(crate::constants::OVERLAP_PLAYBACK_SPEED_MIN);
            });
            let config = context.state.get_config_snapshot();
            tracing::info!("Overlap playback speed: {:.2}", config.overlap_playback_speed);
            hud_for(context, "overlap_speed_down", format!("Prędkość doganiania zmniejszona ({:.2}x)", config.overlap_playback_speed));
            notify_config_changed(context);
        }
        
        "test_sound" => {
            tracing::info!("Test sound requested via hotkey");
            crate::system_sounds::play("test");
            hud_for(context, "test_sound", "Test dźwięku");
        }
        
        "debug_console" => {
            let key = context.state.get_config_snapshot().key_bindings
                .get("debug_console").cloned().unwrap_or_default().to_uppercase();
            let context_clone = Arc::clone(context);
            let _ = context.app_handle.run_on_main_thread(move || {
                toggle_debug_window(&context_clone);
                let on = context_clone.state.get_runtime_snapshot().debug_enabled;
                show_hud(&context_clone, &key, if on { "Pokazano konsolę debug" } else { "Ukryto konsolę debug" });
            });
        }
        
        _ => {
            tracing::warn!("Unknown hotkey action: {}", action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: Tests that use AppContext require a full Tauri application context,
    // which is not easily mockable in unit tests. These tests are disabled for now.
    // Integration tests should be used to test the full command handler functionality.
    
    #[test]
    fn test_event_payload_serialization() {
        // Test that event payloads serialize correctly
        let reader_state = ReaderStateEvent { enabled: true };
        let json = serde_json::to_string(&reader_state).unwrap();
        assert!(json.contains("true"));
        
        let validation_error = ValidationErrorEvent { 
            error: "Test error".to_string() 
        };
        let json = serde_json::to_string(&validation_error).unwrap();
        assert!(json.contains("Test error"));
        
        let preset_loaded = PresetLoadedEvent {
            path: "/path/to/preset.json".to_string(),
            name: "preset".to_string(),
        };
        let json = serde_json::to_string(&preset_loaded).unwrap();
        assert!(json.contains("preset.json"));
    }
}
