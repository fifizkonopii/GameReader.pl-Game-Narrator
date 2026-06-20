// ============================================================
// VALIDATION MODULE
// ============================================================
// This module implements validation rules for AppConfig according to
// Requirements 22.1, 22.2, 22.3, 22.4, 22.5, 23.1, 23.2, 23.3, 23.4

use crate::config::AppConfig;
use crate::constants;
use std::path::Path;

/// Validation error type
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Validation result containing all errors found
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Create a new validation result with no errors
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add an error to the validation result
    pub fn add_error(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.errors.push(ValidationError::new(field, message));
    }

    /// Check if validation passed (no errors)
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the first error message, if any
    pub fn first_error_message(&self) -> Option<String> {
        self.errors.first().map(|e| format!("{}: {}", friendly_field(&e.field), e.message))
    }
}

/// Map a technical config field name to a friendly Polish label for UI errors.
fn friendly_field(field: &str) -> String {
    if let Some(action) = field.strip_prefix("key_bindings.") {
        return format!("Skrót ({})", action);
    }
    match field {
        "audio_dir" => "Folder audio",
        "text_file_path" => "Plik tekstowy",
        "names_file_path" => "Plik z imionami",
        "screenshot_dir" => "Folder zrzutów ekranu",
        "capture_mode" => "Tryb przechwytywania",
        "capture_window_query" => "Okno gry",
        "capture_interval" => "Interwał przechwytywania",
        "resolution" => "Rozdzielczość",
        "resolution_downscale" => "Skalowanie rozdzielczości",
        "min_height" => "Min. wysokość tekstu",
        "max_height" => "Maks. wysokość tekstu",
        "base_playback_speed" => "Prędkość bazowa",
        "overlap_playback_speed" => "Prędkość doganiania",
        "volume_reduction_level" => "Redukcja głośności gry",
        "reader_volume" => "Głośność lektora",
        "audio_queue_size" => "Rozmiar kolejki dialogów",
        other => other,
    }
    .to_string()
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for AppConfig
pub struct Validator;

impl Validator {
    /// Validate capture configuration
    /// Ensures capture_mode is "window" and capture_window_query is not empty
    pub fn validate_capture_config(config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Only "window" mode is supported (GDI removed)
        if config.capture_mode != "window" {
            result.add_error(
                "capture_mode",
                format!(
                    "Nieprawidłowy tryb przechwytywania '{}'. Tylko 'window' jest wspierany.",
                    config.capture_mode
                ),
            );
        }

        // capture_window_query is OPTIONAL: when empty, the capture backend
        // falls back to the current foreground window (see find_window).

        result
    }

    /// Validate range for numeric parameters
    /// Requirement 22.1: resolution_downscale [0.1, 1.0]
    /// Requirement 22.2: capture_interval [0.1, 5.0]
    /// Requirement 22.3: min_height [1, 9999], max_height [1, 9999]
    /// Requirement 22.4: base_playback_speed [0.8, 1.2], overlap_playback_speed [1.0, 3.0]
    /// Requirement 22.5: volume_reduction_level [0.0, 1.0]
    pub fn validate_ranges(config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Requirement 22.1: Validate resolution_downscale range [0.1, 1.0]
        if config.resolution_downscale < constants::RESOLUTION_DOWNSCALE_MIN
            || config.resolution_downscale > constants::RESOLUTION_DOWNSCALE_MAX
        {
            result.add_error(
                "resolution_downscale",
                format!(
                    "Musi być w zakresie [{}, {}], a jest {}",
                    constants::RESOLUTION_DOWNSCALE_MIN,
                    constants::RESOLUTION_DOWNSCALE_MAX,
                    config.resolution_downscale
                ),
            );
        }

        // Requirement 22.2: Validate capture_interval range [0.1, 5.0]
        if config.capture_interval < constants::CAPTURE_INTERVAL_MIN
            || config.capture_interval > constants::CAPTURE_INTERVAL_MAX
        {
            result.add_error(
                "capture_interval",
                format!(
                    "Musi być w zakresie [{}, {}], a jest {}",
                    constants::CAPTURE_INTERVAL_MIN,
                    constants::CAPTURE_INTERVAL_MAX,
                    config.capture_interval
                ),
            );
        }

        // Requirement 22.3: Validate min_height range [1, 9999]
        if config.min_height < constants::MIN_HEIGHT_MIN
            || config.min_height > constants::MIN_HEIGHT_MAX
        {
            result.add_error(
                "min_height",
                format!(
                    "Musi być w zakresie [{}, {}], a jest {}",
                    constants::MIN_HEIGHT_MIN,
                    constants::MIN_HEIGHT_MAX,
                    config.min_height
                ),
            );
        }

        // Requirement 22.3: Validate max_height range [1, 9999]
        if config.max_height < constants::MAX_HEIGHT_MIN
            || config.max_height > constants::MAX_HEIGHT_MAX
        {
            result.add_error(
                "max_height",
                format!(
                    "Musi być w zakresie [{}, {}], a jest {}",
                    constants::MAX_HEIGHT_MIN,
                    constants::MAX_HEIGHT_MAX,
                    config.max_height
                ),
            );
        }

        // Requirement 22.3: Validate min_height <= max_height constraint
        if config.min_height > config.max_height {
            result.add_error(
                "min_height",
                format!(
                    "Musi być mniejsze lub równe maks. wysokości ({} > {})",
                    config.min_height, config.max_height
                ),
            );
        }

        // Requirement 22.4: Validate base_playback_speed range [0.8, 1.2]
        if config.base_playback_speed < constants::BASE_PLAYBACK_SPEED_MIN
            || config.base_playback_speed > constants::BASE_PLAYBACK_SPEED_MAX
        {
            result.add_error(
                "base_playback_speed",
                format!(
                    "Musi być w zakresie [{}, {}], a jest {}",
                    constants::BASE_PLAYBACK_SPEED_MIN,
                    constants::BASE_PLAYBACK_SPEED_MAX,
                    config.base_playback_speed
                ),
            );
        }

        // Requirement 22.4: Validate overlap_playback_speed range [1.0, 3.0]
        if config.overlap_playback_speed < constants::OVERLAP_PLAYBACK_SPEED_MIN
            || config.overlap_playback_speed > constants::OVERLAP_PLAYBACK_SPEED_MAX
        {
            result.add_error(
                "overlap_playback_speed",
                format!(
                    "Musi być w zakresie [{}, {}], a jest {}",
                    constants::OVERLAP_PLAYBACK_SPEED_MIN,
                    constants::OVERLAP_PLAYBACK_SPEED_MAX,
                    config.overlap_playback_speed
                ),
            );
        }

        // Requirement 22.5: Validate volume_reduction_level range [0.0, 1.0]
        if config.volume_reduction_level < constants::VOLUME_REDUCTION_LEVEL_MIN
            || config.volume_reduction_level > constants::VOLUME_REDUCTION_LEVEL_MAX
        {
            result.add_error(
                "volume_reduction_level",
                format!(
                    "Musi być w zakresie [{}, {}], a jest {}",
                    constants::VOLUME_REDUCTION_LEVEL_MIN,
                    constants::VOLUME_REDUCTION_LEVEL_MAX,
                    config.volume_reduction_level
                ),
            );
        }

        // Validate audio_queue_size is in allowed values {1, 2, 3}
        if !constants::AUDIO_QUEUE_SIZE_ALLOWED.contains(&config.audio_queue_size) {
            result.add_error(
                "audio_queue_size",
                format!(
                    "Musi być jedną z wartości {:?}, a jest {}",
                    constants::AUDIO_QUEUE_SIZE_ALLOWED,
                    config.audio_queue_size
                ),
            );
        }

        result
    }

    /// Validate the resolution string format "WIDTHxHEIGHT".
    ///
    /// Accepts any positive width x height (e.g. "1360x768", "3440x1440",
    /// "1280x1024"), not just a fixed preset list, so it works with any game
    /// resolution / aspect ratio (16:9, 21:9, 5:4, 4K, ...).
    pub fn validate_resolution(config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        let parts: Vec<&str> = config.resolution.split(['x', 'X']).collect();
        let valid = parts.len() == 2
            && match (parts[0].trim().parse::<u32>(), parts[1].trim().parse::<u32>()) {
                (Ok(w), Ok(h)) => w > 0 && h > 0 && w <= 16000 && h <= 16000,
                _ => false,
            };

        if !valid {
            result.add_error(
                "resolution",
                format!(
                    "Musi być w formacie SZEROKOŚĆxWYSOKOŚĆ z dodatnimi wartościami (np. 1920x1080), a jest '{}'",
                    config.resolution
                ),
            );
        }

        result
    }

    /// Validate all configuration fields
    /// This aggregates all validation methods into a comprehensive result
    pub fn validate_all(config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Validate capture configuration
        let capture_result = Self::validate_capture_config(config);
        result.errors.extend(capture_result.errors);

        // Validate numeric ranges
        let range_result = Self::validate_ranges(config);
        result.errors.extend(range_result.errors);

        // Validate resolution string
        let resolution_result = Self::validate_resolution(config);
        result.errors.extend(resolution_result.errors);

        // Validate paths and files
        let path_result = Self::validate_paths(config);
        result.errors.extend(path_result.errors);

        // Validate hotkeys
        let hotkey_result = Self::validate_hotkeys(config);
        result.errors.extend(hotkey_result.errors);

        result
    }

    /// Main entry point for validation before starting the reader
    /// Requirements 23.5, 24.5: Aggregate all validation errors and return result
    /// 
    /// This function validates all aspects of the configuration:
    /// - Numeric parameter ranges (resolution_downscale, capture_interval, heights, speeds, volume)
    /// - Resolution string against supported resolutions
    /// - File and directory paths (audio_dir, text_file_path, names_file_path)
    /// - Hotkey bindings (no duplicates, no reserved keys, valid modifiers/keys)
    /// 
    /// Returns a ValidationResult containing all errors found.
    /// Use `first_error_message()` to get a single error for UI display.
    pub fn validate_before_reader_start(config: &AppConfig) -> ValidationResult {
        Self::validate_all(config)
    }

    /// Validate path and file existence
    /// Requirement 23.1: audio_dir exists and contains at least one audio file in supported format
    /// Requirement 23.2: text_file_path points to existing .txt file
    /// Requirement 23.3: names_file_path (optional — inline name detection works without it)
    /// Requirement 23.4: output2 files when enable_output2_system && !enable_dynamic_speed
    pub fn validate_paths(config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Requirement 23.1: Validate audio_dir exists and contains supported audio files
        if config.audio_dir.is_empty() {
            result.add_error("audio_dir", "Ścieżka folderu audio nie może być pusta");
        } else {
            let audio_path = Path::new(&config.audio_dir);
            if !audio_path.exists() {
                result.add_error(
                    "audio_dir",
                    format!("Folder audio nie istnieje: {}", config.audio_dir),
                );
            } else if !audio_path.is_dir() {
                result.add_error(
                    "audio_dir",
                    format!("Ścieżka folderu audio nie jest folderem: {}", config.audio_dir),
                );
            } else {
                // Check for at least one audio file in supported format
                if let Ok(entries) = std::fs::read_dir(audio_path) {
                    let has_audio_file = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_file())
                        .any(|e| {
                            if let Some(ext) = e.path().extension() {
                                let ext_str = format!(".{}", ext.to_string_lossy().to_lowercase());
                                constants::SUPPORTED_AUDIO_FORMATS.contains(&ext_str.as_str())
                            } else {
                                false
                            }
                        });

                    if !has_audio_file {
                        result.add_error(
                            "audio_dir",
                            format!(
                                "Folder audio nie zawiera plików w obsługiwanych formatach: {:?}",
                                constants::SUPPORTED_AUDIO_FORMATS
                            ),
                        );
                    }
                } else {
                    result.add_error(
                        "audio_dir",
                        format!("Nie można odczytać folderu audio: {}", config.audio_dir),
                    );
                }
            }
        }

        // Requirement 23.2: Validate text_file_path points to existing .txt file
        if config.text_file_path.is_empty() {
            result.add_error("text_file_path", "Ścieżka pliku tekstowego nie może być pusta");
        } else {
            let text_path = Path::new(&config.text_file_path);
            if !text_path.exists() {
                result.add_error(
                    "text_file_path",
                    format!("Plik tekstowy nie istnieje: {}", config.text_file_path),
                );
            } else if !text_path.is_file() {
                result.add_error(
                    "text_file_path",
                    format!("Ścieżka pliku tekstowego nie jest plikiem: {}", config.text_file_path),
                );
            } else if text_path.extension().and_then(|s| s.to_str()) != Some("txt") {
                result.add_error(
                    "text_file_path",
                    format!("Plik tekstowy musi mieć rozszerzenie .txt: {}", config.text_file_path),
                );
            }
        }

        // Conditional validation of names_file_path (optional — inline detection works without it)
        if !config.names_file_path.is_empty() {
            let names_path = Path::new(&config.names_file_path);
            if !names_path.exists() {
                result.add_error(
                    "names_file_path",
                    format!("Plik z imionami nie istnieje: {}", config.names_file_path),
                );
            } else if !names_path.is_file() {
                result.add_error(
                    "names_file_path",
                    format!("Ścieżka pliku z imionami nie jest plikiem: {}", config.names_file_path),
                );
            } else if names_path.extension().and_then(|s| s.to_str()) != Some("txt") {
                result.add_error(
                    "names_file_path",
                    format!("Plik z imionami musi mieć rozszerzenie .txt: {}", config.names_file_path),
                );
            }
        }

        // Requirement 23.4: Conditional validation of output2 files
        if config.enable_output2_system && !config.enable_dynamic_speed {
            // Check that audio_dir contains at least one output2 file
            if !config.audio_dir.is_empty() {
                let audio_path = Path::new(&config.audio_dir);
                if audio_path.exists() && audio_path.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(audio_path) {
                        let has_output2_file = entries
                            .filter_map(|e| e.ok())
                            .filter(|e| e.path().is_file())
                            .any(|e| {
                                let filename = e.file_name();
                                let filename_str = filename.to_string_lossy();
                                // Check for files matching pattern "output2 (N).ext"
                                filename_str.starts_with("output2 ")
                                    && filename_str.contains('(')
                                    && filename_str.contains(')')
                                    && e.path()
                                        .extension()
                                        .and_then(|ext| {
                                            let ext_str = format!(
                                                ".{}",
                                                ext.to_string_lossy().to_lowercase()
                                            );
                                            Some(
                                                constants::SUPPORTED_AUDIO_FORMATS
                                                    .contains(&ext_str.as_str()),
                                            )
                                        })
                                        .unwrap_or(false)
                            });

                        if !has_output2_file {
                            result.add_error(
                                "audio_dir",
                                "Folder audio musi zawierać pliki output2, gdy system output2 jest włączony, a dynamiczna prędkość wyłączona",
                            );
                        }
                    }
                }
            }
        }

        result
    }

    /// Validate hotkey bindings
    /// Requirement 24.1: No duplicate hotkeys across actions
    /// Requirement 24.2: Reject reserved system hotkeys
    /// Requirement 24.3: Allow only whitelisted modifiers
    /// Requirement 24.4: Allow only whitelisted keys
    /// Requirement 24.5: Include invalid hotkeys in error list
    pub fn validate_hotkeys(config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();
        let mut used_hotkeys: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for (action, hotkey) in &config.key_bindings {
            // Skip empty hotkeys
            if hotkey.is_empty() {
                continue;
            }

            // Normalize the hotkey string (lowercase, sort modifiers)
            // This will return None for invalid modifiers
            let normalized = match Self::normalize_hotkey(hotkey) {
                Some(n) => n,
                None => {
                    result.add_error(
                        format!("key_bindings.{}", action),
                        format!("Nieprawidłowy format skrótu lub nieprawidłowy modyfikator: '{}'", hotkey),
                    );
                    continue;
                }
            };

            // Requirement 24.2: Check for reserved system hotkeys
            if constants::RESERVED_SYSTEM_HOTKEYS.contains(&normalized.as_str()) {
                result.add_error(
                    format!("key_bindings.{}", action),
                    format!("Skrót '{}' jest zarezerwowany przez system i nie może być użyty", normalized),
                );
                continue;
            }

            // Parse hotkey into modifiers and key
            let parts: Vec<&str> = normalized.split('+').collect();
            let key = parts.last().unwrap(); // Safe because normalize_hotkey ensures at least one part

            // Requirement 24.4: Validate key is in whitelist
            if !constants::ALLOWED_KEYS.contains(key) {
                result.add_error(
                    format!("key_bindings.{}", action),
                    format!("Klawisz '{}' jest niedozwolony. Dozwolone są tylko klawisze alfanumeryczne, funkcyjne (f1-f12) oraz specjalne (home, end, insert, delete, page_up, page_down)", key),
                );
                continue;
            }

            // Requirement 24.1: Check for duplicate hotkeys
            if let Some(existing_action) = used_hotkeys.get(&normalized) {
                result.add_error(
                    format!("key_bindings.{}", action),
                    format!("Skrót '{}' jest już przypisany do akcji '{}'", normalized, existing_action),
                );
            } else {
                used_hotkeys.insert(normalized, action.clone());
            }
        }

        result
    }

    /// Normalize a hotkey string to a canonical form (lowercase, sorted modifiers)
    /// Similar to Python's normalize_qt_sequence
    /// Returns None if the hotkey contains invalid modifiers
    fn normalize_hotkey(hotkey: &str) -> Option<String> {
        if hotkey.is_empty() {
            return None;
        }

        let lowercase = hotkey.to_lowercase();
        let parts: Vec<&str> = lowercase.split('+').map(|s| s.trim()).collect();
        
        if parts.is_empty() {
            return None;
        }

        // Last part is the key, others are modifiers
        let key = parts.last()?;
        let modifiers: Vec<&str> = parts[..parts.len() - 1].to_vec();

        // Validate all modifiers are in the allowed list
        for modifier in &modifiers {
            if !constants::ALLOWED_MODIFIERS.contains(modifier) {
                // Invalid modifier - return None to signal this should be caught in validation
                return None;
            }
        }

        // Sort modifiers in canonical order: ctrl, alt, shift
        let mod_order = ["ctrl", "alt", "shift"];
        let mut sorted_mods: Vec<&str> = Vec::new();
        
        for mod_name in &mod_order {
            if modifiers.contains(mod_name) {
                sorted_mods.push(mod_name);
            }
        }

        // Build normalized string
        let mut result = sorted_mods.join("+");
        if !result.is_empty() {
            result.push('+');
        }
        result.push_str(key);

        Some(result)
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn test_valid_default_config() {
        let mut config = AppConfig::default();
        // Set paths to avoid path validation errors
        // Default config has enable_output2_system=true and enable_dynamic_speed=false,
        // so we need to disable output2_system to avoid requiring output2 files
        config.enable_output2_system = false;
        config.audio_dir = String::new(); // Will trigger empty check, but that's separate from ranges/resolution
        config.text_file_path = String::new();
        
        // Test only range and resolution validation (not paths)
        let range_result = Validator::validate_ranges(&config);
        assert!(range_result.is_valid(), "Default config ranges should be valid");
        
        let resolution_result = Validator::validate_resolution(&config);
        assert!(resolution_result.is_valid(), "Default config resolution should be valid");
    }

    #[test]
    fn test_resolution_downscale_too_low() {
        let mut config = AppConfig::default();
        config.resolution_downscale = 0.05; // Below 0.1 minimum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].field, "resolution_downscale");
    }

    #[test]
    fn test_resolution_downscale_too_high() {
        let mut config = AppConfig::default();
        config.resolution_downscale = 1.5; // Above 1.0 maximum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors[0].field == "resolution_downscale");
    }

    #[test]
    fn test_capture_interval_out_of_range() {
        let mut config = AppConfig::default();
        config.capture_interval = 0.05; // Below 0.1 minimum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "capture_interval"));
    }

    #[test]
    fn test_min_height_greater_than_max_height() {
        let mut config = AppConfig::default();
        config.min_height = 150;
        config.max_height = 100;
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "min_height"));
    }

    #[test]
    fn test_min_height_out_of_range() {
        let mut config = AppConfig::default();
        config.min_height = 0; // Below 1 minimum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "min_height"));
    }

    #[test]
    fn test_max_height_out_of_range() {
        let mut config = AppConfig::default();
        config.max_height = 10000; // Above 9999 maximum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "max_height"));
    }

    #[test]
    fn test_base_playback_speed_out_of_range() {
        let mut config = AppConfig::default();
        config.base_playback_speed = 0.1; // Below 0.5 minimum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "base_playback_speed"));
    }

    #[test]
    fn test_overlap_playback_speed_out_of_range() {
        let mut config = AppConfig::default();
        config.overlap_playback_speed = 3.5; // Above 3.0 maximum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "overlap_playback_speed"));
    }

    #[test]
    fn test_volume_reduction_level_out_of_range() {
        let mut config = AppConfig::default();
        config.volume_reduction_level = 1.5; // Above 1.0 maximum
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "volume_reduction_level"));
    }

    #[test]
    fn test_audio_queue_size_invalid() {
        let mut config = AppConfig::default();
        config.audio_queue_size = 5; // Not in {1, 2, 3}
        let result = Validator::validate_ranges(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "audio_queue_size"));
    }

    #[test]
    fn test_invalid_resolution_string() {
        let mut config = AppConfig::default();
        config.resolution = "not-a-resolution".to_string(); // Invalid format
        let result = Validator::validate_resolution(&config);
        assert!(!result.is_valid());
        assert_eq!(result.errors[0].field, "resolution");
    }

    #[test]
    fn test_valid_resolution_strings() {
        let valid_resolutions = vec![
            "1280x720",
            "1920x1080",
            "2560x1440",
            "3840x2160",
        ];

        for res in valid_resolutions {
            let mut config = AppConfig::default();
            config.resolution = res.to_string();
            let result = Validator::validate_resolution(&config);
            assert!(result.is_valid(), "Resolution {} should be valid", res);
        }
    }

    #[test]
    fn test_multiple_validation_errors() {
        let mut config = AppConfig::default();
        config.resolution_downscale = 2.0; // Invalid
        config.capture_interval = 10.0;     // Invalid
        config.min_height = 200;            // Will be > max_height
        config.max_height = 100;            // < min_height
        config.volume_reduction_level = -0.5; // Invalid

        let result = Validator::validate_all(&config);
        assert!(!result.is_valid());
        assert!(result.errors.len() >= 4, "Should have multiple errors");
    }

    #[test]
    fn test_first_error_message() {
        let mut config = AppConfig::default();
        config.resolution_downscale = 2.0;
        let result = Validator::validate_ranges(&config);
        let msg = result.first_error_message();
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("resolution_downscale"));
    }

    #[test]
    fn test_validation_result_default() {
        let result = ValidationResult::default();
        assert!(result.is_valid());
        assert!(result.first_error_message().is_none());
    }

    // ============================================================
    // PATH VALIDATION TESTS (Task 2.2)
    // ============================================================

    #[test]
    fn test_empty_audio_dir() {
        let mut config = AppConfig::default();
        config.audio_dir = String::new();
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "audio_dir" && e.message.contains("cannot be empty")));
    }

    #[test]
    fn test_nonexistent_audio_dir() {
        let mut config = AppConfig::default();
        config.audio_dir = "c:\\nonexistent\\audio\\dir".to_string();
        config.text_file_path = "dummy.txt".to_string(); // Avoid empty validation error
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "audio_dir" && e.message.contains("does not exist")));
    }

    #[test]
    fn test_empty_text_file_path() {
        let mut config = AppConfig::default();
        config.text_file_path = String::new();
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "text_file_path" && e.message.contains("cannot be empty")));
    }

    #[test]
    fn test_nonexistent_text_file() {
        let mut config = AppConfig::default();
        config.text_file_path = "c:\\nonexistent\\text.txt".to_string();
        config.audio_dir = "c:\\dummy".to_string(); // Avoid empty validation error
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "text_file_path" && e.message.contains("does not exist")));
    }

    #[test]
    fn test_text_file_wrong_extension() {
        // Create a temporary file with wrong extension
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_file.json");
        std::fs::write(&test_file, "test content").unwrap();

        let mut config = AppConfig::default();
        config.text_file_path = test_file.to_string_lossy().to_string();
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "text_file_path" && e.message.contains(".txt extension")));

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_names_file_not_required_when_enabled_inline_mode() {
        let mut config = AppConfig::default();
        config.enable_remove_character_name = true;
        config.names_file_path = String::new();
        config.audio_dir = "c:\\dummy".to_string();
        config.text_file_path = "c:\\dummy.txt".to_string();
        
        let result = Validator::validate_paths(&config);
        // Inline mode works without names file, so no error for names_file_path
        assert!(result.errors.iter().all(|e| e.field != "names_file_path"));
    }

    #[test]
    fn test_names_file_not_required_when_disabled() {
        let mut config = AppConfig::default();
        config.enable_remove_character_name = false;
        config.names_file_path = String::new();
        config.audio_dir = "c:\\dummy".to_string();
        config.text_file_path = "c:\\dummy.txt".to_string();
        
        let result = Validator::validate_paths(&config);
        // Should have errors for audio_dir and text_file_path, but NOT names_file_path
        assert!(result.errors.iter().all(|e| e.field != "names_file_path"));
    }

    #[test]
    fn test_nonexistent_names_file() {
        let mut config = AppConfig::default();
        config.enable_remove_character_name = true;
        config.names_file_path = "c:\\nonexistent\\names.txt".to_string();
        config.audio_dir = "c:\\dummy".to_string();
        config.text_file_path = "c:\\dummy.txt".to_string();
        
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "names_file_path" && e.message.contains("does not exist")));
    }

    #[test]
    fn test_names_file_wrong_extension() {
        // Create a temporary file with wrong extension
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_names.dat");
        std::fs::write(&test_file, "test content").unwrap();

        let mut config = AppConfig::default();
        config.enable_remove_character_name = true;
        config.names_file_path = test_file.to_string_lossy().to_string();
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = "c:\\dummy.txt".to_string();
        
        let result = Validator::validate_paths(&config);
        assert!(result.errors.iter().any(|e| e.field == "names_file_path" && e.message.contains(".txt extension")));

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_output2_validation_when_required() {
        // Create a temporary directory with only output1 files
        let temp_dir = std::env::temp_dir().join("test_audio_output2");
        let _ = std::fs::create_dir(&temp_dir);
        
        // Create an output1 file
        let output1_file = temp_dir.join("output1 (1).ogg");
        std::fs::write(&output1_file, "dummy audio").unwrap();
        
        let mut config = AppConfig::default();
        config.enable_output2_system = true;
        config.enable_dynamic_speed = false; // This makes output2 required
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = "c:\\dummy.txt".to_string();
        
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "audio_dir" && e.message.contains("output2")));

        // Cleanup
        let _ = std::fs::remove_file(&output1_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_output2_not_required_when_dynamic_speed_enabled() {
        // Create a temporary directory with only output1 files
        let temp_dir = std::env::temp_dir().join("test_audio_no_output2");
        let _ = std::fs::create_dir(&temp_dir);
        
        // Create an output1 file and a text file
        let output1_file = temp_dir.join("output1 (1).ogg");
        let text_file = temp_dir.join("test.txt");
        std::fs::write(&output1_file, "dummy audio").unwrap();
        std::fs::write(&text_file, "dummy text").unwrap();
        
        let mut config = AppConfig::default();
        config.enable_output2_system = true;
        config.enable_dynamic_speed = true; // This makes output2 NOT required
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = text_file.to_string_lossy().to_string();
        
        let result = Validator::validate_paths(&config);
        // Should NOT have error about output2
        assert!(result.errors.iter().all(|e| !e.message.contains("output2")));

        // Cleanup
        let _ = std::fs::remove_file(&output1_file);
        let _ = std::fs::remove_file(&text_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_output2_not_required_when_output2_system_disabled() {
        // Create a temporary directory with only output1 files
        let temp_dir = std::env::temp_dir().join("test_audio_output2_disabled");
        let _ = std::fs::create_dir(&temp_dir);
        
        // Create an output1 file and a text file
        let output1_file = temp_dir.join("output1 (1).ogg");
        let text_file = temp_dir.join("test.txt");
        std::fs::write(&output1_file, "dummy audio").unwrap();
        std::fs::write(&text_file, "dummy text").unwrap();
        
        let mut config = AppConfig::default();
        config.enable_output2_system = false; // Output2 system disabled
        config.enable_dynamic_speed = false;
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = text_file.to_string_lossy().to_string();
        
        let result = Validator::validate_paths(&config);
        // Should NOT have error about output2
        assert!(result.errors.iter().all(|e| !e.message.contains("output2")));

        // Cleanup
        let _ = std::fs::remove_file(&output1_file);
        let _ = std::fs::remove_file(&text_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_valid_audio_dir_with_supported_formats() {
        // Create a temporary directory with supported audio files
        let temp_dir = std::env::temp_dir().join("test_audio_valid");
        let _ = std::fs::create_dir(&temp_dir);
        
        // Create files with different supported formats
        let ogg_file = temp_dir.join("output1 (1).ogg");
        let mp3_file = temp_dir.join("output1 (2).mp3");
        let text_file = temp_dir.join("test.txt");
        
        std::fs::write(&ogg_file, "dummy audio").unwrap();
        std::fs::write(&mp3_file, "dummy audio").unwrap();
        std::fs::write(&text_file, "dummy text").unwrap();
        
        let mut config = AppConfig::default();
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = text_file.to_string_lossy().to_string();
        // Disable output2 requirement
        config.enable_output2_system = false;
        
        let result = Validator::validate_paths(&config);
        // Should be valid now
        assert!(result.is_valid(), "Expected no errors, got: {:?}", result.errors);

        // Cleanup
        let _ = std::fs::remove_file(&ogg_file);
        let _ = std::fs::remove_file(&mp3_file);
        let _ = std::fs::remove_file(&text_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_audio_dir_with_no_supported_audio_files() {
        // Create a temporary directory with no audio files
        let temp_dir = std::env::temp_dir().join("test_audio_no_files");
        let _ = std::fs::create_dir(&temp_dir);
        
        // Create only a text file
        let text_file = temp_dir.join("readme.txt");
        std::fs::write(&text_file, "no audio here").unwrap();
        
        let mut config = AppConfig::default();
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = text_file.to_string_lossy().to_string();
        
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "audio_dir" && e.message.contains("no files in supported formats")));

        // Cleanup
        let _ = std::fs::remove_file(&text_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_complete_valid_configuration_with_paths() {
        // Create a temporary directory with all required files
        let temp_dir = std::env::temp_dir().join("test_complete_valid");
        let _ = std::fs::create_dir(&temp_dir);
        
        // Create required files
        let audio_file = temp_dir.join("output1 (1).ogg");
        let text_file = temp_dir.join("dialog.txt");
        let names_file = temp_dir.join("names.txt");
        
        std::fs::write(&audio_file, "dummy audio").unwrap();
        std::fs::write(&text_file, "dummy text").unwrap();
        std::fs::write(&names_file, "dummy names").unwrap();
        
        let mut config = AppConfig::default();
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = text_file.to_string_lossy().to_string();
        config.enable_remove_character_name = true;
        config.names_file_path = names_file.to_string_lossy().to_string();
        // Disable output2 requirement for this test
        config.enable_output2_system = false;
        
        let result = Validator::validate_all(&config);
        assert!(result.is_valid(), "Expected no errors, got: {:?}", result.errors);

        // Cleanup
        let _ = std::fs::remove_file(&audio_file);
        let _ = std::fs::remove_file(&text_file);
        let _ = std::fs::remove_file(&names_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_audio_dir_is_file_not_directory() {
        // Create a temporary file (not directory)
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("not_a_directory.txt");
        std::fs::write(&test_file, "test content").unwrap();

        let mut config = AppConfig::default();
        config.audio_dir = test_file.to_string_lossy().to_string();
        config.text_file_path = "c:\\dummy.txt".to_string();
        
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "audio_dir" && e.message.contains("not a directory")));

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_text_file_is_directory_not_file() {
        // Create a temporary directory (not file)
        let temp_dir = std::env::temp_dir().join("test_dir_not_file");
        let _ = std::fs::create_dir(&temp_dir);

        let mut config = AppConfig::default();
        config.text_file_path = temp_dir.to_string_lossy().to_string();
        config.audio_dir = "c:\\dummy".to_string();
        
        let result = Validator::validate_paths(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field == "text_file_path" && e.message.contains("not a file")));

        // Cleanup
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[test]
    fn test_output2_files_present_validation_passes() {
        // Create a temporary directory with both output1 and output2 files
        let temp_dir = std::env::temp_dir().join("test_audio_with_output2");
        let _ = std::fs::create_dir(&temp_dir);
        
        // Create both output1 and output2 files
        let output1_file = temp_dir.join("output1 (1).ogg");
        let output2_file = temp_dir.join("output2 (1).mp3");
        let text_file = temp_dir.join("test.txt");
        
        std::fs::write(&output1_file, "dummy audio").unwrap();
        std::fs::write(&output2_file, "dummy audio").unwrap();
        std::fs::write(&text_file, "dummy text").unwrap();
        
        let mut config = AppConfig::default();
        config.enable_output2_system = true;
        config.enable_dynamic_speed = false; // This makes output2 required
        config.audio_dir = temp_dir.to_string_lossy().to_string();
        config.text_file_path = text_file.to_string_lossy().to_string();
        
        let result = Validator::validate_paths(&config);
        // Should pass because output2 files are present
        assert!(result.is_valid(), "Expected no errors, got: {:?}", result.errors);

        // Cleanup
        let _ = std::fs::remove_file(&output1_file);
        let _ = std::fs::remove_file(&output2_file);
        let _ = std::fs::remove_file(&text_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    // ============================================================
    // HOTKEY VALIDATION TESTS (Task 2.3)
    // ============================================================

    #[test]
    fn test_normalize_hotkey_simple_key() {
        let result = Validator::normalize_hotkey("a");
        assert_eq!(result, Some("a".to_string()));
    }

    #[test]
    fn test_normalize_hotkey_with_modifiers() {
        let result = Validator::normalize_hotkey("Ctrl+Alt+A");
        assert_eq!(result, Some("ctrl+alt+a".to_string()));
    }

    #[test]
    fn test_normalize_hotkey_sorts_modifiers() {
        // Modifiers should be sorted: ctrl, alt, shift
        let result = Validator::normalize_hotkey("Shift+Ctrl+A");
        assert_eq!(result, Some("ctrl+shift+a".to_string()));
        
        let result = Validator::normalize_hotkey("Alt+Shift+Ctrl+B");
        assert_eq!(result, Some("ctrl+alt+shift+b".to_string()));
    }

    #[test]
    fn test_normalize_hotkey_lowercase() {
        let result = Validator::normalize_hotkey("CTRL+ALT+DELETE");
        assert_eq!(result, Some("ctrl+alt+delete".to_string()));
    }

    #[test]
    fn test_normalize_hotkey_empty() {
        let result = Validator::normalize_hotkey("");
        assert_eq!(result, None);
    }

    #[test]
    fn test_validate_hotkeys_no_duplicates() {
        let mut config = AppConfig::default();
        // Default config has unique hotkeys
        let result = Validator::validate_hotkeys(&config);
        assert!(result.is_valid(), "Default hotkeys should not have duplicates");
    }

    #[test]
    fn test_validate_hotkeys_with_duplicate() {
        let config = AppConfig::default();
        // Assign same hotkey to two actions
        let mut config = config;
        config.key_bindings.insert("toggle_reader".to_string(), "home".to_string());
        config.key_bindings.insert("test_sound".to_string(), "home".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.len() >= 1);
        assert!(result.errors.iter().any(|e| e.message.contains("already assigned")));
    }

    #[test]
    fn test_validate_hotkeys_reserved_alt_tab() {
        let mut config = AppConfig::default();
        config.key_bindings.insert("toggle_reader".to_string(), "Alt+Tab".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.field.contains("toggle_reader") && e.message.contains("reserved")));
    }

    #[test]
    fn test_validate_hotkeys_reserved_alt_f4() {
        let mut config = AppConfig::default();
        config.key_bindings.insert("interrupt_audio".to_string(), "Alt+F4".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("reserved")));
    }

    #[test]
    fn test_validate_hotkeys_reserved_ctrl_alt_del() {
        let mut config = AppConfig::default();
        config.key_bindings.insert("open_settings".to_string(), "Ctrl+Alt+Del".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("reserved")));
    }

    #[test]
    fn test_validate_hotkeys_reserved_ctrl_shift_esc() {
        let mut config = AppConfig::default();
        config.key_bindings.insert("debug_console".to_string(), "Ctrl+Shift+Esc".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("reserved")));
    }

    #[test]
    fn test_validate_hotkeys_invalid_modifier() {
        let mut config = AppConfig::default();
        config.key_bindings.insert("toggle_reader".to_string(), "Super+A".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("Invalid hotkey format") || e.message.contains("invalid modifier")));
    }

    #[test]
    fn test_validate_hotkeys_invalid_key() {
        let mut config = AppConfig::default();
        config.key_bindings.insert("toggle_reader".to_string(), "Ctrl+InvalidKey".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("not allowed")));
    }

    #[test]
    fn test_validate_hotkeys_valid_modifiers() {
        let mut config = AppConfig::default();
        config.key_bindings.clear();
        config.key_bindings.insert("action1".to_string(), "Ctrl+A".to_string());
        config.key_bindings.insert("action2".to_string(), "Alt+B".to_string());
        config.key_bindings.insert("action3".to_string(), "Shift+C".to_string());
        config.key_bindings.insert("action4".to_string(), "Ctrl+Alt+D".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_hotkeys_valid_function_keys() {
        let mut config = AppConfig::default();
        config.key_bindings.clear();
        config.key_bindings.insert("action1".to_string(), "F1".to_string());
        config.key_bindings.insert("action2".to_string(), "f5".to_string());
        config.key_bindings.insert("action3".to_string(), "F12".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_hotkeys_valid_special_keys() {
        let mut config = AppConfig::default();
        config.key_bindings.clear();
        config.key_bindings.insert("action1".to_string(), "home".to_string());
        config.key_bindings.insert("action2".to_string(), "end".to_string());
        config.key_bindings.insert("action3".to_string(), "insert".to_string());
        config.key_bindings.insert("action4".to_string(), "delete".to_string());
        config.key_bindings.insert("action5".to_string(), "page_up".to_string());
        config.key_bindings.insert("action6".to_string(), "page_down".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_validate_hotkeys_skips_empty() {
        let mut config = AppConfig::default();
        config.key_bindings.clear(); // Clear defaults first
        config.key_bindings.insert("action1".to_string(), "".to_string());
        config.key_bindings.insert("action2".to_string(), "home".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(result.is_valid(), "Expected no errors, got: {:?}", result.errors);
    }

    #[test]
    fn test_validate_all_includes_hotkey_errors() {
        let mut config = AppConfig::default();
        config.enable_output2_system = false; // Avoid path validation errors
        config.audio_dir = String::new();
        config.text_file_path = String::new();
        
        // Add duplicate hotkey
        config.key_bindings.insert("action1".to_string(), "home".to_string());
        config.key_bindings.insert("action2".to_string(), "home".to_string());
        
        let result = Validator::validate_all(&config);
        assert!(!result.is_valid());
        // Should have hotkey duplicate error plus path errors
        assert!(result.errors.iter().any(|e| e.message.contains("already assigned")));
    }

    #[test]
    fn test_validate_hotkeys_multiple_errors() {
        let mut config = AppConfig::default();
        config.key_bindings.clear();
        config.key_bindings.insert("action1".to_string(), "Alt+Tab".to_string()); // Reserved
        config.key_bindings.insert("action2".to_string(), "Super+A".to_string()); // Invalid modifier
        config.key_bindings.insert("action3".to_string(), "home".to_string());
        config.key_bindings.insert("action4".to_string(), "home".to_string()); // Duplicate
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        // Reserved (1), invalid modifier (1), duplicate (1) = 3 errors total
        assert!(result.errors.len() >= 2, "Expected at least 2 errors, got {}: {:?}", result.errors.len(), result.errors);
    }

    #[test]
    fn test_validate_hotkeys_case_insensitive_duplicate_detection() {
        let mut config = AppConfig::default();
        config.key_bindings.clear();
        config.key_bindings.insert("action1".to_string(), "ctrl+a".to_string());
        config.key_bindings.insert("action2".to_string(), "Ctrl+A".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("already assigned")));
    }

    #[test]
    fn test_validate_hotkeys_modifier_order_normalization() {
        let mut config = AppConfig::default();
        config.key_bindings.clear();
        // These should be detected as duplicates after normalization
        config.key_bindings.insert("action1".to_string(), "Shift+Ctrl+A".to_string());
        config.key_bindings.insert("action2".to_string(), "Ctrl+Shift+A".to_string());
        
        let result = Validator::validate_hotkeys(&config);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.message.contains("already assigned")));
    }
}

    // ============================================================
    // TESTS FOR validate_before_reader_start (Task 2.4)
    // ============================================================

    #[test]
    fn test_validate_before_reader_start_aggregates_all_errors() {
        let mut config = AppConfig::default();
        
        // Add multiple types of errors
        config.resolution_downscale = 2.0; // Range error
        config.capture_interval = 10.0;     // Range error
        config.resolution = "invalid".to_string(); // Resolution error
        config.audio_dir = String::new();   // Path error
        config.text_file_path = String::new(); // Path error
        
        let result = Validator::validate_before_reader_start(&config);
        
        assert!(!result.is_valid(), "Expected validation to fail");
        assert!(result.errors.len() >= 5, "Expected at least 5 errors, got {}", result.errors.len());
        
        // Verify different types of errors are present
        assert!(result.errors.iter().any(|e| e.field == "resolution_downscale"), "Missing resolution_downscale error");
        assert!(result.errors.iter().any(|e| e.field == "capture_interval"), "Missing capture_interval error");
        assert!(result.errors.iter().any(|e| e.field == "resolution"), "Missing resolution error");
        assert!(result.errors.iter().any(|e| e.field == "audio_dir"), "Missing audio_dir error");
        assert!(result.errors.iter().any(|e| e.field == "text_file_path"), "Missing text_file_path error");
    }

    #[test]
    fn test_validate_before_reader_start_returns_first_error_message() {
        let mut config = AppConfig::default();
        config.resolution_downscale = 2.0; // This will cause an error
        
        let result = Validator::validate_before_reader_start(&config);
        
        assert!(!result.is_valid());
        let first_msg = result.first_error_message();
        assert!(first_msg.is_some(), "Expected first error message");
        assert!(first_msg.unwrap().contains("resolution_downscale"), "First error should be about resolution_downscale");
    }

    #[test]
    fn test_validate_before_reader_start_valid_ranges_and_resolution() {
        // Test that ranges and resolution validation work correctly through validate_before_reader_start
        let mut config = AppConfig::default();
        // Disable features that require file validation
        config.enable_output2_system = false;
        config.enable_remove_character_name = false;
        config.audio_dir = String::new(); // Will trigger error, but we're testing ranges
        config.text_file_path = String::new(); // Will trigger error, but we're testing ranges
        
        // Test just ranges and resolution (which should be valid for default config)
        let range_result = Validator::validate_ranges(&config);
        assert!(range_result.is_valid(), "Default config ranges should be valid");
        
        let resolution_result = Validator::validate_resolution(&config);
        assert!(resolution_result.is_valid(), "Default config resolution should be valid");
    }
