//! Native Win32 layered-window HUD matching old Python Qt overlay style.

use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

/// Screen rect of the active OCR capture region – used to position the HUD
/// on the monitor where OCR is running.
#[derive(Clone, Copy, Debug)]
pub struct MonitorRect {
    pub top: i32,
    pub left: i32,
    pub width: u32,
    pub height: u32,
}

struct HudContent {
    key: String,
    text: String,
    region: MonitorRect,
    show_since: Instant,
    duration_ms: u32,
}

static CONTENT: Mutex<Option<HudContent>> = Mutex::new(None);
static STARTED: OnceLock<()> = OnceLock::new();
static LAST_RENDERED: Mutex<Option<(String, String)>> = Mutex::new(None);

const W: i32 = 520;
const H: i32 = 120;
const R: i32 = 14;

pub fn show(key: &str, text: &str, duration_ms: u32, region: MonitorRect) {
    if let Ok(mut c) = CONTENT.lock() {
        *c = Some(HudContent {
            key: key.to_string(),
            text: text.to_string(),
            region,
            show_since: Instant::now(),
            duration_ms,
        });
    }
    STARTED.get_or_init(|| { std::thread::spawn(run_impl); });
}

pub fn hide() {
    if let Ok(mut c) = CONTENT.lock() { *c = None; }
}

#[cfg(windows)]
fn run_impl() { imp::run_hud_thread(); }
#[cfg(not(windows))]
fn run_impl() {}

#[cfg(windows)]
mod imp {
    use core::ffi::c_void;
    use std::sync::Mutex;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteDC, DeleteObject, SelectObject,
        SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS,
        HDC, HGDIOBJ, OPAQUE, BACKGROUND_MODE, MONITORINFO, GetMonitorInfoW,
        MONITOR_DEFAULTTONEAREST, MonitorFromPoint, DrawTextW, DT_CENTER, DT_VCENTER, DT_SINGLELINE, SetBkColor,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use crate::hud_overlay::{LAST_RENDERED, CONTENT, MonitorRect, W, H, R};

    static MDC: Mutex<isize> = Mutex::new(0);
    static BMP: Mutex<isize> = Mutex::new(0);
    static BITS: Mutex<usize> = Mutex::new(0);
    static F_TITLE: Mutex<isize> = Mutex::new(0);
    static F_KEY: Mutex<isize> = Mutex::new(0);
    static F_MSG: Mutex<isize> = Mutex::new(0);

    const fn pm(b: u32, g: u32, r: u32, a: u32) -> u32 {
        (a << 24) | ((r * a / 255) << 16) | ((g * a / 255) << 8) | (b * a / 255)
    }

    const fn rgb(r: u8, g: u8, b: u8) -> u32 {
        0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
    }

    const BG: u32 = pm(0x26, 0x18, 0x1A, 245);
    const SHAD: u32 = pm(0, 0, 0, 50);

    unsafe fn dib() -> *mut u32 {
        {
            let old_dc = *MDC.lock().unwrap();
            let old_bmp = *BMP.lock().unwrap();
            if old_bmp != 0 { let _ = DeleteObject(HGDIOBJ(old_bmp as *mut c_void)); }
            if old_dc != 0 { let _ = DeleteDC(HDC(old_dc as *mut c_void)); }
        }
        let di = BITMAPINFOHEADER {
            biSize: core::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: W, biHeight: -H, biPlanes: 1, biBitCount: 32, biCompression: 0,
            ..Default::default()
        };
        let bmi = BITMAPINFO { bmiHeader: di, ..Default::default() };
        let dc = CreateCompatibleDC(HDC::default());
        let mut ptr: *mut c_void = core::ptr::null_mut();
        let dib = CreateDIBSection(HDC::default(), &bmi, DIB_RGB_COLORS, &mut ptr, HANDLE::default(), 0).unwrap_or_default();
        SelectObject(dc, HGDIOBJ(dib.0));
        *MDC.lock().unwrap() = dc.0 as isize;
        *BMP.lock().unwrap() = dib.0 as isize;
        *BITS.lock().unwrap() = ptr as usize;
        ptr as *mut u32
    }

    unsafe fn fonts(dc: HDC) {
        if *F_TITLE.lock().unwrap() == 0 {
            let f = CreateFontW(-13, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 4, 0, w!("Segoe UI"));
            SelectObject(dc, HGDIOBJ(f.0 as *mut c_void));
            *F_TITLE.lock().unwrap() = f.0 as isize;
        }
        if *F_KEY.lock().unwrap() == 0 {
            let f = CreateFontW(-16, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 4, 0, w!("Consolas"));
            SelectObject(dc, HGDIOBJ(f.0 as *mut c_void));
            *F_KEY.lock().unwrap() = f.0 as isize;
        }
        if *F_MSG.lock().unwrap() == 0 {
            let f = CreateFontW(-18, 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 4, 0, w!("Segoe UI"));
            SelectObject(dc, HGDIOBJ(f.0 as *mut c_void));
            *F_MSG.lock().unwrap() = f.0 as isize;
        }
    }

    fn rr(x: i32, y: i32, rx: i32, ry: i32, rw: i32, rh: i32, r: i32) -> bool {
        if x < rx || x >= rx + rw || y < ry || y >= ry + rh { return false; }
        let tl = x < rx + r && y < ry + r;
        let tr = x >= rx + rw - r && y < ry + r;
        let bl = x < rx + r && y >= ry + rh - r;
        let br = x >= rx + rw - r && y >= ry + rh - r;
        if !tl && !tr && !bl && !br { return true; }
        let (cx, cy) = if tl { (rx + r, ry + r) } else if tr { (rx + rw - r - 1, ry + r) } else if bl { (rx + r, ry + rh - r - 1) } else { (rx + rw - r - 1, ry + rh - r - 1) };
        let (dx, dy) = (x - cx, y - cy);
        dx * dx + dy * dy <= r * r
    }

    // Render text with proper anti-aliased alpha using mask technique:
    // 1. Save bg, fill rect with 0xFF000000, render white text,
    // 2. Use R channel as coverage alpha, blend text_color over saved bg.
    unsafe fn render_text(dc: HDC, buf: &mut [u32], rect: RECT, color: u32, text: &str) {
        let r_left = rect.left.max(0);
        let r_top = rect.top.max(0);
        let r_right = rect.right.min(W);
        let r_bottom = rect.bottom.min(H);
        if r_left >= r_right || r_top >= r_bottom { return; }
        let w_ = (r_right - r_left) as usize;
        let h_ = (r_bottom - r_top) as usize;

        // 1. Save background
        let mut saved: Vec<u32> = Vec::with_capacity(w_ * h_);
        for y in r_top..r_bottom {
            for x in r_left..r_right {
                saved.push(buf[(y * W + x) as usize]);
            }
        }

        // 2. Fill with opaque black
        for y in r_top..r_bottom {
            for x in r_left..r_right {
                buf[(y * W + x) as usize] = 0xFF000000;
            }
        }

        // 3. Render white text
        let old_col = SetTextColor(dc, COLORREF(0xFFFFFF));
        let old_bk = SetBkMode(dc, OPAQUE);
        SetBkColor(dc, COLORREF(0x000000));
        let mut t16: Vec<u16> = text.encode_utf16().collect();
        let mut r2 = rect;
        DrawTextW(dc, &mut t16, &mut r2, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        SetTextColor(dc, old_col);
        SetBkMode(dc, BACKGROUND_MODE(old_bk as u32));

        // 4. Extract alpha from R channel and blend
        let tc_r = (color >> 16) & 0xFF;
        let tc_g = (color >> 8) & 0xFF;
        let tc_b = color & 0xFF;
        let cv = (tc_r as u32) << 16 | (tc_g as u32) << 8 | tc_b as u32;

        for y in 0..h_ {
            for x in 0..w_ {
                let idx = ((r_top + y as i32) * W + (r_left + x as i32)) as usize;
                let pix = buf[idx];
                let coverage = (pix >> 16) & 0xFF; // R channel from white-on-black
                if coverage == 0 {
                    // No text here, restore background
                    buf[idx] = saved[y * w_ + x];
                } else if coverage == 255 {
                    // Fully covered text pixel
                    let sbg = saved[y * w_ + x];
                    let bg_a = (sbg >> 24) as u32;
                    if bg_a == 255 {
                        buf[idx] = cv | 0xFF000000;
                    } else {
                        // Blend over semi-transparent bg
                        let bg_r = (sbg >> 16) & 0xFF;
                        let bg_g = (sbg >> 8) & 0xFF;
                        let bg_b = sbg & 0xFF;
                        let out_a = 255;
                        let out_r = (tc_r * 255 + bg_r * (255 - 255)) / 255; // simplifies to tc_r
                        let out_g = (tc_g * 255 + bg_g * (255 - 255)) / 255;
                        let out_b = (tc_b * 255 + bg_b * (255 - 255)) / 255;
                        buf[idx] = (out_a as u32) << 24 | (out_r as u32) << 16 | (out_g as u32) << 8 | out_b as u32;
                    }
                } else {
                    // Anti-aliased edge pixel
                    let a = coverage as u32;
                    let sbg = saved[y * w_ + x];
                    let bg_a = (sbg >> 24) as u32;
                    let bg_r = (sbg >> 16) & 0xFF;
                    let bg_g = (sbg >> 8) & 0xFF;
                    let bg_b = sbg & 0xFF;

                    if bg_a >= 245 {
                        // Opaque or nearly opaque bg: standard over blend
                        let out_a = 255u32;
                        let out_r = (tc_r * a + bg_r * (255 - a)) / 255;
                        let out_g = (tc_g * a + bg_g * (255 - a)) / 255;
                        let out_b = (tc_b * a + bg_b * (255 - a)) / 255;
                        buf[idx] = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
                    } else {
                        // Semi-transparent bg: alpha-aware blend
                        let out_a = a + (bg_a * (255 - a)) / 255;
                        let out_r = if out_a > 0 { (tc_r * a + bg_r * bg_a * (255 - a) / 255) / out_a } else { 0u32 };
                        let out_g = if out_a > 0 { (tc_g * a + bg_g * bg_a * (255 - a) / 255) / out_a } else { 0u32 };
                        let out_b = if out_a > 0 { (tc_b * a + bg_b * bg_a * (255 - a) / 255) / out_a } else { 0u32 };
                        buf[idx] = (out_a << 24) | ((out_r.min(255) as u32) << 16) | ((out_g.min(255) as u32) << 8) | (out_b.min(255) as u32);
                    }
                }
            }
        }
    }

    unsafe fn comp(hwnd: HWND, key: &str, text: &str, region: MonitorRect) {
        let bits = dib();
        if bits.is_null() { return; }
        let buf = core::slice::from_raw_parts_mut(bits, (W * H) as usize);
        buf.fill(0);
        let dc = HDC(*MDC.lock().unwrap() as *mut c_void);
        fonts(dc);

        // Shadow
        for y in 0..H {
            for x in 0..W {
                if !rr(x, y, 0, 0, W, H, R) && rr(x - 4, y - 6, 0, 0, W, H, R) {
                    buf[(y * W + x) as usize] = SHAD;
                }
            }
        }

        // Background panel
        for y in 0..H {
            for x in 0..W {
                if rr(x, y, 0, 0, W, H, R) {
                    buf[(y * W + x) as usize] = BG;
                }
            }
        }

        // Border
        for y in 0..H {
            for x in 0..W {
                if !rr(x, y, 1, 1, W - 2, H - 2, (R - 1).max(1)) && rr(x, y, 0, 0, W, H, R) {
                    buf[(y * W + x) as usize] = SHAD;
                }
            }
        }

        // Make all panel pixels fully opaque for clean text blending
        for y in 0..H {
            for x in 0..W {
                let idx = (y * W + x) as usize;
                let pix = buf[idx];
                if pix != 0 && (pix & 0xFF000000) != 0xFF000000 {
                    let (r, g, b) = ((pix >> 16) & 0xFF, (pix >> 8) & 0xFF, pix & 0xFF);
                    buf[idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                }
            }
        }

        // Layout: margins 24,18,24,18 spacing 10
        let pad_x = 24;
        let top = 18;
        let title_y = top + 14;
        let key_y = title_y + 10 + 22;
        let text_y = key_y + 10 + 22;

        // Title "GameReader" (accent color 0x816afe -> COLORREF 0xFE6A81)
        SelectObject(dc, HGDIOBJ(*F_TITLE.lock().unwrap() as *mut c_void));
        let tr = RECT { left: pad_x, top: title_y - 10, right: W - pad_x, bottom: title_y + 14 };
        render_text(dc, buf, tr, rgb(0x81, 0x6a, 0xfe), "GameReader");

        // Key text (white 0xF0EEF5)
        SelectObject(dc, HGDIOBJ(*F_KEY.lock().unwrap() as *mut c_void));
        let kr = RECT { left: pad_x, top: key_y - 14, right: W - pad_x, bottom: key_y + 14 };
        render_text(dc, buf, kr, rgb(0xF0, 0xEE, 0xF5), key);

        // Message text (dimmed 0xC0A0A5)
        SelectObject(dc, HGDIOBJ(*F_MSG.lock().unwrap() as *mut c_void));
        let mr = RECT { left: pad_x, top: text_y - 14, right: W - pad_x, bottom: text_y + 14 };
        render_text(dc, buf, mr, rgb(0xC0, 0xA0, 0xA5), text);

        // Push layered window
        let (bx, by) = wpos(hwnd, region);
        let pp = POINT { x: bx, y: by };
        let ps = SIZE { cx: W, cy: H };
        let bl = BLENDFUNCTION { BlendOp: 0, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: 1 };
        let _ = UpdateLayeredWindow(hwnd, HDC::default(), Some(&pp), Some(&ps), dc, Some(&POINT { x: 0, y: 0 }), COLORREF(0), Some(&bl), ULW_ALPHA);
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, bx, by, W, H, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }

    fn wpos(_hwnd: HWND, region: MonitorRect) -> (i32, i32) {
        unsafe {
            let cx = region.left + (region.width as i32) / 2;
            let cy = region.top + (region.height as i32) / 2;
            let pt = POINT { x: cx, y: cy };
            let m = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO::default();
            mi.cbSize = core::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(m, &mut mi).as_bool() {
                let r = mi.rcMonitor;
                return (r.left + (r.right - r.left - W) / 2, r.top + 30);
            }
        }
        (0, 0)
    }

    unsafe extern "system" fn wnd(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_TIMER => { poll(hwnd); LRESULT(0) }
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    unsafe fn poll(hwnd: HWND) {
        let mut c = match CONTENT.lock() { Ok(c) => c, Err(_) => return };
        if c.is_none() { let _ = ShowWindow(hwnd, SW_HIDE); return; }
        let content = c.as_ref().unwrap();
        if content.show_since.elapsed().as_millis() as u32 >= content.duration_ms {
            *c = None;
            let _ = ShowWindow(hwnd, SW_HIDE);
        } else {
            let mut last = LAST_RENDERED.lock().unwrap();
            let tracking = (content.key.clone(), content.text.clone());
            if last.as_ref() != Some(&tracking) {
                comp(hwnd, &content.key, &content.text, content.region);
                *last = Some(tracking);
            }
        }
    }

    pub(crate) fn run_hud_thread() {
        unsafe {
            let hi = GetModuleHandleW(None).unwrap();
            let cn = w!("GameReaderHudOverlay");
            RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(wnd), hInstance: HINSTANCE(hi.0), lpszClassName: cn, ..Default::default() });
            let ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            let h = CreateWindowExW(ex, cn, PCWSTR::null(), WS_POPUP, 0, 0, W, H, None, None, HINSTANCE(hi.0), None).unwrap();
            let init_region = CONTENT.lock().ok().and_then(|c| c.as_ref().map(|h| h.region)).unwrap_or(MonitorRect { top: 0, left: 0, width: 1920, height: 1080 });
            let (cx, cy) = wpos(h, init_region);
            let _ = SetWindowPos(h, HWND_TOPMOST, cx, cy, W, H, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            SetTimer(h, 1, 50, None);
            let mut m = MSG::default();
            while GetMessageW(&mut m, HWND::default(), 0, 0).as_bool() { let _ = TranslateMessage(&m); DispatchMessageW(&m); }
        }
    }
}
