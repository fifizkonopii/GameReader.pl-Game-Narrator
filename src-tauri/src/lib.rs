// Modules
pub mod constants;
pub mod config;
pub mod logging;
pub mod state;
pub mod preset;
pub mod validation;
pub mod ocr;
pub mod ocr_engine;
pub mod capture;
pub mod capture_wgc;
pub mod capture_dxgi;
pub mod frame_diff;
pub mod preprocessing;
pub mod text_grouping;
pub mod text_filters;
pub mod matcher;
pub mod dedup;
pub mod audio;
pub mod time_stretch;
pub mod ducking;
pub mod region_overlay;
pub mod hud_overlay;
pub mod region_selector;
pub mod window_picker;
pub mod scaling;
pub mod region;
pub mod pipeline;
pub mod single_instance;
pub mod process_priority;
pub mod commands;
pub mod tray;
pub mod hotkeys;
pub mod system_sounds;

use std::sync::Arc;
use std::path::PathBuf;
use tauri::{Manager, Emitter};
use state::AppState;
use preset::PresetManager;
use region::RegionManager;
use pipeline::Pipeline;
use commands::AppContext;
use ocr_engine::create_ocr_engine;
use config::AppConfig;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Check for single instance before initializing anything else
    if let Err(e) = single_instance::check_single_instance() {
        // Another instance is running - show error and exit
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    
    // Set low process priority (below normal) to avoid interfering with game performance
    if let Err(e) = process_priority::set_low_priority() {
        // Log warning but continue - not fatal if this fails
        eprintln!("Warning: Failed to set low priority: {}", e);
    }
    
    // Limit the number of threads used by the OCR backend (MNN/OpenMP).
    // The MNN inference thread count is set explicitly via InferenceConfig
    // (constants::OCR_THREAD_COUNT). These env vars cap OpenMP-parallelized
    // helper code; keep them in sync with the inference thread count so OCR
    // isn't throttled (lower values increase OCR latency / lag).
    if std::env::var_os("OMP_NUM_THREADS").is_none() {
        std::env::set_var("OMP_NUM_THREADS", "4");
    }
    if std::env::var_os("MNN_NUM_THREADS").is_none() {
        std::env::set_var("MNN_NUM_THREADS", "4");
    }
    
    // Initialize OCR engine (Requirement 4.1: Initialize OCR engine once at startup)
    tracing::info!("Initializing OCR engine...");
    let mut ocr_engine = create_ocr_engine();
    
    // Try to initialize OCR engine with model paths
    // For now, we use placeholder paths since actual models are TODO
    // When rust-paddle-ocr is integrated, replace with actual model paths
    let models_dir = PathBuf::from("models"); // TODO: Use app_dir/models or resource dir
    let det_model = models_dir.join("det.mnn");
    let rec_model = models_dir.join("rec.mnn");
    let keys_file = models_dir.join("keys.txt");
    
    match ocr_engine.init(&det_model, &rec_model, &keys_file) {
        Ok(()) => {
            tracing::info!("OCR engine initialized successfully");
        }
        Err(e) => {
            // Log warning but continue - OCR engine can be initialized later
            tracing::warn!("Failed to initialize OCR engine: {:?}", e);
            tracing::warn!("OCR functionality will be limited until models are provided");
        }
    }
    
    // Wrap OCR engine for thread-safe access
    let ocr_engine = Arc::new(parking_lot::Mutex::new(ocr_engine));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Get app data directory
            let app_dir = app.path().app_data_dir()
                .expect("Failed to get app data directory");
            
            // Create directory if it doesn't exist
            std::fs::create_dir_all(&app_dir)
                .expect("Failed to create app data directory");
            
            // Initialize logging
            let log_receivers = logging::init_logging(app_dir.clone())
                .expect("Failed to initialize logging");

            // Forward log lines to the in-app debug console (mirrors cmd output).
            {
                use tauri::Emitter;
                let log_handle = app.handle().clone();
                let logging::LogReceivers { mut raw, mut user } = log_receivers;
                // Raw / technical stream.
                let raw_handle = log_handle.clone();
                tauri::async_runtime::spawn(async move {
                    while let Some(line) = raw.recv().await {
                        if let Some(win) = raw_handle.get_webview_window("debug") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = raw_handle.emit("log_line", line);
                            }
                        }
                    }
                });
                // User-friendly stream.
                tauri::async_runtime::spawn(async move {
                    while let Some(line) = user.recv().await {
                        if let Some(win) = log_handle.get_webview_window("debug") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = log_handle.emit("user_log", line);
                            }
                        }
                    }
                });
            }
            
            tracing::info!(
                "Starting {} v{} {}",
                constants::APP_NAME,
                constants::APP_VERSION,
                constants::APP_VERSION_TAG
            );
            
            // Initialize preset manager
            let preset_manager = PresetManager::new(app_dir.clone());
            
            // Load configuration (Requirement 20.2: Load last preset or defaults at startup)
            let (config, preset_name) = load_startup_config(&app_dir, &preset_manager);

            // Ensure the audio prefs file exists from the first run, so the
            // persisted audio settings are visible and saved going forward.
            {
                let prefs_path = app_dir.join("audio_prefs.json");
                if !prefs_path.exists() {
                    let prefs = serde_json::json!({
                        "READER_VOLUME": config.reader_volume,
                        "VOLUME_REDUCTION_LEVEL": config.volume_reduction_level,
                        "BASE_PLAYBACK_SPEED": config.base_playback_speed,
                        "OVERLAP_PLAYBACK_SPEED": config.overlap_playback_speed,
                    });
                    if let Ok(s) = serde_json::to_string_pretty(&prefs) {
                        let _ = std::fs::write(&prefs_path, s);
                    }
                }
            }
            
            // Initialize app state with loaded configuration
            let app_state = Arc::new(AppState::with_config(config.clone()));
            
            // Set preset filename if preset was loaded
            if let Some(preset_name) = preset_name {
                let default_preset_path = app_dir.join("default.json");
                app_state.update_runtime(|state| {
                    state.preset_filename = preset_name.clone();
                    state.preset_path = default_preset_path.to_string_lossy().to_string();
                });
                
                // Emit preset_loaded event for tray menu
                let payload = crate::commands::PresetLoadedEvent {
                    path: default_preset_path.to_string_lossy().to_string(),
                    name: preset_name,
                };
                let _ = app.handle().emit("preset_loaded", payload);
            }
            
            // Initialize region manager
            let region_manager = Arc::new(parking_lot::Mutex::new(RegionManager::new(config.monitor2_enabled)));
            
            // Create Pipeline instance with audio parameters
            let pipeline = Arc::new(tokio::sync::Mutex::new(Pipeline::new(
                Arc::clone(&app_state),
                Arc::clone(&region_manager),
                config.audio_queue_size as usize, // Use config value (convert u8 to usize)
                config.enable_dynamic_speed,      // Use config value
                config.base_playback_speed,       // Use config value
                config.overlap_playback_speed,    // Use config value
                config.volume_reduction_level,    // Use config value
            )            ));

            // Initialize system tray
            let tray = tray::setup_tray(&app.handle())
                .expect("Failed to initialize system tray");

            // Store tray handle in Arc<Mutex<Option<TrayIcon>>>
            let tray_handle = Arc::new(parking_lot::Mutex::new(Some(tray)));

            // Initialize hotkey manager
            let hotkey_manager = match hotkeys::HotkeyManager::new(config.key_bindings.clone()) {
                Ok(mgr) => {
                    tracing::info!("Hotkey manager initialized successfully");
                    Some(Arc::new(mgr))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize hotkey manager: {}", e);
                    None
                }
            };
            
            // Create AppContext struct with state, pipeline, region_manager, ocr_engine, tray_handle, hotkey_manager, and app_handle
            let app_context = Arc::new(AppContext {
                state: Arc::clone(&app_state),
                pipeline,
                region_manager,
                ocr_engine: Arc::clone(&ocr_engine),
                app_handle: app.handle().clone(),
                tray_handle,
                hotkey_manager: hotkey_manager.clone(),
            });
            
            // Set up hotkey callback if hotkey manager is available
            if let Some(hotkey_mgr) = &hotkey_manager {
                let context_for_hotkeys = Arc::clone(&app_context);
                hotkey_mgr.set_action_callback(move |action| {
                    commands::handle_hotkey_action(&context_for_hotkeys, action);
                });
                
                // Start the keyboard hook
                #[cfg(windows)]
                if let Err(e) = hotkey_mgr.start() {
                    tracing::error!("Failed to start hotkey manager: {}", e);
                } else {
                    tracing::info!("Hotkey system started successfully");
                }
                
                #[cfg(not(windows))]
                tracing::warn!("Hotkey system is only supported on Windows");
            }
            
            // Make the debug window's close button hide it instead of
            // destroying it, and keep the debug flag in sync.
            if let Some(debug_win) = app.get_webview_window("debug") {
                let _ = debug_win.hide();
                let state_for_debug = Arc::clone(&app_state);
                let win = debug_win.clone();
                debug_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                        state_for_debug.update_runtime(|runtime| runtime.debug_enabled = false);
                    }
                });
            }

            // Check recent presets before moving the manager into app state.
            let has_recent = preset_manager
                .get_recent()
                .map(|r| !r.is_empty())
                .unwrap_or(false);

            // Store managers in app state BEFORE showing any window, so the
            // launcher's get_recent_presets call (fired as soon as it loads)
            // finds the managed PresetManager instead of racing it.
            app.manage(app_state.clone());
            app.manage(preset_manager);
            app.manage(app_context);

            // Decide the startup window: show the launcher (recent-presets
            // picker) if there are recent presets; otherwise go straight to the
            // main window. Both are defined hidden in tauri.conf.json.
            {
                if has_recent {
                    if let Some(launcher) = app.get_webview_window("launcher") {
                        let _ = launcher.show();
                        let _ = launcher.set_focus();
                    } else if let Some(main) = app.get_webview_window("main") {
                        let _ = main.show();
                    }
                } else if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
            
            // Start tracking the last non-self foreground window so an empty
            // window query can auto-target the game the user was focused on.
            #[cfg(windows)]
            capture_wgc::start_foreground_tracker();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::enable_reader,
            commands::disable_reader,
            commands::toggle_reader,
            commands::interrupt_audio,
            commands::skip_next_line,
            commands::switch_active_region,
            commands::set_region_overlay,
            commands::load_preset,
            commands::save_preset,
            commands::get_recent_presets,
            commands::remove_recent_preset,
            commands::clear_recent_presets,
            commands::get_current_preset,
            commands::tray_action,
            commands::finish_launch,
            commands::update_config,
            commands::get_config,
            commands::get_runtime_state,
            commands::list_windows,
            commands::list_monitors,
            commands::pick_window,
            commands::clear_pinned_window,
            commands::detect_window_resolution,
            commands::select_screen_region,
            commands::trigger_hotkey,
            commands::get_recent_logs,
            commands::get_recent_user_logs,
            commands::play_sound,
            commands::export_logs,
            commands::close_debug_console,
            commands::exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Load configuration at startup
/// 
/// This function implements Requirement 20.2: Load last preset or defaults at startup
/// 
/// Priority:
/// 1. If a "default.json" preset exists in app_dir, load it
/// 2. Otherwise, use AppConfig::default()
/// 
/// Returns: (AppConfig, Option<preset_name>)
fn load_startup_config(app_dir: &PathBuf, preset_manager: &PresetManager) -> (AppConfig, Option<String>) {
    let default_preset_path = app_dir.join("default.json");
    
    let (mut config, preset_name) = if default_preset_path.exists() {
        tracing::info!("Loading default preset from: {}", default_preset_path.display());
        match preset_manager.load(&default_preset_path) {
            Ok(config) => {
                tracing::info!("Default preset loaded successfully");
                let name = default_preset_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("default")
                    .to_string();
                (config, Some(name))
            }
            Err(e) => {
                tracing::warn!("Failed to load default preset: {}. Using defaults.", e);
                (AppConfig::default(), None)
            }
        }
    } else {
        tracing::info!("No default preset found. Using default configuration.");
        (AppConfig::default(), None)
    };

    // Overlay persisted audio prefs (reader volume, game ducking, playback
    // speeds) saved automatically on change.
    apply_audio_prefs(app_dir, &mut config);
    (config, preset_name)
}

/// Overlay the small audio_prefs.json (auto-saved on change) onto a config.
fn apply_audio_prefs(app_dir: &PathBuf, config: &mut AppConfig) {
    let path = app_dir.join("audio_prefs.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return; };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return; };
    if let Some(x) = v.get("READER_VOLUME").and_then(|x| x.as_f64()) {
        config.reader_volume = (x as f32).clamp(0.0, 1.0);
    }
    if let Some(x) = v.get("VOLUME_REDUCTION_LEVEL").and_then(|x| x.as_f64()) {
        config.volume_reduction_level = (x as f32).clamp(0.0, 1.0);
    }
    if let Some(x) = v.get("BASE_PLAYBACK_SPEED").and_then(|x| x.as_f64()) {
        config.base_playback_speed = x as f32;
    }
    if let Some(x) = v.get("OVERLAP_PLAYBACK_SPEED").and_then(|x| x.as_f64()) {
        config.overlap_playback_speed = x as f32;
    }
    tracing::info!("Applied saved audio prefs from {}", path.display());
}
