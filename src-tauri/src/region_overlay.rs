//! Native Win32 layered-window overlay marking the OCR capture region(s).
//!
//! Draws a thin coloured frame around EACH active capture region (one when a
//! single region is used, two when the second region is enabled) using GDI in a
//! borderless, always-on-top, click-through layered window. A background colour
//! key makes the interior fully transparent (and clicks pass through), so only
//! the frames are visible over the game.
//!
//! The window covers the bounding box of all regions and lives on its own
//! thread with a Win32 message pump. The desired rectangles / visibility are
//! published into shared state and applied by a short timer.

/// Active center-line parameters, in OCR-region pixel space, used to draw the
/// vertical helper lines inside the first region frame.
#[derive(Clone, Copy)]
pub struct CenterLines {
    pub l1: bool,
    pub l2: bool,
    pub l3: bool,
    pub margin: i32,
    pub l2_start: i32,
    pub l3_ratio: f32,
}

impl CenterLines {
    pub const NONE: CenterLines = CenterLines {
        l1: false,
        l2: false,
        l3: false,
        margin: 0,
        l2_start: 0,
        l3_ratio: 0.0,
    };

    pub fn any(&self) -> bool {
        self.l1 || self.l2 || self.l3
    }
}

#[cfg(windows)]
mod imp {
    use core::ffi::c_void;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HDC, HGDIOBJ,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;

    use super::CenterLines;

    struct OverlayState {
        visible: bool,
        /// Fallback rectangles (screen coords) used when no live capture rects
        /// are available (reader not running).
        rects: Vec<(i32, i32, i32, i32)>,
    }

    static STATE: Mutex<OverlayState> = Mutex::new(OverlayState {
        visible: false,
        rects: Vec::new(),
    });

    // Last set of absolute rects applied (avoids redundant recomposition).
    static LAST_APPLIED: Mutex<Option<Vec<(i32, i32, i32, i32)>>> = Mutex::new(None);
    // Active center-line parameters (drawn inside the first region frame).
    static CL: Mutex<CenterLines> = Mutex::new(CenterLines::NONE);

    // Per-pixel-alpha DIB resources (recreated when the bounding box resizes).
    static MEM_DC: Mutex<isize> = Mutex::new(0);
    static DIB_BMP: Mutex<isize> = Mutex::new(0);
    static BITS: Mutex<usize> = Mutex::new(0);
    static DIM: Mutex<(i32, i32)> = Mutex::new((0, 0));

    static STARTED: OnceLock<()> = OnceLock::new();

    const BORDER: i32 = 2;

    /// Premultiplied-alpha BGRA packed little-endian: (A<<24)|(R<<16)|(G<<8)|B.
    const fn premul(b: u32, g: u32, r: u32, a: u32) -> u32 {
        let pb = b * a / 255;
        let pg = g * a / 255;
        let pr = r * a / 255;
        (a << 24) | (pr << 16) | (pg << 8) | pb
    }
    // Accent #816afe (B=0xFE, G=0x6A, R=0x81) frame, near-opaque.
    const ACCENT: u32 = premul(0xFE, 0x6A, 0x81, 235);
    // Soft blue translucent fill for center-line bands.
    const BAND_FILL: u32 = premul(255, 140, 40, 64);
    // Brighter blue band edges for definition.
    const BAND_EDGE: u32 = premul(255, 160, 60, 205);

    /// Publish the desired rectangles + visibility; ensure the thread is running.
    pub fn set(visible: bool, rects: Vec<(i32, i32, i32, i32)>, lines: CenterLines) {
        STARTED.get_or_init(|| {
            std::thread::spawn(run_overlay_thread);
        });
        if let Ok(mut s) = STATE.lock() {
            s.visible = visible;
            s.rects = rects;
        }
        if let Ok(mut c) = CL.lock() {
            *c = lines;
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_TIMER => {
                apply_state(hwnd);
                LRESULT(0)
            }
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    fn bounding_box(rects: &[(i32, i32, i32, i32)]) -> Option<(i32, i32, i32, i32)> {
        if rects.is_empty() {
            return None;
        }
        let min_x = rects.iter().map(|r| r.0).min().unwrap();
        let min_y = rects.iter().map(|r| r.1).min().unwrap();
        let max_x = rects.iter().map(|r| r.0 + r.2).max().unwrap();
        let max_y = rects.iter().map(|r| r.1 + r.3).max().unwrap();
        Some((min_x, min_y, (max_x - min_x).max(1), (max_y - min_y).max(1)))
    }

    /// (Re)create the DIB section sized to (w, h). Returns the bits pointer.
    unsafe fn ensure_dib(w: i32, h: i32) -> *mut u32 {
        {
            let dim = *DIM.lock().unwrap();
            if dim == (w, h) {
                return *BITS.lock().unwrap() as *mut u32;
            }
        }
        // Destroy old.
        {
            let old_dc = *MEM_DC.lock().unwrap();
            let old_bmp = *DIB_BMP.lock().unwrap();
            if old_bmp != 0 {
                let _ = DeleteObject(HGDIOBJ(old_bmp as *mut c_void));
            }
            if old_dc != 0 {
                let _ = DeleteDC(HDC(old_dc as *mut c_void));
            }
        }
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: core::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0, // BI_RGB
                ..Default::default()
            },
            ..Default::default()
        };
        let mem_dc = CreateCompatibleDC(HDC::default());
        let mut bits_ptr: *mut c_void = core::ptr::null_mut();
        let dib = match CreateDIBSection(HDC::default(), &bmi, DIB_RGB_COLORS, &mut bits_ptr, HANDLE::default(), 0) {
            Ok(b) => b,
            Err(_) => return core::ptr::null_mut(),
        };
        SelectObject(mem_dc, HGDIOBJ(dib.0));
        *MEM_DC.lock().unwrap() = mem_dc.0 as isize;
        *DIB_BMP.lock().unwrap() = dib.0 as isize;
        *BITS.lock().unwrap() = bits_ptr as usize;
        *DIM.lock().unwrap() = (w, h);
        bits_ptr as *mut u32
    }

    unsafe fn fill(buf: &mut [u32], w: i32, h: i32, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32, val: u32) {
        x0 = x0.max(0);
        y0 = y0.max(0);
        x1 = x1.min(w);
        y1 = y1.min(h);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        for y in y0..y1 {
            let s = (y * w + x0) as usize;
            let e = (y * w + x1) as usize;
            buf[s..e].fill(val);
        }
    }

    /// Compose the per-pixel-alpha buffer (frames + center-line bands) and push
    /// it to the layered window at `bbox`.
    unsafe fn compose(hwnd: HWND, bbox: (i32, i32, i32, i32), rects_rel: &[(i32, i32, i32, i32)], cl: CenterLines) {
        let (bx, by, w, h) = bbox;
        let bits = ensure_dib(w, h);
        if bits.is_null() {
            return;
        }
        let n = (w * h) as usize;
        let dst = core::slice::from_raw_parts_mut(bits, n);
        dst.fill(0); // fully transparent

        // Center-line bands inside the first region (drawn under the frame).
        if cl.any() {
            if let Some(&(rx, ry, rw, rh)) = rects_rel.first() {
                let mut bands: Vec<(i32, i32)> = Vec::new();
                if cl.l1 {
                    let cx = rw / 2;
                    bands.push((cx - cl.margin / 2, cx + cl.margin / 2));
                }
                if cl.l2 {
                    bands.push((cl.l2_start, cl.l2_start + cl.margin));
                }
                if cl.l3 {
                    let x = (rw as f32 * cl.l3_ratio) as i32;
                    bands.push((x, x + cl.margin));
                }
                for (bx0, bx1) in bands {
                    let x0 = (rx + bx0).clamp(rx, rx + rw);
                    let x1 = (rx + bx1).clamp(rx, rx + rw);
                    fill(dst, w, h, x0, ry, x1, ry + rh, BAND_FILL);
                    fill(dst, w, h, x0, ry, x0 + 2, ry + rh, BAND_EDGE);
                    fill(dst, w, h, x1 - 2, ry, x1, ry + rh, BAND_EDGE);
                }
            }
        }

        // Accent frame around each region (on top).
        for &(x, y, rw, rh) in rects_rel {
            let left = x;
            let top = y;
            let right = x + rw;
            let bottom = y + rh;
            fill(dst, w, h, left, top, right, top + BORDER, ACCENT);
            fill(dst, w, h, left, bottom - BORDER, right, bottom, ACCENT);
            fill(dst, w, h, left, top, left + BORDER, bottom, ACCENT);
            fill(dst, w, h, right - BORDER, top, right, bottom, ACCENT);
        }

        let mem_dc = HDC(*MEM_DC.lock().unwrap() as *mut c_void);
        let ppt_dst = POINT { x: bx, y: by };
        let psize = SIZE { cx: w, cy: h };
        let ppt_src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: 0,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1, // AC_SRC_ALPHA
        };
        let _ = UpdateLayeredWindow(
            hwnd,
            HDC::default(),
            Some(&ppt_dst),
            Some(&psize),
            mem_dc,
            Some(&ppt_src),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, bx, by, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }

    unsafe fn apply_state(hwnd: HWND) {
        let (visible, fallback) = match STATE.lock() {
            Ok(s) => (s.visible, s.rects.clone()),
            Err(_) => return,
        };

        if !visible {
            if let Ok(mut last) = LAST_APPLIED.lock() {
                if last.is_some() {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                    *last = None;
                }
            }
            return;
        }

        let mut rects = crate::capture_wgc::overlay_rects();
        if rects.is_empty() {
            rects = fallback;
        }
        let Some(bbox) = bounding_box(&rects) else {
            return;
        };
        let cl = CL.lock().map(|c| *c).unwrap_or(CenterLines::NONE);

        let mut last = match LAST_APPLIED.lock() {
            Ok(l) => l,
            Err(_) => return,
        };

        if last.as_deref() != Some(rects.as_slice()) {
            let rel: Vec<(i32, i32, i32, i32)> = rects
                .iter()
                .map(|r| (r.0 - bbox.0, r.1 - bbox.1, r.2, r.3))
                .collect();
            compose(hwnd, bbox, &rel, cl);
            *last = Some(rects);
        } else {
            // Unchanged: re-assert topmost so the frames stay above the game.
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }

    fn run_overlay_thread() {
        unsafe {
            let hinst = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(_) => return,
            };

            let class_name = w!("GameReaderOcrOverlay");
            let hinstance = HINSTANCE(hinst.0);

            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let ex_style = WS_EX_LAYERED
                | WS_EX_TRANSPARENT
                | WS_EX_TOPMOST
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE;

            let hwnd = match CreateWindowExW(
                ex_style,
                class_name,
                PCWSTR::null(),
                WS_POPUP,
                0,
                0,
                100,
                100,
                None,
                None,
                hinstance,
                None,
            ) {
                Ok(h) => h,
                Err(_) => return,
            };

            // Poll the shared state ~10x/sec to apply moves / visibility.
            SetTimer(hwnd, TIMER_ID_VAL, 100, None);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    const TIMER_ID_VAL: usize = 1;
}

/// Show frames around the given OCR capture region rectangles (screen coords),
/// plus the active center-line helpers inside the first region.
#[cfg(windows)]
pub fn show_regions(rects: Vec<(i32, i32, i32, i32)>, lines: CenterLines) {
    imp::set(true, rects, lines);
}

/// Show a single OCR region frame (convenience wrapper).
#[cfg(windows)]
pub fn show_region(x: i32, y: i32, w: i32, h: i32) {
    imp::set(true, vec![(x, y, w, h)], CenterLines::NONE);
}

/// Hide the OCR region frame(s).
#[cfg(windows)]
pub fn hide_region() {
    imp::set(false, Vec::new(), CenterLines::NONE);
}

#[cfg(not(windows))]
pub fn show_regions(_rects: Vec<(i32, i32, i32, i32)>, _lines: CenterLines) {}

#[cfg(not(windows))]
pub fn show_region(_x: i32, _y: i32, _w: i32, _h: i32) {}

#[cfg(not(windows))]
pub fn hide_region() {}
