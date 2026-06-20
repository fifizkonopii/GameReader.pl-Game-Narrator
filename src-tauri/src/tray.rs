use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Emitter};
use tauri::tray::{TrayIcon, TrayIconBuilder, MouseButton, TrayIconEvent};

use crate::commands::AppContext;

const DEBOUNCE_MS: u64 = 300;

// 🔥 Komenda obsługująca kliknięcia w menu podręcznym traya
#[tauri::command]
pub fn handle_tray_action(app: AppHandle, akcja: String) {
    match akcja.as_str() {
        "pokaz_ustawienia" => {
            // Szukamy głównego okna "main" z tauri.conf.json
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.show();
                let _ = main_window.unminimize();
                let _ = main_window.set_focus();
            }
            // Po wybraniu opcji chowamy małe menu traya
            if let Some(tray_window) = app.get_webview_window("tray_menu") {
                let _ = tray_window.hide();
            }
        }
        "skroty" => {
            // Otwieramy okno konsoli debugowania / skrótów
            if let Some(debug_window) = app.get_webview_window("debug") {
                let _ = debug_window.show();
                let _ = debug_window.unminimize();
                let _ = debug_window.set_focus();
            }
            if let Some(tray_window) = app.get_webview_window("tray_menu") {
                let _ = tray_window.hide();
            }
        }
        "wybierz_obszar" => {
            // Emitujemy zdarzenie globalne do aplikacji - frontend je złapie i odpali tryb wyboru
            let _ = app.emit("zmien_obszar", ());
            if let Some(tray_window) = app.get_webview_window("tray_menu") {
                let _ = tray_window.hide();
            }
        }
        "pauzuj_ocr" => {
            // Wysyłamy zdarzenie pauzy do głównego procesu OCR
            let _ = app.emit("pauzuj_ocr", ());
        }
        "zamknij_aplikacje" => {
            // Całkowite bezpieczne wyjście z aplikacji
            app.exit(0);
        }
        _ => {}
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let _tray_window = WebviewWindowBuilder::new(
        app,
        "tray_menu",
        WebviewUrl::App("tray_menu.html".into())
    )
    .title("GameReader Menu")
    .decorations(false)
    .transparent(false)
    .always_on_top(true)
    .resizable(false)
    .inner_size(250.0, 350.0)
    .visible(false)
    .skip_taskbar(true)
    .build()?;

    // 🎯 AUTOMATYCZNE UKRYWANIE (Gdy gracz kliknie gdziekolwiek indziej, np. w grę)
    let tray_win_clone = _tray_window.clone();
    _tray_window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(focused) = event {
            if !*focused {
                let _ = tray_win_clone.hide();
            }
        }
    });

    let last_toggle = Arc::new(AtomicU64::new(0));

    let tray = TrayIconBuilder::new()
        .tooltip("GameReader")
        .icon(app.default_window_icon().unwrap().clone())
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click { button, position, .. } = event {
                if button == MouseButton::Left || button == MouseButton::Right {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    let last = last_toggle.load(Ordering::SeqCst);
                    if now.saturating_sub(last) < DEBOUNCE_MS {
                        return;
                    }
                    last_toggle.store(now, Ordering::SeqCst);

                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("tray_menu") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let monitor = app.primary_monitor().ok().flatten();
                            let scale_factor = monitor.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);

                            let window_width = 250.0;
                            let window_height = 350.0;

                            let tray_x = position.x / scale_factor;
                            let tray_y = position.y / scale_factor;

                            let monitor_size = monitor.as_ref().map(|m| {
                                let size = m.size();
                                (size.width as f64 / scale_factor, size.height as f64 / scale_factor)
                            }).unwrap_or((1920.0, 1080.0));

                            let mut x = tray_x - window_width / 2.0;
                            let mut y = tray_y - window_height - 8.0;

                            if x < 8.0 { x = 8.0; }
                            else if x + window_width > monitor_size.0 - 8.0 {
                                x = monitor_size.0 - window_width - 8.0;
                            }
                            if y < 8.0 { y = tray_y + 32.0; }

                            let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                            let _ = window.show();
                            let _ = window.set_focus();
                            
                            // Read preset name directly from state and update tray HTML
                            if let Some(ctx) = app.try_state::<Arc<AppContext>>() {
                                let state = ctx.state.get_runtime_snapshot();
                                let name = if state.preset_filename.is_empty() {
                                    "Brak presetu".to_string()
                                } else {
                                    state.preset_filename
                                };
                                let escaped = name.replace('\'', "\\'");
                                let _ = window.eval(&format!(
                                    "document.getElementById('preset-name').textContent = '{}';",
                                    escaped
                                ));
                            } else {
                                let _ = window.eval("document.getElementById('preset-name').textContent = 'Błąd: brak AppContext';");
                            }
                        }
                    }
                }
            }
        })
        .build(app)?;

    Ok(tray)
}