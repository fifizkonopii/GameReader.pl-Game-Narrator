//! Native Win32 screen-region selector — a focused "snipping tool".
//!
//! Unlike Win+Shift+S (which dims everything uniformly), this blacks out the
//! whole screen EXCEPT the target game window, so only the game is visible
//! while the user drags out the OCR region. It uses a per-pixel-alpha layered
//! window (`UpdateLayeredWindow`) so the game area can stay visible AND still
//! capture mouse input (areas with alpha > 0 are not click-through).
//!
//! `select_region()` is blocking and runs its own Win32 message loop on a
//! dedicated thread, so it must be wrapped in `spawn_blocking` when called from
//! an async context.

#[cfg(windows)]
mod imp {
    use core::ffi::c_void;
    use std::sync::Mutex;

    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BLENDFUNCTION, DIB_RGB_COLORS, HDC, HGDIOBJ,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, SetFocus};
    use windows::Win32::UI::WindowsAndMessaging::*;

    #[derive(Clone, Copy)]
    struct DragState {
        dragging: bool,
        active: bool,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
    }

    const EMPTY_DRAG: DragState = DragState {
        dragging: false,
        active: false,
        x0: 0,
        y0: 0,
        x1: 0,
        y1: 0,
    };

    static DRAG: Mutex<DragState> = Mutex::new(EMPTY_DRAG);
    // Selected rectangle in SCREEN pixels (x, y, w, h), or None if cancelled.
    static RESULT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);
    // Virtual-screen origin so client coords map to screen coords.
    static ORIGIN: Mutex<(i32, i32)> = Mutex::new((0, 0));
    // Target game window rect in SCREEN pixels (x, y, w, h); None = whole screen.
    static GAME_RECT: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);
    // DIB pixel buffer pointer (as usize), DIB size, and memory DC (as isize).
    static BITS: Mutex<usize> = Mutex::new(0);
    static DIMS: Mutex<(i32, i32)> = Mutex::new((0, 0));
    static MEM_DC: Mutex<isize> = Mutex::new(0);
    // Precomputed static background (black outside + dimmed game), built once.
    static BASE: Mutex<Vec<u32>> = Mutex::new(Vec::new());

    const BORDER: i32 = 2;

    // Premultiplied BGRA packed as little-endian u32: (A<<24)|(R<<16)|(G<<8)|B.
    // Black with alpha => premultiplied RGB stays 0, so just (alpha<<24).
    const OUT_FILL: u32 = 236 << 24; // outside the game: near-opaque black
    const GAME_FILL: u32 = 60 << 24; // over the game: lightly dimmed, visible
    const SEL_FILL: u32 = 0; // selection interior: fully clear
    // Accent #816afe (R=0x81, G=0x6A, B=0xFE) at full alpha.
    const BORDER_FILL: u32 = (0xFFu32 << 24) | (0x81 << 16) | (0x6A << 8) | 0xFE;

    #[inline]
    fn lo_word(lp: LPARAM) -> i32 {
        (lp.0 & 0xFFFF) as u16 as i16 as i32
    }
    #[inline]
    fn hi_word(lp: LPARAM) -> i32 {
        ((lp.0 >> 16) & 0xFFFF) as u16 as i16 as i32
    }

    /// Fast row-based fill of a sub-rectangle in a flat w*h pixel buffer.
    fn fill_slice(buf: &mut [u32], w: i32, h: i32, mut x0: i32, mut y0: i32, mut x1: i32, mut y1: i32, val: u32) {
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

    /// Build the static background once: near-opaque black everywhere, with the
    /// game window's client area only lightly dimmed (so it stays visible).
    fn build_base() {
        let (w, h) = *DIMS.lock().unwrap();
        let (ox, oy) = *ORIGIN.lock().unwrap();
        let n = (w.max(0) * h.max(0)) as usize;
        let mut base = vec![OUT_FILL; n];
        let (gx0, gy0, gx1, gy1) = match *GAME_RECT.lock().unwrap() {
            Some((gx, gy, gw, gh)) => (gx - ox, gy - oy, gx - ox + gw, gy - oy + gh),
            None => (0, 0, w, h),
        };
        fill_slice(&mut base, w, h, gx0, gy0, gx1, gy1, GAME_FILL);
        *BASE.lock().unwrap() = base;
    }

    /// Recompose the dimming buffer and push it to the layered window.
    ///
    /// Cheap per frame: a single memcpy of the precomputed background, then the
    /// (small) selection border drawn on top.
    unsafe fn compose_and_update(hwnd: HWND) {
        let bits = *BITS.lock().unwrap() as *mut u32;
        if bits.is_null() {
            return;
        }
        let (w, h) = *DIMS.lock().unwrap();
        let (ox, oy) = *ORIGIN.lock().unwrap();
        let n = (w * h) as usize;
        let dst = core::slice::from_raw_parts_mut(bits, n);

        {
            let base = BASE.lock().unwrap();
            if base.len() == n {
                dst.copy_from_slice(&base);
            } else {
                dst.fill(OUT_FILL);
            }
        }

        let d = *DRAG.lock().unwrap();
        if d.active {
            let sl = d.x0.min(d.x1);
            let st = d.y0.min(d.y1);
            let sr = d.x0.max(d.x1);
            let sb = d.y0.max(d.y1);
            // Clear interior (crisp game) + accent border.
            fill_slice(dst, w, h, sl, st, sr, sb, SEL_FILL);
            fill_slice(dst, w, h, sl, st, sr, st + BORDER, BORDER_FILL);
            fill_slice(dst, w, h, sl, sb - BORDER, sr, sb, BORDER_FILL);
            fill_slice(dst, w, h, sl, st, sl + BORDER, sb, BORDER_FILL);
            fill_slice(dst, w, h, sr - BORDER, st, sr, sb, BORDER_FILL);
        }

        let mem_dc = HDC(*MEM_DC.lock().unwrap() as *mut c_void);
        let ppt_dst = POINT { x: ox, y: oy };
        let psize = SIZE { cx: w, cy: h };
        let ppt_src = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: 0,           // AC_SRC_OVER
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1,       // AC_SRC_ALPHA
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
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_SETCURSOR => {
                if let Ok(cur) = LoadCursorW(None, IDC_CROSS) {
                    SetCursor(cur);
                }
                LRESULT(1)
            }
            WM_LBUTTONDOWN => {
                SetCapture(hwnd);
                if let Ok(mut d) = DRAG.lock() {
                    d.dragging = true;
                    d.active = true;
                    d.x0 = lo_word(lp);
                    d.y0 = hi_word(lp);
                    d.x1 = d.x0;
                    d.y1 = d.y0;
                }
                compose_and_update(hwnd);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let mut redraw = false;
                if let Ok(mut d) = DRAG.lock() {
                    if d.dragging {
                        d.x1 = lo_word(lp);
                        d.y1 = hi_word(lp);
                        redraw = true;
                    }
                }
                if redraw {
                    compose_and_update(hwnd);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let _ = ReleaseCapture();
                let (ox, oy) = ORIGIN.lock().map(|o| *o).unwrap_or((0, 0));
                if let Ok(mut d) = DRAG.lock() {
                    d.dragging = false;
                    let sl = d.x0.min(d.x1);
                    let st = d.y0.min(d.y1);
                    let w = (d.x0.max(d.x1)) - sl;
                    let h = (d.y0.max(d.y1)) - st;
                    if w >= 5 && h >= 5 {
                        if let Ok(mut res) = RESULT.lock() {
                            *res = Some((sl + ox, st + oy, w, h));
                        }
                    }
                }
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_RBUTTONDOWN => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wp.0 == 0x1B {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    fn run() -> Option<(i32, i32, i32, i32)> {
        unsafe {
            if let Ok(mut d) = DRAG.lock() {
                *d = EMPTY_DRAG;
            }
            if let Ok(mut r) = RESULT.lock() {
                *r = None;
            }

            let hinst = GetModuleHandleW(None).ok()?;
            let hinstance = HINSTANCE(hinst.0);
            let class_name = w!("GameReaderRegionSelector");

            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
                ..Default::default()
            };
            RegisterClassW(&wc);

            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if vw <= 0 || vh <= 0 {
                return None;
            }
            *ORIGIN.lock().unwrap() = (vx, vy);
            *DIMS.lock().unwrap() = (vw, vh);

            // Create a top-down 32-bit DIB section for the per-pixel-alpha buffer.
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: core::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: vw,
                    biHeight: -vh, // negative => top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0, // BI_RGB
                    ..Default::default()
                },
                ..Default::default()
            };
            let mem_dc = CreateCompatibleDC(HDC::default());
            let mut bits_ptr: *mut c_void = core::ptr::null_mut();
            let dib = CreateDIBSection(
                HDC::default(),
                &bmi,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                HANDLE::default(),
                0,
            )
            .ok()?;
            let old_bmp = SelectObject(mem_dc, HGDIOBJ(dib.0));

            *BITS.lock().unwrap() = bits_ptr as usize;
            *MEM_DC.lock().unwrap() = mem_dc.0 as isize;

            // Precompute the static background once (memcpy'd each frame).
            build_base();

            // No WS_EX_TRANSPARENT: areas with alpha > 0 must capture the mouse.
            let ex_style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
            let hwnd = CreateWindowExW(
                ex_style,
                class_name,
                w!("Zaznacz obszar"),
                WS_POPUP,
                vx,
                vy,
                vw,
                vh,
                None,
                None,
                hinstance,
                None,
            )
            .ok()?;

            compose_and_update(hwnd);

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, vx, vy, vw, vh, SWP_SHOWWINDOW | SWP_NOACTIVATE);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(hwnd);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Cleanup GDI objects.
            SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(HGDIOBJ(dib.0));
            let _ = DeleteDC(mem_dc);
            *BITS.lock().unwrap() = 0;
            *MEM_DC.lock().unwrap() = 0;

            RESULT.lock().ok().and_then(|r| *r)
        }
    }

    /// Run the selector on its own thread and block until done / cancelled.
    /// `game_rect` is the target window's screen rect (x, y, w, h); the area
    /// outside it is blacked out. Pass `None` to dim the whole screen.
    pub fn select_region_blocking(game_rect: Option<(i32, i32, i32, i32)>) -> Option<(i32, i32, i32, i32)> {
        *GAME_RECT.lock().unwrap() = game_rect;
        std::thread::spawn(run).join().ok().flatten()
    }
}

/// Show the snipping overlay and return the chosen rectangle in physical SCREEN
/// pixels (x, y, w, h), or `None` if cancelled. Everything outside `game_rect`
/// (also screen pixels) is blacked out so only the game is visible.
#[cfg(windows)]
pub fn select_region(game_rect: Option<(i32, i32, i32, i32)>) -> Option<(i32, i32, i32, i32)> {
    imp::select_region_blocking(game_rect)
}

#[cfg(not(windows))]
pub fn select_region(_game_rect: Option<(i32, i32, i32, i32)>) -> Option<(i32, i32, i32, i32)> {
    None
}
