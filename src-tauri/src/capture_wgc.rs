//! Windows Graphics Capture (WGC) backend - captures a specific window/app.
//!
//! WGC is the modern Windows capture API. Unlike GDI BitBlt it works in
//! flip-model fullscreen and captures a chosen window by HWND, so OCR can read
//! a game even in fullscreen. Capture runs on its own thread (via the
//! `windows-capture` crate); each arriving frame is stored as the "latest
//! frame" and `grab()` crops the requested region from it.

use crate::capture::{CaptureError, CaptureRegion, Frame};
use std::sync::Arc;
use parking_lot::Mutex;

use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame as WgcFrame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

/// Lightweight info about a capturable window (for the UI picker).
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfo {
    /// Window title text.
    pub title: String,
    /// Owning process executable name (e.g. "GTA-SA.exe").
    pub process_name: String,
    /// Raw HWND as a string (for stable identification).
    pub hwnd: String,
}

/// Most recently captured full-window frame (tightly packed BGRA).
struct LatestFrame {
    width: u32,
    height: u32,
    bgra: Vec<u8>,
}

type SharedFrame = Arc<Mutex<Option<LatestFrame>>>;

/// The exact on-screen rectangle (x, y, w, h in physical pixels) of the most
/// recent OCR capture crop, or `None` when not capturing. Published by `grab()`
/// and read by the region overlay so the frame matches the capture exactly.
pub static LAST_OCR_RECT: std::sync::Mutex<Option<(i32, i32, i32, i32)>> =
    std::sync::Mutex::new(None);

/// Returns the most recent OCR capture rectangle on screen, if available.
pub fn last_ocr_rect() -> Option<(i32, i32, i32, i32)> {
    LAST_OCR_RECT.lock().ok().and_then(|g| *g)
}

/// On-screen rectangles of ALL active OCR capture regions this cycle (one entry
/// per region: one when a single region is used, two when the second region is
/// enabled). Published by the capture worker, read by the region overlay so it
/// can draw a frame around each region.
pub static OVERLAY_RECTS: std::sync::Mutex<Vec<(i32, i32, i32, i32)>> =
    std::sync::Mutex::new(Vec::new());

/// Publish the set of on-screen rectangles the overlay should outline.
pub fn set_overlay_rects(rects: Vec<(i32, i32, i32, i32)>) {
    if let Ok(mut g) = OVERLAY_RECTS.lock() {
        *g = rects;
    }
}

/// Returns the current set of OCR region rectangles to outline.
pub fn overlay_rects() -> Vec<(i32, i32, i32, i32)> {
    OVERLAY_RECTS.lock().map(|g| g.clone()).unwrap_or_default()
}

/// Screen-space top-left of the WGC frame = the window's VISIBLE rectangle, via
/// DWM extended frame bounds. This is the authoritative origin the captured
/// frame maps to (excludes invisible resize borders; includes title bar/visible
/// borders), so the overlay lands exactly on the captured area automatically.
#[cfg(windows)]
fn frame_screen_origin(hwnd_raw: isize) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};

    if hwnd_raw == 0 {
        return None;
    }
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let mut rect = RECT::default();
        let res = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut core::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        );
        if res.is_ok() {
            Some((rect.left, rect.top))
        } else {
            // Fallback to client origin if DWM query fails.
            window_origin(hwnd_raw)
        }
    }
}

#[cfg(not(windows))]
fn frame_screen_origin(hwnd_raw: isize) -> Option<(i32, i32)> {
    window_origin(hwnd_raw)
}

/// Screen-space top-left of a window's CLIENT area (physical pixels).
///
/// The WGC frame is the window's client surface, so the captured frame's (0,0)
/// corresponds to the client area top-left on screen — not the full window
/// top-left (which sits above the title bar). Using ClientToScreen here keeps
/// the published OCR rectangle aligned with what is actually captured.
#[cfg(windows)]
fn window_origin(hwnd_raw: isize) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    if hwnd_raw == 0 {
        return None;
    }
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let mut p = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(hwnd, &mut p);
        Some((p.x, p.y))
    }
}

#[cfg(not(windows))]
fn window_origin(_hwnd_raw: isize) -> Option<(i32, i32)> {
    None
}

/// Debug helper: (window left, window top, client width, client height).
#[cfg(windows)]
fn window_rect_dbg(hwnd_raw: isize) -> (i32, i32, i32, i32) {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect};
    if hwnd_raw == 0 {
        return (0, 0, 0, 0);
    }
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        let mut cr = RECT::default();
        let _ = GetClientRect(hwnd, &mut cr);
        (wr.left, wr.top, cr.right - cr.left, cr.bottom - cr.top)
    }
}

#[cfg(not(windows))]
fn window_rect_dbg(_hwnd_raw: isize) -> (i32, i32, i32, i32) {
    (0, 0, 0, 0)
}

/// Frame handler that copies each WGC frame into shared storage.
struct FrameHandler {
    shared: SharedFrame,
}

impl GraphicsCaptureApiHandler for FrameHandler {
    type Flags = SharedFrame;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self { shared: ctx.flags })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut WgcFrame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let fb = frame.buffer()?;
        let width = fb.width();
        let height = fb.height();

        // Get tightly-packed BGRA (ColorFormat::Bgra8 set in settings)
        let mut scratch = Vec::new();
        let data = fb.as_nopadding_buffer(&mut scratch);

        let mut latest = self.shared.lock();
        *latest = Some(LatestFrame {
            width,
            height,
            bgra: data.to_vec(),
        });

        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        tracing::info!("WGC capture session closed (target window gone)");
        Ok(())
    }
}

/// WGC window-capture backend.
pub struct WgcCapture {
    shared: SharedFrame,
    control: Option<CaptureControl<FrameHandler, Box<dyn std::error::Error + Send + Sync>>>,
    target_desc: String,
    /// Raw HWND of the captured window (for client-area mapping).
    hwnd_raw: isize,
    /// Base resolution the region coordinates were defined for (screen pixels).
    base_w: u32,
    base_h: u32,
}

impl WgcCapture {
    /// Creates a new (not yet started) WGC capture.
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(None)),
            control: None,
            target_desc: String::new(),
            hwnd_raw: 0,
            base_w: 1920,
            base_h: 1080,
        }
    }

    /// Finds a target window matching `query`.
    ///
    /// Matching order:
    /// 1. Process executable name equals `query` (case-insensitive), e.g. "GTA-SA.exe"
    /// 2. Window title contains `query` (case-insensitive)
    fn find_window(query: &str) -> Result<Window, CaptureError> {
        let query_trim = query.trim();

        let windows = Window::enumerate()
            .map_err(|e| CaptureError::Other(format!("Failed to enumerate windows: {e}")))?;

        // Exact pinned window (chosen in the thumbnail picker): if the user
        // picked a specific window and the query still matches what we pinned
        // it for, capture THAT exact window — this disambiguates several
        // windows sharing one process name (e.g. multiple brave.exe).
        {
            let pin = PINNED_TARGET.lock();
            if pin.0 != 0 && !pin.1.is_empty() && pin.1 == query_trim {
                let pinned_raw = pin.0;
                drop(pin);
                for w in &windows {
                    if w.as_raw_hwnd() as isize == pinned_raw {
                        return Ok(*w);
                    }
                }
                // Pinned window no longer exists -> fall through to name match.
            }
        }

        // Empty query: fall back to the auto-detected game window (the last
        // non-self foreground window). At UI-start time our own settings window
        // is the foreground one, so a plain GetForegroundWindow isn't enough —
        // resolve_game_hwnd remembers the game the user was focused on.
        if query_trim.is_empty() {
            #[cfg(windows)]
            {
                if let Some(target_raw) = resolve_game_hwnd() {
                    for w in &windows {
                        if w.as_raw_hwnd() as isize == target_raw {
                            return Ok(*w);
                        }
                    }
                }
            }
            return Err(CaptureError::Other(
                "No window query set and no usable game window detected".into(),
            ));
        }

        // 1. Match by process executable name
        for w in &windows {
            if let Ok(proc) = w.process_name() {
                if proc.eq_ignore_ascii_case(query_trim) {
                    return Ok(*w);
                }
            }
        }

        // 2. Match by title substring (case-insensitive)
        let q_lower = query_trim.to_lowercase();
        for w in &windows {
            if let Ok(title) = w.title() {
                if title.to_lowercase().contains(&q_lower) {
                    return Ok(*w);
                }
            }
        }

        Err(CaptureError::Other(format!("No window matching '{query}'")))
    }

    /// Starts capturing the window identified by `query` (process name or title).
    ///
    /// `base_w`/`base_h` are the resolution the OCR region coordinates were
    /// defined for (screen pixels); used to scale the region into the window's
    /// client area when the game runs windowed.
    pub fn start_for_window(&mut self, query: &str, base_w: u32, base_h: u32) -> Result<(), CaptureError> {
        // Stop any existing session first
        self.stop();

        let window = Self::find_window(query)?;
        self.hwnd_raw = window.as_raw_hwnd() as isize;
        // base_w/base_h is the REFERENCE resolution the OCR regions were
        // defined for. grab() scales regions from this reference to the live
        // window client area, so regions adapt to any actual resolution /
        // aspect ratio (e.g. 720p -> 4K, 16:9 -> 21:9 / 5:4).
        self.base_w = base_w.max(1);
        self.base_h = base_h.max(1);
        let desc = format!(
            "{} [{}]",
            window.title().unwrap_or_default(),
            window.process_name().unwrap_or_default()
        );

        let settings = Settings::new(
            window,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            // We only OCR a few times per second; throttle to ~8 FPS to keep
            // the per-frame memcpy overhead low.
            MinimumUpdateIntervalSettings::Custom(std::time::Duration::from_millis(125)),
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            Arc::clone(&self.shared),
        );

        let control = FrameHandler::start_free_threaded(settings)
            .map_err(|e| CaptureError::WgcInitFailed { reason: format!("{e}") })?;

        self.control = Some(control);
        self.target_desc = desc.clone();
        tracing::info!("WGC capture started for window: {} (region base resolution {}x{})",
            desc, self.base_w, self.base_h);
        Ok(())
    }

    /// Stops the capture session if running.
    pub fn stop(&mut self) {
        if let Some(control) = self.control.take() {
            let _ = control.stop();
            tracing::info!("WGC capture stopped for: {}", self.target_desc);
        }
        *self.shared.lock() = None;
        if let Ok(mut last) = LAST_OCR_RECT.lock() {
            *last = None;
        }
    }

    /// Returns true if a capture session is active.
    pub fn is_running(&self) -> bool {
        self.control.is_some()
    }

    /// Crops the requested region from the latest captured window frame.
    ///
    /// `region` coordinates are in base-resolution screen pixels. They are
    /// mapped into the window's client area (excluding title bar / borders) and
    /// scaled to the current window size, so it works both fullscreen (scale=1)
    /// and windowed (scaled, title-bar-aware).
    pub fn grab(&mut self, region: &CaptureRegion) -> Result<Frame, CaptureError> {
        if !region.is_valid() {
            return Err(CaptureError::InvalidRegion {
                details: format!("Invalid region: {}x{}", region.width, region.height),
            });
        }

        let latest = self.shared.lock();
        let frame = latest.as_ref().ok_or_else(|| {
            CaptureError::Other("No WGC frame available yet".into())
        })?;

        let fw = frame.width as f32;
        let fh = frame.height as f32;

        // Determine the client area within the captured frame.
        //
        // The WGC frame is the full window surface (client + title bar +
        // borders). We use the ACTUAL client size (GetClientRect) and derive
        // the border/title-bar offset from the frame vs client size. This is
        // exact (unlike fraction-of-GetWindowRect, which includes invisible
        // borders) so region scaling is precise at any resolution.
        let (cw, ch) = window_client_size(self.hwnd_raw)
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((fw, fh));
        let client_w = cw.max(1.0);
        let client_h = ch.max(1.0);
        let left_border = ((fw - client_w) / 2.0).max(0.0);
        let title_bar = (fh - client_h - left_border).max(0.0);
        let client_off_x = left_border;
        let client_off_y = title_bar;

        // Scale region (base-resolution coords) to the live client area size.
        // Independent X/Y scaling adapts to any aspect ratio (16:9, 21:9, 5:4).
        let scale_x = client_w / self.base_w as f32;
        let scale_y = client_h / self.base_h as f32;

        let crop_x = (client_off_x + region.left as f32 * scale_x).round() as i32;
        let crop_y = (client_off_y + region.top as f32 * scale_y).round() as i32;
        let crop_w = (region.width as f32 * scale_x).round() as i32;
        let crop_h = (region.height as f32 * scale_y).round() as i32;

        // Clamp to frame bounds
        let src_x = crop_x.clamp(0, frame.width as i32 - 1) as u32;
        let src_y = crop_y.clamp(0, frame.height as i32 - 1) as u32;
        let avail_w = (frame.width - src_x) as i32;
        let avail_h = (frame.height - src_y) as i32;
        let copy_w = crop_w.clamp(1, avail_w.max(1)) as usize;
        let copy_h = crop_h.clamp(1, avail_h.max(1)) as usize;

        // Publish the exact on-screen rectangle being captured. The WGC frame
        // maps to the window's VISIBLE rectangle (DWM extended frame bounds),
        // so frame coords + that origin = screen coords. Fully automatic, no
        // border/title-bar guessing.
        if let Some((wx, wy)) = frame_screen_origin(self.hwnd_raw) {
            if let Ok(mut last) = LAST_OCR_RECT.lock() {
                *last = Some((wx + src_x as i32, wy + src_y as i32, copy_w as i32, copy_h as i32));
            }
        }

        // Throttled geometry debug (~1/s) to diagnose overlay alignment.
        {
            use std::sync::atomic::{AtomicU64, Ordering};
            static LAST_GEO_LOG: AtomicU64 = AtomicU64::new(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now != LAST_GEO_LOG.swap(now, Ordering::Relaxed) {
                let client_origin = window_origin(self.hwnd_raw);
                let wr = window_rect_dbg(self.hwnd_raw);
                tracing::info!(
                    "[GEO] frame={}x{} clientOrigin={:?} winRect(l,t,cw,ch)={:?} src=({},{}) copy=({}x{}) base={}x{} scale=({:.3},{:.3})",
                    frame.width, frame.height, client_origin, wr,
                    src_x, src_y, copy_w, copy_h, self.base_w, self.base_h, scale_x, scale_y
                );
            }
        }

        // Output buffer is the requested (base) region size; we resize the
        // scaled crop back up so OCR always sees the configured region size.
        let dst_w = region.width as usize;
        let dst_h = region.height as usize;
        let dst_stride = dst_w * 4;
        let src_stride = frame.width as usize * 4;
        let mut out = vec![0u8; dst_stride * dst_h];

        // Nearest-neighbor resample from the scaled crop (copy_w x copy_h at
        // src_x,src_y) into the dst_w x dst_h output.
        for dy in 0..dst_h {
            let sy = src_y as usize + (dy * copy_h) / dst_h;
            let src_row = sy * src_stride;
            let dst_row = dy * dst_stride;
            for dx in 0..dst_w {
                let sx = src_x as usize + (dx * copy_w) / dst_w;
                let s = src_row + sx * 4;
                let d = dst_row + dx * 4;
                out[d] = frame.bgra[s];
                out[d + 1] = frame.bgra[s + 1];
                out[d + 2] = frame.bgra[s + 2];
                out[d + 3] = 255;
            }
        }

        Frame::from_data(region.width, region.height, dst_stride, out)
    }
}

impl Default for WgcCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WgcCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Computes the client-area rectangle within the full window, as fractions
/// (offset_x, offset_y, width, height) of the window size.
///
/// Used to map region coordinates past the title bar / borders when capturing
/// a windowed game. Returns `None` if the window handle is invalid.
#[cfg(windows)]
#[allow(dead_code)]
fn client_area_fractions(hwnd_raw: isize) -> Option<(f32, f32, f32, f32)> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect};

    if hwnd_raw == 0 {
        return None;
    }

    unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let mut wr = RECT::default();
        let mut cr = RECT::default();
        if GetWindowRect(hwnd, &mut wr).is_err() {
            return None;
        }
        if GetClientRect(hwnd, &mut cr).is_err() {
            return None;
        }

        let win_w = (wr.right - wr.left).max(1) as f32;
        let win_h = (wr.bottom - wr.top).max(1) as f32;
        let client_w = (cr.right - cr.left).max(1) as f32;
        let client_h = (cr.bottom - cr.top).max(1) as f32;

        // Side/bottom border thickness (assume symmetric horizontal borders),
        // top offset = title bar + top border.
        let border = ((win_w - client_w) / 2.0).max(0.0);
        let top = (win_h - client_h - border).max(0.0);

        Some((border / win_w, top / win_h, client_w / win_w, client_h / win_h))
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
fn client_area_fractions(_hwnd_raw: isize) -> Option<(f32, f32, f32, f32)> {
    None
}

/// Returns the window's client-area size in pixels (width, height).
///
/// Used to auto-detect the game's resolution so OCR region coordinates are
/// interpreted in the window's native pixels (no manual base-resolution setup).
#[cfg(windows)]
fn window_client_size(hwnd_raw: isize) -> Option<(u32, u32)> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    if hwnd_raw == 0 {
        return None;
    }

    unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let mut cr = RECT::default();
        if GetClientRect(hwnd, &mut cr).is_err() {
            return None;
        }
        let w = (cr.right - cr.left).max(0) as u32;
        let h = (cr.bottom - cr.top).max(0) as u32;
        if w == 0 || h == 0 {
            None
        } else {
            Some((w, h))
        }
    }
}

#[cfg(not(windows))]
fn window_client_size(_hwnd_raw: isize) -> Option<(u32, u32)> {
    None
}

/// Finds a window matching `query` (process name or title) and returns its
/// client-area resolution in pixels. Used by the UI to display the detected
/// resolution and to set it as the region base resolution.
pub fn window_resolution_for_query(query: &str) -> Option<(u32, u32)> {
    let window = WgcCapture::find_window(query).ok()?;
    let hwnd_raw = window.as_raw_hwnd() as isize;
    window_client_size(hwnd_raw)
}

/// Client area of the window matching `query`, in SCREEN pixels: (x, y, w, h).
///
/// Used to position the OCR region overlay correctly over the game window
/// (accounts for window position, borders and title bar).
pub fn window_client_screen_rect(query: &str) -> Option<(i32, i32, u32, u32)> {
    let window = WgcCapture::find_window(query).ok()?;
    let hwnd_raw = window.as_raw_hwnd() as isize;
    client_screen_rect(hwnd_raw)
}

/// Client area of the current FOREGROUND window, in SCREEN pixels: (x, y, w, h).
///
/// Used by the region selector when no explicit game window query is set: the
/// game in the foreground is targeted automatically.
#[cfg(windows)]
pub fn foreground_client_screen_rect() -> Option<(i32, i32, u32, u32)> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let raw = unsafe { GetForegroundWindow().0 as isize };
    if raw == 0 {
        return None;
    }
    client_screen_rect(raw)
}

#[cfg(not(windows))]
pub fn foreground_client_screen_rect() -> Option<(i32, i32, u32, u32)> {
    None
}

/// HWND (as isize) of the last foreground window that did NOT belong to our own
/// process — i.e. the game the user was most recently focused on. Used as the
/// auto-target when no explicit window query is set, because at the moment the
/// reader is started from the UI our own settings window is the foreground one.
static LAST_GAME_HWND: Mutex<isize> = Mutex::new(0);

/// Start a lightweight background poller that records the last foreground window
/// that isn't ours. Idempotent. Cheap (one GetForegroundWindow call ~3x/sec).
#[cfg(windows)]
pub fn start_foreground_tracker() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(|| {
            use windows::Win32::System::Threading::GetCurrentProcessId;
            use windows::Win32::UI::WindowsAndMessaging::{
                GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
            };
            let self_pid = unsafe { GetCurrentProcessId() };
            loop {
                unsafe {
                    let hwnd = GetForegroundWindow();
                    let raw = hwnd.0 as isize;
                    if raw != 0 && IsWindowVisible(hwnd).as_bool() {
                        let mut pid = 0u32;
                        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
                        if pid != 0 && pid != self_pid {
                            *LAST_GAME_HWND.lock() = raw;
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        });
    });
}

#[cfg(not(windows))]
pub fn start_foreground_tracker() {}

/// Resolve the auto-target game window HWND (as isize) when no query is set:
/// prefer the current foreground window if it isn't ours, otherwise fall back
/// to the last tracked non-self foreground window.
#[cfg(windows)]
fn resolve_game_hwnd() -> Option<isize> {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible,
    };
    unsafe {
        let self_pid = GetCurrentProcessId();
        let hwnd = GetForegroundWindow();
        let raw = hwnd.0 as isize;
        if raw != 0 && IsWindowVisible(hwnd).as_bool() {
            let mut pid = 0u32;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid != 0 && pid != self_pid {
                return Some(raw);
            }
        }
    }
    let g = LAST_GAME_HWND.lock();
    if *g != 0 {
        Some(*g)
    } else {
        None
    }
}

/// Client area of the auto-detected game window (no explicit query), in SCREEN
/// pixels: (x, y, w, h).
#[cfg(windows)]
pub fn auto_game_client_screen_rect() -> Option<(i32, i32, u32, u32)> {
    let raw = resolve_game_hwnd()?;
    client_screen_rect(raw)
}

#[cfg(not(windows))]
pub fn auto_game_client_screen_rect() -> Option<(i32, i32, u32, u32)> {
    None
}

/// Resolve the capture target HWND (as isize): the window matching `query`, or
/// the auto-detected game window when `query` is empty.
pub fn resolve_target_hwnd(query: &str) -> Option<isize> {
    let q = query.trim();
    if q.is_empty() {
        #[cfg(windows)]
        {
            return resolve_game_hwnd();
        }
        #[cfg(not(windows))]
        {
            return None;
        }
    }
    WgcCapture::find_window(q).ok().map(|w| w.as_raw_hwnd() as isize)
}

/// Bring the given window to the foreground (above other apps) so it is the
/// only thing visible under the region-selection overlay.
#[cfg(windows)]
pub fn bring_to_foreground(hwnd_raw: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };
    if hwnd_raw == 0 {
        return;
    }
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(not(windows))]
pub fn bring_to_foreground(_hwnd_raw: isize) {}

/// Client-area screen rectangle for a specific HWND (public wrapper).
pub fn client_screen_rect_for(hwnd_raw: isize) -> Option<(i32, i32, u32, u32)> {
    client_screen_rect(hwnd_raw)
}

/// Lightweight info about a physical monitor (for the UI picker).
#[derive(Debug, Clone, serde::Serialize)]
pub struct MonitorInfo {
    /// Device name, e.g. "\\\\.\\DISPLAY1" — stable id used in config.
    pub id: String,
    /// Display label for the UI.
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

#[cfg(windows)]
unsafe extern "system" fn monitor_enum_proc(
    hmon: windows::Win32::Graphics::Gdi::HMONITOR,
    _hdc: windows::Win32::Graphics::Gdi::HDC,
    _rc: *mut windows::Win32::Foundation::RECT,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO, MONITORINFOEXW};

    let out = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = core::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmon, &mut mi.monitorInfo as *mut MONITORINFO).as_bool() {
        let r = mi.monitorInfo.rcMonitor;
        // MONITORINFOF_PRIMARY == 0x1
        let primary = (mi.monitorInfo.dwFlags & 0x1) != 0;
        let dev = String::from_utf16_lossy(&mi.szDevice);
        let dev = dev.trim_end_matches('\0').to_string();
        out.push(MonitorInfo {
            id: dev.clone(),
            name: dev,
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
            primary,
        });
    }
    windows::Win32::Foundation::BOOL(1)
}

/// Enumerate the physical monitors (primary first, then left-to-right).
#[cfg(windows)]
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC};

    let mut monitors: Vec<MonitorInfo> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
        );
    }
    monitors.sort_by(|a, b| b.primary.cmp(&a.primary).then(a.x.cmp(&b.x)));
    monitors
}

#[cfg(not(windows))]
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    Vec::new()
}

/// Screen rect (x, y, w, h) of the monitor with the given device id, if found.
pub fn monitor_rect_by_id(id: &str) -> Option<(i32, i32, i32, i32)> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    enumerate_monitors()
        .into_iter()
        .find(|m| m.id == id)
        .map(|m| (m.x, m.y, m.width, m.height))
}

/// Enumerate top-level windows as (raw HWND, title, process_name) tuples, for
/// the native DWM-thumbnail window picker.
pub fn enumerate_window_handles() -> Vec<(isize, String, String)> {
    let windows = match Window::enumerate() {
        Ok(w) => w,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for w in windows {
        let title = w.title().unwrap_or_default();
        if title.trim().is_empty() {
            continue;
        }
        let proc = w.process_name().unwrap_or_default();
        out.push((w.as_raw_hwnd() as isize, title, proc));
    }
    out
}

/// Resolve a raw HWND to a capture query string (process name, or title if the
/// process name is unavailable).
pub fn query_for_hwnd(raw: isize) -> Option<String> {
    enumerate_window_handles()
        .into_iter()
        .find(|(h, _, _)| *h == raw)
        .map(|(_, title, proc)| if !proc.is_empty() { proc } else { title })
}

/// Resolve a raw HWND to (capture query, window title) for display.
pub fn window_info_for_hwnd(raw: isize) -> Option<(String, String)> {
    enumerate_window_handles()
        .into_iter()
        .find(|(h, _, _)| *h == raw)
        .map(|(_, title, proc)| {
            let query = if !proc.is_empty() { proc.clone() } else { title.clone() };
            (query, title)
        })
}

/// Exact window chosen in the thumbnail picker: (raw HWND, the query string it
/// was pinned for). Lets capture target one specific window even when several
/// share a process name. Cleared when the user edits the query manually.
static PINNED_TARGET: Mutex<(isize, String)> = Mutex::new((0, String::new()));

/// Pin an exact window (by HWND) as the capture target for the given query.
pub fn set_pinned_target(hwnd_raw: isize, query: String) {
    *PINNED_TARGET.lock() = (hwnd_raw, query);
}

/// Forget any pinned window (e.g. when the user types a query by hand).
pub fn clear_pinned_target() {
    *PINNED_TARGET.lock() = (0, String::new());
}

#[cfg(windows)]
fn client_screen_rect(hwnd_raw: isize) -> Option<(i32, i32, u32, u32)> {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    if hwnd_raw == 0 {
        return None;
    }

    unsafe {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        let mut cr = RECT::default();
        if GetClientRect(hwnd, &mut cr).is_err() {
            return None;
        }
        let w = (cr.right - cr.left).max(1) as u32;
        let h = (cr.bottom - cr.top).max(1) as u32;

        // Convert the client area's top-left (0,0) directly to screen
        // coordinates. This is exact and avoids guessing border / title-bar
        // thickness (which was placing the overlay in the wrong spot).
        let mut origin = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(hwnd, &mut origin);

        Some((origin.x, origin.y, w, h))
    }
}

#[cfg(not(windows))]
fn client_screen_rect(_hwnd_raw: isize) -> Option<(i32, i32, u32, u32)> {
    None
}

/// Enumerates capturable windows (visible, top-level) for the UI picker.
///
/// Returns windows that have a non-empty title, sorted by process name.
pub fn enumerate_windows() -> Vec<WindowInfo> {
    let windows = match Window::enumerate() {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("Failed to enumerate windows: {}", e);
            return Vec::new();
        }
    };

    let mut result: Vec<WindowInfo> = Vec::new();
    for w in windows {
        let title = w.title().unwrap_or_default();
        if title.trim().is_empty() {
            continue;
        }
        let process_name = w.process_name().unwrap_or_default();
        result.push(WindowInfo {
            title,
            process_name,
            hwnd: format!("{:?}", w.as_raw_hwnd()),
        });
    }

    result.sort_by(|a, b| a.process_name.to_lowercase().cmp(&b.process_name.to_lowercase()));
    result
}
