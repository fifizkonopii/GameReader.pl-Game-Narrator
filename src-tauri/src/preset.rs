use std::path::{Path, PathBuf};
use std::fs;
use serde_json;
use anyhow::{Context, Result};
use crate::config::{AppConfig, RecentPreset};
use crate::constants;
use std::collections::{HashMap, HashSet};

/// Normalize a hotkey string to canonical form (lowercase, trimmed, sorted
/// modifiers). Returns None if a modifier is invalid or the string is malformed.
fn normalize_hotkey(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        return None;
    }
    let lower = s.to_lowercase();
    let mut parts: Vec<String> = lower.split('+').map(|p| p.trim().to_string()).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let key = parts.pop().unwrap();
    let mut mods = parts;
    for m in &mods {
        if !constants::ALLOWED_MODIFIERS.contains(&m.as_str()) {
            return None;
        }
    }
    mods.sort();
    mods.dedup();
    let mut out = mods.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&key);
    Some(out)
}

/// Check whether a hotkey string is valid and bindable.
fn is_valid_hotkey(s: &str) -> bool {
    match normalize_hotkey(s) {
        None => false,
        Some(n) => {
            if constants::RESERVED_SYSTEM_HOTKEYS.contains(&n.as_str()) {
                return false;
            }
            let key = n.split('+').last().unwrap();
            constants::ALLOWED_KEYS.contains(&key)
        }
    }
}

/// Sanitize a key-bindings map in place and return a list of human-readable
/// notes describing what was changed. Rules:
/// - Unknown actions are removed.
/// - Invalid / no-longer-allowed hotkeys are reset to their default.
/// - Duplicate hotkeys keep the first action; the rest get unbound.
/// - Any missing default action is added back.
pub fn sanitize_key_bindings(kb: &mut HashMap<String, String>) -> Vec<String> {
    let defaults = crate::config::default_key_bindings();
    let allowed: HashSet<String> = defaults.keys().cloned().collect();
    let mut issues: Vec<String> = Vec::new();

    // Migration: the old default "show/hide settings" shortcut was alt+`
    // (backtick), which is awkward to type on many layouts. Convert it to the
    // new default alt+' so existing presets pick up the change automatically.
    if kb.get("open_settings").map(|v| v.as_str()) == Some("alt+`") {
        kb.insert("open_settings".to_string(), "alt+'".to_string());
        issues.push("zmieniono skrót ustawień: alt+` → alt+'".to_string());
    }

    // 1. Drop actions we don't know about (e.g. removed features).
    let unknown: Vec<String> = kb
        .keys()
        .filter(|k| !allowed.contains(*k))
        .cloned()
        .collect();
    for action in unknown {
        kb.remove(&action);
        issues.push(format!("usunięto nieznany skrót „{}”", action));
    }

    // 2. Validate each known binding; fix invalid ones to the default.
    for (action, def) in &defaults {
        match kb.get(action).cloned() {
            Some(v) if !v.trim().is_empty() => {
                if !is_valid_hotkey(&v) {
                    kb.insert(action.clone(), def.clone());
                    issues.push(format!("naprawiono skrót „{}”: „{}” → „{}”", action, v, def));
                }
            }
            Some(_) => { /* explicitly empty = unbound, leave as is */ }
            None => {
                kb.insert(action.clone(), def.clone());
            }
        }
    }

    // 3. Resolve duplicates: the first action (alphabetical) keeps the key,
    //    every later action sharing it gets unbound.
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut actions: Vec<String> = kb.keys().cloned().collect();
    actions.sort();
    for action in actions {
        let val = kb.get(&action).cloned().unwrap_or_default();
        if val.trim().is_empty() {
            continue;
        }
        let norm = normalize_hotkey(&val).unwrap_or(val.clone());
        if let Some(other) = seen.get(&norm) {
            kb.insert(action.clone(), String::new());
            issues.push(format!(
                "skrót „{}” był taki sam jak „{}” — wyłączono",
                action, other
            ));
        } else {
            seen.insert(norm, action.clone());
        }
    }

    issues
}

/// Preset manager handles saving/loading configuration presets
pub struct PresetManager {
    recent_presets_path: PathBuf,
}

impl PresetManager {
    pub fn new(app_dir: PathBuf) -> Self {
        Self {
            recent_presets_path: app_dir.join("recent_presets.json"),
        }
    }
    
    /// Save configuration to preset file
    pub fn save(&self, path: &Path, config: &AppConfig) -> Result<()> {
        tracing::info!("Saving preset to: {}", path.display());
        
        let json = serde_json::to_string_pretty(config)
            .context("Failed to serialize config")?;
        
        fs::write(path, json)
            .context("Failed to write preset file")?;
        
        tracing::info!("Preset saved successfully");
        Ok(())
    }
    
    /// Load configuration from preset file.
    pub fn load(&self, path: &Path) -> Result<AppConfig> {
        let (config, issues) = self.load_with_report(path)?;
        for issue in &issues {
            tracing::warn!("Preset hotkey sanitized: {}", issue);
        }
        Ok(config)
    }

    /// Load configuration and also return a list of human-readable issues that
    /// were auto-fixed in the key bindings (unknown actions, invalid keys,
    /// duplicates). Used so the UI can inform the user when a preset JSON had
    /// something wrong with its shortcuts.
    pub fn load_with_report(&self, path: &Path) -> Result<(AppConfig, Vec<String>)> {
        tracing::info!("Loading preset from: {}", path.display());

        let json = fs::read_to_string(path)
            .context("Failed to read preset file")?;

        // Try parsing as new format first
        let mut config: AppConfig = match serde_json::from_str(&json) {
            Ok(cfg) => cfg,
            Err(e) => {
                // Try converting from legacy format
                tracing::info!("Attempting legacy preset conversion...");
                Self::convert_legacy_preset(&json)
                    .context(format!("Failed to parse preset JSON (tried both new and legacy formats): {}", e))?
            }
        };

        // Clean up the key bindings (drop unknown actions, fix invalid keys,
        // resolve duplicates) and ensure all defaults are present.
        let issues = sanitize_key_bindings(&mut config.key_bindings);

        tracing::info!("Preset loaded successfully");
        Ok((config, issues))
    }
    
    /// Convert legacy preset format (UPPER_CASE keys) to new format (snake_case keys)
    fn convert_legacy_preset(json: &str) -> Result<AppConfig> {
        use serde_json::Value;
        
        let legacy: Value = serde_json::from_str(json)
            .context("Failed to parse JSON")?;
        
        let obj = legacy.as_object()
            .context("Preset must be a JSON object")?;
        
        // Helper to get field with fallback
        let get_str = |key1: &str, key2: &str, default: &str| -> String {
            obj.get(key1)
                .or_else(|| obj.get(key2))
                .and_then(|v| v.as_str())
                .unwrap_or(default)
                .to_string()
        };
        
        let get_f32 = |key1: &str, key2: &str, default: f32| -> f32 {
            obj.get(key1)
                .or_else(|| obj.get(key2))
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(default)
        };
        
        let get_i32 = |key1: &str, key2: &str, default: i32| -> i32 {
            obj.get(key1)
                .or_else(|| obj.get(key2))
                .and_then(|v| v.as_i64())
                .map(|v| v as i32)
                .unwrap_or(default)
        };
        
        let get_u8 = |key1: &str, key2: &str, default: u8| -> u8 {
            obj.get(key1)
                .or_else(|| obj.get(key2))
                .and_then(|v| v.as_u64())
                .map(|v| v as u8)
                .unwrap_or(default)
        };
        
        let get_bool = |key1: &str, key2: &str, default: bool| -> bool {
            obj.get(key1)
                .or_else(|| obj.get(key2))
                .and_then(|v| v.as_bool())
                .unwrap_or(default)
        };
        
        // Extract monitor rect (legacy has flat fields)
        let monitor = obj.get("monitor")
            .and_then(|v| v.as_object())
            .map(|m| crate::config::MonitorRect {
                top: m.get("top").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                left: m.get("left").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                width: m.get("width").and_then(|v| v.as_i64()).unwrap_or(800) as u32,
                height: m.get("height").and_then(|v| v.as_i64()).unwrap_or(600) as u32,
            })
            .unwrap_or_else(|| crate::config::MonitorRect {
                top: 0,
                left: 0,
                width: 800,
                height: 600,
            });
        
        let monitor2 = obj.get("monitor2")
            .and_then(|v| v.as_object())
            .map(|m| crate::config::MonitorRect {
                top: m.get("top").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                left: m.get("left").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                width: m.get("width").and_then(|v| v.as_i64()).unwrap_or(800) as u32,
                height: m.get("height").and_then(|v| v.as_i64()).unwrap_or(600) as u32,
            })
            .or_else(|| {
                // Try legacy flat fields (monitor2_top, monitor2_left, etc.)
                Some(crate::config::MonitorRect {
                    top: get_i32("monitor2_top", "MONITOR2_TOP", 0),
                    left: get_i32("monitor2_left", "MONITOR2_LEFT", 0),
                    width: obj.get("monitor2_width")
                        .or_else(|| obj.get("MONITOR2_WIDTH"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32)
                        .unwrap_or(800),
                    height: obj.get("monitor2_height")
                        .or_else(|| obj.get("MONITOR2_HEIGHT"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32)
                        .unwrap_or(600),
                })
            })
            .unwrap();
        
        // Extract key bindings, keeping only recognised actions so obsolete
        // bindings from old presets (e.g. skip_next_line, switch_monitor_toggle,
        // toggle_areas) don't trip validation.
        let known: std::collections::HashSet<String> =
            crate::config::default_key_bindings().into_keys().collect();
        let key_bindings = obj.get("key_bindings")
            .and_then(|v| v.as_object())
            .map(|map| {
                map.iter()
                    .filter(|(k, _)| known.contains(*k))
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_else(crate::config::default_key_bindings);
        
        let config = AppConfig {
            audio_dir: get_str("audio_dir", "AUDIO_DIR", ""),
            text_file_path: get_str("text_file_path", "TEXT_FILE_PATH", ""),
            names_file_path: get_str("names_file_path", "NAMES_FILE_PATH", ""),
            screenshot_dir: get_str("screenshot_dir", "SCREENSHOT_DIR", ""),
            resolution_downscale: get_f32("resolution_downscale", "RESOLUTION_DOWNSCALE", constants::RESOLUTION_DOWNSCALE),
            capture_interval: get_f32("capture_interval", "CAPTURE_INTERVAL", constants::CAPTURE_INTERVAL),
            min_height: get_i32("min_height", "MIN_HEIGHT", constants::MIN_HEIGHT),
            max_height: get_i32("max_height", "MAX_HEIGHT", constants::MAX_HEIGHT),
            ocr_min_confidence: get_f32("ocr_min_confidence", "OCR_MIN_CONFIDENCE", 0.4),
            capture_mode: get_str("capture_mode", "CAPTURE_MODE", constants::CAPTURE_MODE),
            capture_window_query: get_str("capture_window_query", "CAPTURE_WINDOW_QUERY", constants::CAPTURE_WINDOW_QUERY),
            capture_monitor: get_str("capture_monitor", "CAPTURE_MONITOR", ""),
            enable_remove_character_name: get_bool("enable_remove_character_name", "ENABLE_REMOVE_CHARACTER_NAME", false),
            enable_screenshots: get_bool("enable_screenshots", "ENABLE_SCREENSHOTS", false),
            enable_paragraph_ocr: get_bool("enable_paragraph_ocr", "ENABLE_PARAGRAPH_OCR", false),
            enable_typewriter_wait: get_bool("enable_typewriter_wait", "ENABLE_TYPEWRITER_WAIT", false),
            enable_region_overlay: get_bool("enable_region_overlay", "ENABLE_REGION_OVERLAY", false),
            enable_outline_text_mode: get_bool("enable_outline_text_mode", "ENABLE_OUTLINE_TEXT_MODE", constants::ENABLE_OUTLINE_TEXT_MODE),
            outline_white_threshold: get_u8("outline_white_threshold", "OUTLINE_WHITE_THRESHOLD", constants::OUTLINE_WHITE_THRESHOLD),
            outline_dark_threshold: get_u8("outline_dark_threshold", "OUTLINE_DARK_THRESHOLD", constants::OUTLINE_DARK_THRESHOLD),
            use_center_line_1: get_bool("use_center_line_1", "USE_CENTER_LINE_1", true),
            use_center_line_2: get_bool("use_center_line_2", "USE_CENTER_LINE_2", false),
            use_center_line_3: get_bool("use_center_line_3", "USE_CENTER_LINE_3", false),
            center_line_2_start: get_i32("center_line_2_start", "CENTER_LINE_2_START", 1),
            center_line_3_start_ratio: get_f32("center_line_3_start_ratio", "CENTER_LINE_3_START_RATIO", 0.3),
            center_line_margin: get_i32("center_line_margin", "CENTER_LINE_MARGIN", 100),
            volume_reduction_level: get_f32("volume_reduction_level", "VOLUME_REDUCTION_LEVEL", constants::VOLUME_REDUCTION_LEVEL),
            reader_volume: get_f32("reader_volume", "READER_VOLUME", 1.0),
            ducking_target_process: get_str("ducking_target_process", "DUCKING_TARGET_PROCESS", ""),
            enable_output2_system: get_bool("enable_output2_system", "ENABLE_OUTPUT2_SYSTEM", constants::ENABLE_OUTPUT2_SYSTEM),
            enable_dynamic_speed: get_bool("enable_dynamic_speed", "ENABLE_DYNAMIC_SPEED", constants::ENABLE_DYNAMIC_SPEED),
            base_playback_speed: get_f32("base_playback_speed", "BASE_PLAYBACK_SPEED", constants::BASE_PLAYBACK_SPEED),
            overlap_playback_speed: get_f32("overlap_playback_speed", "OVERLAP_PLAYBACK_SPEED", constants::OVERLAP_PLAYBACK_SPEED),
            audio_queue_size: get_u8("audio_queue_size", "AUDIO_QUEUE_SIZE", constants::AUDIO_QUEUE_SIZE),
            minimize_to_tray_on_reader_start: get_bool("minimize_to_tray_on_reader_start", "MINIMIZE_TO_TRAY_ON_READER_START", true),
            resolution: get_str("resolution", "RESOLUTION", "1920x1080"),
            monitor,
            monitor2_enabled: get_bool("monitor2_enabled", "MONITOR2_ENABLED", false),
            monitor2_top: monitor2.top,
            monitor2_left: monitor2.left,
            monitor2_width: monitor2.width,
            monitor2_height: monitor2.height,
            key_bindings,
        };
        
        tracing::info!("Legacy preset converted successfully");
        Ok(config)
    }
    
    /// Get list of recent presets
    pub fn get_recent(&self) -> Result<Vec<RecentPreset>> {
        if !self.recent_presets_path.exists() {
            return Ok(Vec::new());
        }
        
        let json = fs::read_to_string(&self.recent_presets_path)
            .context("Failed to read recent presets file")?;
        
        let presets: Vec<RecentPreset> = serde_json::from_str(&json)
            .context("Failed to parse recent presets JSON")?;
        
        Ok(presets)
    }
    
    /// Add preset to recent list (max 10, newest first)
    pub fn add_to_recent(&self, path: &Path, name: String) -> Result<()> {
        let mut recent = self.get_recent().unwrap_or_default();
        
        // Remove existing entry with same path
        recent.retain(|p| p.path != path.to_string_lossy().to_string());
        
        // Add new entry at front
        let new_entry = RecentPreset {
            path: path.to_string_lossy().to_string(),
            name,
            timestamp: chrono::Utc::now().timestamp(),
        };
        recent.insert(0, new_entry);
        
        // Keep only max entries
        recent.truncate(constants::MAX_RECENT_PRESETS);
        
        // Save updated list
        let json = serde_json::to_string_pretty(&recent)
            .context("Failed to serialize recent presets")?;
        
        fs::write(&self.recent_presets_path, json)
            .context("Failed to write recent presets file")?;
        
        Ok(())
    }
    
    /// Remove preset from recent list
    pub fn remove_from_recent(&self, path: &str) -> Result<()> {
        let mut recent = self.get_recent().unwrap_or_default();
        recent.retain(|p| p.path != path);
        
        let json = serde_json::to_string_pretty(&recent)
            .context("Failed to serialize recent presets")?;
        
        fs::write(&self.recent_presets_path, json)
            .context("Failed to write recent presets file")?;
        
        Ok(())
    }
    
    /// Clear all recent presets
    pub fn clear_recent(&self) -> Result<()> {
        if self.recent_presets_path.exists() {
            fs::remove_file(&self.recent_presets_path)
                .context("Failed to remove recent presets file")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_save_and_load_preset() {
        let temp_dir = TempDir::new().unwrap();
        let preset_path = temp_dir.path().join("test_preset.json");
        let manager = PresetManager::new(temp_dir.path().to_path_buf());
        
        // Create config
        let mut config = AppConfig::default();
        config.resolution = "2560x1440".to_string();
        config.capture_interval = 0.8;
        
        // Save
        manager.save(&preset_path, &config).unwrap();
        assert!(preset_path.exists());
        
        // Load
        let loaded = manager.load(&preset_path).unwrap();
        assert_eq!(loaded.resolution, "2560x1440");
        assert_eq!(loaded.capture_interval, 0.8);
    }
    
    #[test]
    fn test_recent_presets() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PresetManager::new(temp_dir.path().to_path_buf());
        
        // Initially empty
        let recent = manager.get_recent().unwrap();
        assert_eq!(recent.len(), 0);
        
        // Add presets
        let path1 = temp_dir.path().join("preset1.json");
        let path2 = temp_dir.path().join("preset2.json");
        
        manager.add_to_recent(&path1, "Preset 1".to_string()).unwrap();
        manager.add_to_recent(&path2, "Preset 2".to_string()).unwrap();
        
        let recent = manager.get_recent().unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "Preset 2"); // Newest first
        assert_eq!(recent[1].name, "Preset 1");
    }
    
    #[test]
    fn test_recent_presets_max_limit() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PresetManager::new(temp_dir.path().to_path_buf());
        
        // Add more than max
        for i in 0..15 {
            let path = temp_dir.path().join(format!("preset{}.json", i));
            manager.add_to_recent(&path, format!("Preset {}", i)).unwrap();
        }
        
        let recent = manager.get_recent().unwrap();
        assert_eq!(recent.len(), constants::MAX_RECENT_PRESETS);
        assert_eq!(recent[0].name, "Preset 14"); // Newest
    }
    
    #[test]
    fn test_remove_from_recent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PresetManager::new(temp_dir.path().to_path_buf());
        
        let path1 = temp_dir.path().join("preset1.json");
        let path2 = temp_dir.path().join("preset2.json");
        
        manager.add_to_recent(&path1, "Preset 1".to_string()).unwrap();
        manager.add_to_recent(&path2, "Preset 2".to_string()).unwrap();
        
        manager.remove_from_recent(&path1.to_string_lossy()).unwrap();
        
        let recent = manager.get_recent().unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "Preset 2");
    }
}
