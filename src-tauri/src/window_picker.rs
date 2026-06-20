//! Native alt+tab-style window picker with LIVE thumbnails.
//!
//! Instead of taking screenshots, this uses the DWM Thumbnail API
//! (`DwmRegisterThumbnail`) — the same mechanism Windows uses for the taskbar
//! and alt+tab previews. DWM composites a live, GPU-scaled preview of each
//! source window into a destination rectangle of our picker window, so there
//! is no per-frame pixel copying in our process.
//!
//! Blocking; runs its own Win32 message loop on a dedicated thread, so wrap it
//! in `spawn_blocking` when calling from async code. Returns the chosen raw
//! HWND (as isize), or `None` on cancel.

#[cfg(windows)]
mod imp {
    use std::sync::Mutex;

    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{
        DwmRegisterThumbnail, DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES,
        DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION, DWM_TNP_VISIBLE,
    };
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint,
        FillRect, GetStockObject, InvalidateRect, SelectObject, SetBkMode, SetTextColor,
        SetWindowRgn, DEFAULT_GUI_FONT, DT_CENTER, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE,
        DT_VCENTER, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::*;

    // Clickable cells: (x, y, w, h, raw_hwnd).
    static CELLS: Mutex<Vec<(i32, i32, i32, i32, isize)>> = Mutex::new(Vec::new());
    static TITLES: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static HOVER: Mutex<i32> = Mutex::new(-1);
    static RESULT: Mutex<Option<isize>> = Mutex::new(None);
    // Scaled layout metrics (set per run): title-strip height and header height.
    static TITLE_STRIP: Mutex<i32> = Mutex::new(30);
    static HEADER_H: Mutex<i32> = Mutex::new(54);

    const BG: COLORREF = COLORREF(0x161616);
    const PANEL: COLORREF = COLORREF(0x242424);
    const PANEL_HOVER: COLORREF = COLORREF(0x2E2A22);
    const ACCENT: COLORREF = COLORREF(0x00FE6A81); // #816afe
    const TEXT: COLORREF = COLORREF(0x00DDDDDD);

    struct Geom {
        panel_w: i32,
        panel_h: i32,
        header_h: i32,
        inner: i32,
        title_h: i32,
        thumb_h: i32,
        cells: Vec<(i32, i32, i32, i32)>, // client coords: x, y, w, h
    }

    /// Centered grid layout with fixed 16:9 thumbnail cells, scaled to fit the
    /// monitor (so it looks right on 16:9 and 21:9 alike, never fullscreen).
    fn compute_layout(n: usize, sw: i32, sh: i32) -> Geom {
        let cols: i32 = match n {
            0..=1 => 1,
            2 => 2,
            3..=4 => 2,
            5..=9 => 3,
            _ => 4,
        };
        let rows = (((n as i32) + cols - 1) / cols).max(1);

        // Base (unscaled) metrics.
        let header = 56.0f32;
        let outer = 24.0;
        let gap = 18.0;
        let inner = 10.0;
        let title = 30.0;
        let thumb_w = 300.0;
        let thumb_h = thumb_w * 9.0 / 16.0;
        let cell_w = thumb_w + 2.0 * inner;
        let cell_h = thumb_h + title + 2.0 * inner;

        let panel_w = 2.0 * outer + cols as f32 * cell_w + (cols as f32 - 1.0) * gap;
        let panel_h = header + outer + rows as f32 * cell_h + (rows as f32 - 1.0) * gap + outer;

        let scale = (0.92 * sw as f32 / panel_w)
            .min(0.92 * sh as f32 / panel_h)
            .min(1.0)
            .max(0.4);
        let s = |v: f32| (v * scale).round() as i32;

        let header_h = s(header);
        let outer_s = s(outer);
        let gap_s = s(gap);
        let inner_s = s(inner);
        let title_s = s(title);
        let thumb_h_s = s(thumb_h);
        let cell_w_s = s(cell_w);
        let cell_h_s = s(cell_h);

        let panel_w_s = 2 * outer_s + cols * cell_w_s + (cols - 1) * gap_s;
        let panel_h_s = header_h + outer_s + rows * cell_h_s + (rows - 1) * gap_s + outer_s;

        let mut cells = Vec::with_capacity(n);
        for i in 0..n as i32 {
            let col = i % cols;
            let row = i / cols;
            let x = outer_s + col * (cell_w_s + gap_s);
            let y = header_h + row * (cell_h_s + gap_s);
            cells.push((x, y, cell_w_s, cell_h_s));
        }

        Geom {
            panel_w: panel_w_s,
            panel_h: panel_h_s,
            header_h,
            inner: inner_s,
            title_h: title_s,
            thumb_h: thumb_h_s,
            cells,
        }
    }

    #[inline]
    fn lo(lp: LPARAM) -> i32 {
        (lp.0 & 0xFFFF) as u16 as i16 as i32
    }
    #[inline]
    fn hi(lp: LPARAM) -> i32 {
        ((lp.0 >> 16) & 0xFFFF) as u16 as i16 as i32
    }

    fn cell_at(x: i32, y: i32) -> i32 {
        let cells = CELLS.lock().unwrap();
        for (i, c) in cells.iter().enumerate() {
            if x >= c.0 && x < c.0 + c.2 && y >= c.1 && y < c.1 + c.3 {
                return i as i32;
            }
        }
        -1
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_MOUSEMOVE => {
                let idx = cell_at(lo(lp), hi(lp));
                let mut h = HOVER.lock().unwrap();
                if *h != idx {
                    *h = idx;
                    drop(h);
                    let _ = InvalidateRect(hwnd, None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let idx = cell_at(lo(lp), hi(lp));
                if idx >= 0 {
                    if let Some(c) = CELLS.lock().unwrap().get(idx as usize) {
                        *RESULT.lock().unwrap() = Some(c.4);
                    }
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if wp.0 == 0x1B {
                    let _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                let bg = CreateSolidBrush(BG);
                FillRect(hdc, &rc, bg);
                let _ = DeleteObject(HGDIOBJ(bg.0));

                let font = GetStockObject(DEFAULT_GUI_FONT);
                let old_font = SelectObject(hdc, font);
                SetBkMode(hdc, TRANSPARENT);

                let title_strip = *TITLE_STRIP.lock().unwrap();
                let header_h = *HEADER_H.lock().unwrap();

                // Header.
                {
                    let mut htext: Vec<u16> = "Wybierz okno gry".encode_utf16().collect();
                    let mut hr = RECT { left: 18, top: 0, right: rc.right - 18, bottom: header_h };
                    SetTextColor(hdc, TEXT);
                    DrawTextW(hdc, &mut htext, &mut hr, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
                }

                let cells = CELLS.lock().unwrap();
                let titles = TITLES.lock().unwrap();
                let hover = *HOVER.lock().unwrap();

                for (i, c) in cells.iter().enumerate() {
                    let hovered = i as i32 == hover;
                    let panel_rect = RECT { left: c.0, top: c.1, right: c.0 + c.2, bottom: c.1 + c.3 };
                    let pb = CreateSolidBrush(if hovered { PANEL_HOVER } else { PANEL });
                    FillRect(hdc, &panel_rect, pb);
                    let _ = DeleteObject(HGDIOBJ(pb.0));

                    if hovered {
                        let ab = CreateSolidBrush(ACCENT);
                        let b = 2;
                        let bars = [
                            RECT { left: panel_rect.left, top: panel_rect.top, right: panel_rect.right, bottom: panel_rect.top + b },
                            RECT { left: panel_rect.left, top: panel_rect.bottom - b, right: panel_rect.right, bottom: panel_rect.bottom },
                            RECT { left: panel_rect.left, top: panel_rect.top, right: panel_rect.left + b, bottom: panel_rect.bottom },
                            RECT { left: panel_rect.right - b, top: panel_rect.top, right: panel_rect.right, bottom: panel_rect.bottom },
                        ];
                        for bar in &bars {
                            FillRect(hdc, bar, ab);
                        }
                        let _ = DeleteObject(HGDIOBJ(ab.0));
                    }

                    // Title text in the strip below the thumbnail.
                    if let Some(t) = titles.get(i) {
                        let mut text: Vec<u16> = t.encode_utf16().collect();
                        let mut tr = RECT {
                            left: c.0 + 8,
                            top: c.1 + c.3 - title_strip,
                            right: c.0 + c.2 - 8,
                            bottom: c.1 + c.3,
                        };
                        SetTextColor(hdc, TEXT);
                        DrawTextW(
                            hdc,
                            &mut text,
                            &mut tr,
                            DT_SINGLELINE | DT_CENTER | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                        );
                    }
                }

                SelectObject(hdc, old_font);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }

    fn run() -> Option<isize> {
        unsafe {
            *CELLS.lock().unwrap() = Vec::new();
            *TITLES.lock().unwrap() = Vec::new();
            *HOVER.lock().unwrap() = -1;
            *RESULT.lock().unwrap() = None;

            let self_pid = GetCurrentProcessId();

            // Candidate windows: visible, titled, not minimised, not ours.
            let mut wins: Vec<(isize, String)> = Vec::new();
            for (raw, title, proc) in crate::capture_wgc::enumerate_window_handles() {
                let hwnd = HWND(raw as *mut core::ffi::c_void);
                if IsIconic(hwnd).as_bool() {
                    continue;
                }
                let mut pid = 0u32;
                let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid == self_pid {
                    continue;
                }
                let mut cr = RECT::default();
                if GetClientRect(hwnd, &mut cr).is_err() {
                    continue;
                }
                if (cr.right - cr.left) < 120 || (cr.bottom - cr.top) < 80 {
                    continue;
                }
                let label = if !proc.is_empty() {
                    format!("{}  ·  {}", title, proc)
                } else {
                    title
                };
                wins.push((raw, label));
            }

            if wins.is_empty() {
                return None;
            }

            let hinst = GetModuleHandleW(None).ok()?;
            let hinstance = HINSTANCE(hinst.0);
            let class_name = w!("GameReaderWindowPicker");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            RegisterClassW(&wc);

            // Lay out a centered panel on the primary monitor.
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let geom = compute_layout(wins.len(), sw, sh);

            *TITLE_STRIP.lock().unwrap() = geom.title_h;
            *HEADER_H.lock().unwrap() = geom.header_h;

            let mut cells: Vec<(i32, i32, i32, i32, isize)> = Vec::with_capacity(wins.len());
            let mut titles: Vec<String> = Vec::with_capacity(wins.len());
            for (i, (raw, label)) in wins.iter().enumerate() {
                let (cx, cy, cw, ch) = geom.cells[i];
                cells.push((cx, cy, cw, ch, *raw));
                titles.push(label.clone());
            }
            *CELLS.lock().unwrap() = cells.clone();
            *TITLES.lock().unwrap() = titles;

            // Center the panel.
            let px = ((sw - geom.panel_w) / 2).max(0);
            let py = ((sh - geom.panel_h) / 2).max(0);

            let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
            let hwnd = CreateWindowExW(
                ex_style,
                class_name,
                w!("Wybierz okno gry"),
                WS_POPUP,
                px,
                py,
                geom.panel_w,
                geom.panel_h,
                None,
                None,
                hinstance,
                None,
            )
            .ok()?;

            // Rounded corners.
            let rgn = CreateRoundRectRgn(0, 0, geom.panel_w + 1, geom.panel_h + 1, 20, 20);
            let _ = SetWindowRgn(hwnd, rgn, true);

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, px, py, geom.panel_w, geom.panel_h, SWP_SHOWWINDOW | SWP_NOACTIVATE);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(hwnd);

            // Register a live DWM thumbnail for each window, scaled into the
            // upper part of its cell (the strip below holds the title).
            for c in &cells {
                let src = HWND(c.4 as *mut core::ffi::c_void);
                if let Ok(thumb) = DwmRegisterThumbnail(hwnd, src) {
                    let dest = RECT {
                        left: c.0 + geom.inner,
                        top: c.1 + geom.inner,
                        right: c.0 + c.2 - geom.inner,
                        bottom: c.1 + geom.inner + geom.thumb_h,
                    };
                    let props = DWM_THUMBNAIL_PROPERTIES {
                        dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY,
                        rcDestination: dest,
                        opacity: 255,
                        fVisible: true.into(),
                        ..Default::default()
                    };
                    let _ = DwmUpdateThumbnailProperties(thumb, &props);
                    // Thumbnails are auto-released when the picker window dies.
                }
            }

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            RESULT.lock().ok().and_then(|r| *r)
        }
    }

    pub fn pick_window_blocking() -> Option<isize> {
        std::thread::spawn(run).join().ok().flatten()
    }
}

/// Show the native window picker (live DWM thumbnails) and return the chosen
/// raw HWND as isize, or `None` if cancelled.
#[cfg(windows)]
pub fn pick_window() -> Option<isize> {
    imp::pick_window_blocking()
}

#[cfg(not(windows))]
pub fn pick_window() -> Option<isize> {
    None
}
