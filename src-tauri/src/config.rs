use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::constants;

/// Core configuration types matching JSON preset format
/// These structures are serialized/deserialized for preset compatibility

// ============================================================
// MONITOR RECT
// ============================================================

/// Capture region geometry in physical pixels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonitorRect {
    pub top: i32,
    pub left: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for MonitorRect {
    fn default() -> Self {
        Self {
            top: 900,
            left: 375,
            width: 1170,
            height: 120,
        }
    }
}

// ============================================================
// KEY BINDINGS
// ============================================================

/// Global keyboard shortcuts mapping action names to key combinations
pub type KeyBindings = HashMap<String, String>;

/// Create default key bindings
pub fn default_key_bindings() -> KeyBindings {
    constants::default_key_bindings()
}

/// Serde default for capture_mode field
fn default_capture_mode() -> String {
    constants::CAPTURE_MODE.to_string()
}

fn default_outline_white() -> u8 {
    constants::OUTLINE_WHITE_THRESHOLD
}

fn default_outline_dark() -> u8 {
    constants::OUTLINE_DARK_THRESHOLD
}

// ============================================================
// APP CONFIG (SERIALIZABLE PRESET)
// ============================================================

/// Complete application configuration (matches Python preset JSON format)
/// This is the main structure saved to/loaded from preset files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Monitor / Capture
    pub monitor: MonitorRect,
    pub resolution: String,
    
    // Monitor 2 (optional second capture area)
    pub monitor2_enabled: bool,
    pub monitor2_top: i32,
    pub monitor2_left: i32,
    pub monitor2_width: u32,
    pub monitor2_height: u32,
    
    // Center Lines
    #[serde(rename = "CENTER_LINE_MARGIN")]
    pub center_line_margin: i32,
    #[serde(rename = "CENTER_LINE_2_START")]
    pub center_line_2_start: i32,
    #[serde(rename = "CENTER_LINE_3_START_RATIO")]
    pub center_line_3_start_ratio: f32,
    
    // OCR Settings
    #[serde(rename = "RESOLUTION_DOWNSCALE")]
    pub resolution_downscale: f32,
    #[serde(rename = "CAPTURE_INTERVAL")]
    pub capture_interval: f32,
    #[serde(rename = "MIN_HEIGHT")]
    pub min_height: i32,
    #[serde(rename = "MAX_HEIGHT")]
    pub max_height: i32,
    #[serde(rename = "OCR_MIN_CONFIDENCE")]
    pub ocr_min_confidence: f32,

    // Capture mode: "region" (GDI screen region) or "window" (WGC window capture)
    #[serde(rename = "CAPTURE_MODE", default = "default_capture_mode")]
    pub capture_mode: String,
    // Window query for "window" mode: process exe name or title substring
    #[serde(rename = "CAPTURE_WINDOW_QUERY", default)]
    pub capture_window_query: String,
    // Preferred physical monitor device id (e.g. "\\\\.\\DISPLAY1"); empty = auto.
    // Used to constrain the region selector overlay on multi-monitor setups.
    #[serde(rename = "CAPTURE_MONITOR", default)]
    pub capture_monitor: String,
    
    // Feature Flags
    #[serde(rename = "ENABLE_REMOVE_CHARACTER_NAME")]
    pub enable_remove_character_name: bool,
    #[serde(rename = "ENABLE_SCREENSHOTS")]
    pub enable_screenshots: bool,
    #[serde(rename = "ENABLE_PARAGRAPH_OCR")]
    pub enable_paragraph_ocr: bool,
    #[serde(rename = "ENABLE_TYPEWRITER_WAIT")]
    pub enable_typewriter_wait: bool,
    #[serde(rename = "ENABLE_REGION_OVERLAY", default)]
    pub enable_region_overlay: bool,
    // White-outlined-subtitle OCR mode (good for bright backgrounds).
    #[serde(rename = "ENABLE_OUTLINE_TEXT_MODE", default)]
    pub enable_outline_text_mode: bool,
    #[serde(rename = "OUTLINE_WHITE_THRESHOLD", default = "default_outline_white")]
    pub outline_white_threshold: u8,
    #[serde(rename = "OUTLINE_DARK_THRESHOLD", default = "default_outline_dark")]
    pub outline_dark_threshold: u8,
    #[serde(rename = "ENABLE_OUTPUT2_SYSTEM")]
    pub enable_output2_system: bool,
    #[serde(rename = "ENABLE_DYNAMIC_SPEED")]
    pub enable_dynamic_speed: bool,
    
    // Audio Settings
    #[serde(rename = "BASE_PLAYBACK_SPEED")]
    pub base_playback_speed: f32,
    #[serde(rename = "OVERLAP_PLAYBACK_SPEED")]
    pub overlap_playback_speed: f32,
    #[serde(rename = "VOLUME_REDUCTION_LEVEL")]
    pub volume_reduction_level: f32,
    // Reader (TTS) playback volume, 0.0–1.0. Controlled by the volume hotkeys.
    #[serde(rename = "READER_VOLUME", default = "default_reader_volume")]
    pub reader_volume: f32,
    // Ducking target process (e.g. "GTA-SA.exe"). If empty, uses capture_window_query.
    #[serde(rename = "DUCKING_TARGET_PROCESS", default)]
    pub ducking_target_process: String,
    #[serde(rename = "AUDIO_QUEUE_SIZE")]
    pub audio_queue_size: u8,
    
    // UI Behavior
    /// Minimize the main window to tray when the reader starts
    #[serde(rename = "MINIMIZE_TO_TRAY_ON_READER_START", default = "default_true")]
    pub minimize_to_tray_on_reader_start: bool,
    
    // Center Line Filters
    #[serde(rename = "USE_CENTER_LINE_1")]
    pub use_center_line_1: bool,
    #[serde(rename = "USE_CENTER_LINE_2")]
    pub use_center_line_2: bool,
    #[serde(rename = "USE_CENTER_LINE_3")]
    pub use_center_line_3: bool,
    
    // User-selected Paths
    pub audio_dir: String,
    pub text_file_path: String,
    pub names_file_path: String,
    pub screenshot_dir: String,
    
    // Key Bindings
    pub key_bindings: KeyBindings,
}

fn default_reader_volume() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            monitor: MonitorRect::default(),
            resolution: "1920x1080".to_string(),
            
            monitor2_enabled: false,
            monitor2_top: 100,
            monitor2_left: 375,
            monitor2_width: 1170,
            monitor2_height: 120,
            
            center_line_margin: constants::CENTER_LINE_MARGIN,
            center_line_2_start: constants::CENTER_LINE_2_START,
            center_line_3_start_ratio: constants::CENTER_LINE_3_START_RATIO,
            
            resolution_downscale: constants::RESOLUTION_DOWNSCALE,
            capture_interval: constants::CAPTURE_INTERVAL,
            min_height: constants::MIN_HEIGHT,
            max_height: constants::MAX_HEIGHT,
            ocr_min_confidence: constants::OCR_MIN_CONFIDENCE,

            capture_mode: constants::CAPTURE_MODE.to_string(),
            capture_window_query: constants::CAPTURE_WINDOW_QUERY.to_string(),
            capture_monitor: String::new(),
            enable_outline_text_mode: constants::ENABLE_OUTLINE_TEXT_MODE,
            outline_white_threshold: constants::OUTLINE_WHITE_THRESHOLD,
            outline_dark_threshold: constants::OUTLINE_DARK_THRESHOLD,
            
            enable_remove_character_name: constants::ENABLE_REMOVE_CHARACTER_NAME,
            enable_screenshots: constants::ENABLE_SCREENSHOTS,
            enable_paragraph_ocr: constants::ENABLE_PARAGRAPH_OCR,
            enable_typewriter_wait: constants::ENABLE_TYPEWRITER_WAIT,
            enable_region_overlay: constants::ENABLE_REGION_OVERLAY,
            enable_output2_system: constants::ENABLE_OUTPUT2_SYSTEM,
            enable_dynamic_speed: constants::ENABLE_DYNAMIC_SPEED,
            
            base_playback_speed: constants::BASE_PLAYBACK_SPEED,
            overlap_playback_speed: constants::OVERLAP_PLAYBACK_SPEED,
            volume_reduction_level: constants::VOLUME_REDUCTION_LEVEL,
            reader_volume: 1.0,
            ducking_target_process: String::new(), // Empty by default, will use capture_window_query
            audio_queue_size: constants::AUDIO_QUEUE_SIZE,
            minimize_to_tray_on_reader_start: true,
            
            use_center_line_1: constants::USE_CENTER_LINE_1,
            use_center_line_2: constants::USE_CENTER_LINE_2,
            use_center_line_3: constants::USE_CENTER_LINE_3,
            
            audio_dir: String::new(),
            text_file_path: String::new(),
            names_file_path: String::new(),
            screenshot_dir: String::new(),
            
            key_bindings: default_key_bindings(),
        }
    }
}

impl AppConfig {
    /// Get the active monitor rect based on monitor2_enabled and active_monitor selection
    pub fn get_active_monitor(&self, active_monitor: u8) -> MonitorRect {
        if self.monitor2_enabled && active_monitor == 2 {
            MonitorRect {
                top: self.monitor2_top,
                left: self.monitor2_left,
                width: self.monitor2_width,
                height: self.monitor2_height,
            }
        } else {
            self.monitor
        }
    }
    
    /// Creates a test configuration with valid values (for unit tests)
    #[cfg(test)]
    pub fn test_valid() -> Self {
        use std::env;
        
        // Create temp dir for test paths
        let temp_dir = env::temp_dir();
        let audio_dir = temp_dir.join("test_audio");
        let _ = std::fs::create_dir_all(&audio_dir);
        
        // Create dummy audio file
        let audio_file = audio_dir.join("test.ogg");
        let _ = std::fs::write(&audio_file, b"dummy");
        
        // Create text file
        let text_file = temp_dir.join("test_lines.txt");
        let _ = std::fs::write(&text_file, "Line 1\nLine 2\nLine 3\n");
        
        let mut config = Self::default();
        config.audio_dir = audio_dir.to_str().unwrap().to_string();
        config.text_file_path = text_file.to_str().unwrap().to_string();
        config.names_file_path = String::new(); // Not required when enable_remove_character_name is false
        config.screenshot_dir = temp_dir.to_str().unwrap().to_string();
        
        // Disable output2_system to avoid needing output2 files
        config.enable_output2_system = false;
        
        config
    }
}

// ============================================================
// RUNTIME STATE (NON-SERIALIZED)
// ============================================================

/// Runtime state that is not saved to presets
/// This contains transient application state
#[derive(Debug, Clone, serde::Serialize)]
pub struct RuntimeState {
    // Reader state
    pub capture_enabled: bool,
    pub active_monitor: u8,         // 1 or 2
    pub selected_screen_monitor: u8, // Qt screen index (1-based)
    
    // Dialog data (loaded at runtime)
    pub dialog_lines: Vec<String>,
    pub character_names: Vec<String>,
    
    // Preset info
    pub preset_filename: String,
    pub preset_path: String,
    
    // Debug state
    pub debug_enabled: bool,
    
    // Audio state
    pub is_audio_playing: bool,
    pub audio_queue_len: usize,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            capture_enabled: false,
            active_monitor: 1,
            selected_screen_monitor: 1,
            dialog_lines: Vec::new(),
            character_names: Vec::new(),
            preset_filename: String::new(),
            preset_path: String::new(),
            debug_enabled: false,
            is_audio_playing: false,
            audio_queue_len: 0,
        }
    }
}

// ============================================================
// RECENT PRESET
// ============================================================

/// Recent preset entry for the recent presets list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPreset {
    pub path: String,
    pub name: String,
    pub timestamp: i64, // Unix timestamp
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_monitor_rect_default() {
        let rect = MonitorRect::default();
        assert_eq!(rect.width, 1170);
        assert_eq!(rect.height, 120);
    }
    
    #[test]
    fn test_app_config_serialization() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("\"monitor\""));
        assert!(json.contains("\"RESOLUTION_DOWNSCALE\""));
    }
    
    #[test]
    fn test_app_config_deserialization() {
        let json = r#"{
            "monitor": {"top": 900, "left": 375, "width": 1170, "height": 120},
            "resolution": "1920x1080",
            "monitor2_enabled": false,
            "monitor2_top": 100,
            "monitor2_left": 375,
            "monitor2_width": 1170,
            "monitor2_height": 120,
            "CENTER_LINE_MARGIN": 100,
            "CENTER_LINE_2_START": 1,
            "CENTER_LINE_3_START_RATIO": 0.3,
            "RESOLUTION_DOWNSCALE": 0.45,
            "CAPTURE_INTERVAL": 0.5,
            "MIN_HEIGHT": 10,
            "MAX_HEIGHT": 100,
            "OCR_MIN_CONFIDENCE": 0.4,
            "ENABLE_REMOVE_CHARACTER_NAME": false,
            "ENABLE_SCREENSHOTS": false,
            "ENABLE_PARAGRAPH_OCR": false,
            "ENABLE_TYPEWRITER_WAIT": false,
            "ENABLE_OUTPUT2_SYSTEM": true,
            "ENABLE_DYNAMIC_SPEED": false,
            "BASE_PLAYBACK_SPEED": 1.0,
            "OVERLAP_PLAYBACK_SPEED": 1.2,
            "VOLUME_REDUCTION_LEVEL": 0.2,
            "AUDIO_QUEUE_SIZE": 1,
            "USE_CENTER_LINE_1": false,
            "USE_CENTER_LINE_2": false,
            "USE_CENTER_LINE_3": false,
            "audio_dir": "",
            "text_file_path": "",
            "names_file_path": "",
            "screenshot_dir": "",
            "key_bindings": {
                "toggle_reader": "home"
            }
        }"#;
        
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.resolution, "1920x1080");
        assert_eq!(config.resolution_downscale, 0.45);
        assert_eq!(config.monitor.width, 1170);
    }
    
    #[test]
    fn test_get_active_monitor() {
        let mut config = AppConfig::default();
        config.monitor2_enabled = true;
        config.monitor2_top = 200;
        config.monitor2_width = 2000;
        
        let mon1 = config.get_active_monitor(1);
        assert_eq!(mon1.top, 900); // default monitor
        
        let mon2 = config.get_active_monitor(2);
        assert_eq!(mon2.top, 200);
        assert_eq!(mon2.width, 2000);
    }
}
