//! Resolution scaling for preset geometry and parameters.
//!
//! This module implements proportional scaling of capture regions and OCR parameters
//! when switching between different display resolutions.

use crate::config::{AppConfig, MonitorRect};

/// Supported base resolutions for scaling
pub const SUPPORTED_RESOLUTIONS: &[&str] = &[
    "1280x720",   // 720p
    "1920x1080",  // 1080p
    "2560x1440",  // 1440p
    "3840x2160",  // 4K
    "5120x2880",  // 5K
];

/// Resolution scaler for proportional geometry and parameter scaling
pub struct ResolutionScaler {
    base_resolution: (u32, u32),
    base_monitor: MonitorRect,
    base_monitor2_enabled: bool,
    base_monitor2_top: i32,
    base_monitor2_left: i32,
    base_monitor2_width: u32,
    base_monitor2_height: u32,
    base_downscale: f32,
    base_min_height: i32,
    base_max_height: i32,
}

impl ResolutionScaler {
    /// Creates a new resolution scaler with base values from current config
    pub fn new(config: &AppConfig) -> Self {
        let base_resolution = parse_resolution(&config.resolution)
            .unwrap_or((1920, 1080));
        
        Self {
            base_resolution,
            base_monitor: config.monitor,
            base_monitor2_enabled: config.monitor2_enabled,
            base_monitor2_top: config.monitor2_top,
            base_monitor2_left: config.monitor2_left,
            base_monitor2_width: config.monitor2_width,
            base_monitor2_height: config.monitor2_height,
            base_downscale: config.resolution_downscale,
            base_min_height: config.min_height,
            base_max_height: config.max_height,
        }
    }

    /// Scales configuration to target resolution
    pub fn scale_to(&self, config: &mut AppConfig, target_resolution: &str) -> Result<(), String> {
        let target = parse_resolution(target_resolution)
            .ok_or_else(|| format!("Invalid resolution: {}", target_resolution))?;

        let scale_x = target.0 as f32 / self.base_resolution.0 as f32;
        let scale_y = target.1 as f32 / self.base_resolution.1 as f32;

        // Scale monitor geometry
        config.monitor = scale_monitor_rect(&self.base_monitor, scale_x, scale_y);
        
        // Scale monitor2 if enabled
        if self.base_monitor2_enabled {
            config.monitor2_top = (self.base_monitor2_top as f32 * scale_y).round() as i32;
            config.monitor2_left = (self.base_monitor2_left as f32 * scale_x).round() as i32;
            config.monitor2_width = (self.base_monitor2_width as f32 * scale_x).round() as u32;
            config.monitor2_height = (self.base_monitor2_height as f32 * scale_y).round() as u32;
        }

        // Scale resolution_downscale proportionally
        config.resolution_downscale = f32::max(0.1, f32::min(1.0, self.base_downscale * scale_y));

        // Scale height filters proportionally
        config.min_height = (self.base_min_height as f32 * scale_y).round() as i32;
        config.max_height = (self.base_max_height as f32 * scale_y).round() as i32;

        // Update resolution string
        config.resolution = target_resolution.to_string();

        Ok(())
    }

    /// Restores base values (scales back to base resolution)
    pub fn restore_base(&self, config: &mut AppConfig) {
        config.monitor = self.base_monitor;
        config.monitor2_enabled = self.base_monitor2_enabled;
        config.monitor2_top = self.base_monitor2_top;
        config.monitor2_left = self.base_monitor2_left;
        config.monitor2_width = self.base_monitor2_width;
        config.monitor2_height = self.base_monitor2_height;
        config.resolution_downscale = self.base_downscale;
        config.min_height = self.base_min_height;
        config.max_height = self.base_max_height;
        config.resolution = format!("{}x{}", self.base_resolution.0, self.base_resolution.1);
    }

    /// Updates base values from current config (call when user modifies settings)
    pub fn update_base(&mut self, config: &AppConfig) {
        self.base_resolution = parse_resolution(&config.resolution)
            .unwrap_or(self.base_resolution);
        self.base_monitor = config.monitor;
        self.base_monitor2_enabled = config.monitor2_enabled;
        self.base_monitor2_top = config.monitor2_top;
        self.base_monitor2_left = config.monitor2_left;
        self.base_monitor2_width = config.monitor2_width;
        self.base_monitor2_height = config.monitor2_height;
        self.base_downscale = config.resolution_downscale;
        self.base_min_height = config.min_height;
        self.base_max_height = config.max_height;
    }
}

/// Parses resolution string "WIDTHxHEIGHT" into (width, height) tuple
fn parse_resolution(res: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = res.split('x').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let width = parts[0].parse::<u32>().ok()?;
    let height = parts[1].parse::<u32>().ok()?;
    
    Some((width, height))
}

/// Scales a monitor rect proportionally
fn scale_monitor_rect(rect: &MonitorRect, scale_x: f32, scale_y: f32) -> MonitorRect {
    MonitorRect {
        top: (rect.top as f32 * scale_y).round() as i32,
        left: (rect.left as f32 * scale_x).round() as i32,
        width: (rect.width as f32 * scale_x).round() as u32,
        height: (rect.height as f32 * scale_y).round() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.resolution = "1920x1080".to_string();
        config.monitor = MonitorRect {
            top: 100,
            left: 200,
            width: 1600,
            height: 900,
        };
        config.resolution_downscale = 0.8;
        config.min_height = 20;
        config.max_height = 50;
        config
    }

    #[test]
    fn test_parse_resolution() {
        assert_eq!(parse_resolution("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_resolution("2560x1440"), Some((2560, 1440)));
        assert_eq!(parse_resolution("3840x2160"), Some((3840, 2160)));
        assert_eq!(parse_resolution("invalid"), None);
        assert_eq!(parse_resolution("1920"), None);
    }

    #[test]
    fn test_scale_monitor_rect() {
        let rect = MonitorRect {
            top: 100,
            left: 200,
            width: 1600,
            height: 900,
        };

        // Scale 1080p -> 1440p (1.333x)
        let scaled = scale_monitor_rect(&rect, 1.333, 1.333);
        
        assert_eq!(scaled.top, 133); // 100 * 1.333 = 133.3 -> 133
        assert_eq!(scaled.left, 267); // 200 * 1.333 = 266.6 -> 267
        assert_eq!(scaled.width, 2133); // 1600 * 1.333 = 2132.8 -> 2133
        assert_eq!(scaled.height, 1200); // 900 * 1.333 = 1199.7 -> 1200
    }

    // Property 9: Reversible scaling to base
    // Validates: Requirement 25.4
    #[test]
    fn test_reversible_scaling() {
        let config = create_test_config();
        let scaler = ResolutionScaler::new(&config);
        
        let mut scaled_config = config.clone();
        
        // Scale to 1440p
        scaler.scale_to(&mut scaled_config, "2560x1440").unwrap();
        
        // Values should be scaled
        assert_ne!(scaled_config.monitor.width, config.monitor.width);
        assert_ne!(scaled_config.min_height, config.min_height);
        
        // Restore to base
        scaler.restore_base(&mut scaled_config);
        
        // Should match original values
        assert_eq!(scaled_config.resolution, config.resolution);
        assert_eq!(scaled_config.monitor.top, config.monitor.top);
        assert_eq!(scaled_config.monitor.left, config.monitor.left);
        assert_eq!(scaled_config.monitor.width, config.monitor.width);
        assert_eq!(scaled_config.monitor.height, config.monitor.height);
        assert_eq!(scaled_config.resolution_downscale, config.resolution_downscale);
        assert_eq!(scaled_config.min_height, config.min_height);
        assert_eq!(scaled_config.max_height, config.max_height);
    }

    // Property 32: Downscale proportionality
    // Validates: Requirement 5.3, 25.2
    #[test]
    fn test_downscale_proportionality() {
        let config = create_test_config();
        let scaler = ResolutionScaler::new(&config);
        
        let mut scaled_config = config.clone();
        
        // Scale from 1080p to 1440p (1.333x vertical)
        scaler.scale_to(&mut scaled_config, "2560x1440").unwrap();
        
        // Downscale should be proportional
        let expected_downscale = f32::max(0.1, f32::min(1.0, 0.8 * 1.333));
        assert!((scaled_config.resolution_downscale - expected_downscale).abs() < 0.01);
    }

    // Property 35: Resolution scaling proportionality
    // Validates: Requirement 25.2
    #[test]
    fn test_resolution_scaling_proportionality() {
        let config = create_test_config();
        let scaler = ResolutionScaler::new(&config);
        
        let mut scaled_config = config.clone();
        
        // Scale from 1080p (1920x1080) to 4K (3840x2160) - exactly 2x
        scaler.scale_to(&mut scaled_config, "3840x2160").unwrap();
        
        // All dimensions should be 2x
        assert_eq!(scaled_config.monitor.top, 200); // 100 * 2
        assert_eq!(scaled_config.monitor.left, 400); // 200 * 2
        assert_eq!(scaled_config.monitor.width, 3200); // 1600 * 2
        assert_eq!(scaled_config.monitor.height, 1800); // 900 * 2
        assert_eq!(scaled_config.min_height, 40); // 20 * 2
        assert_eq!(scaled_config.max_height, 100); // 50 * 2
    }

    #[test]
    fn test_scale_to_invalid_resolution() {
        let config = create_test_config();
        let scaler = ResolutionScaler::new(&config);
        
        let mut scaled_config = config.clone();
        
        let result = scaler.scale_to(&mut scaled_config, "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_scale_with_monitor2() {
        let mut config = create_test_config();
        config.monitor2_enabled = true;
        config.monitor2_top = 50;
        config.monitor2_left = 100;
        config.monitor2_width = 800;
        config.monitor2_height = 450;
        
        let scaler = ResolutionScaler::new(&config);
        let mut scaled_config = config.clone();
        
        // Scale to 4K (2x)
        scaler.scale_to(&mut scaled_config, "3840x2160").unwrap();
        
        // monitor2 should also be scaled
        assert_eq!(scaled_config.monitor2_top, 100); // 50 * 2
        assert_eq!(scaled_config.monitor2_left, 200); // 100 * 2
        assert_eq!(scaled_config.monitor2_width, 1600); // 800 * 2
        assert_eq!(scaled_config.monitor2_height, 900); // 450 * 2
    }

    #[test]
    fn test_update_base_values() {
        let config = create_test_config();
        let mut scaler = ResolutionScaler::new(&config);
        
        // Modify config
        let mut modified_config = config.clone();
        modified_config.monitor.width = 1200;
        modified_config.min_height = 25;
        
        // Update scaler base
        scaler.update_base(&modified_config);
        
        // Restore should now use new base
        let mut test_config = create_test_config();
        test_config.monitor.width = 9999; // Set to something different
        
        scaler.restore_base(&mut test_config);
        
        assert_eq!(test_config.monitor.width, 1200); // Should restore to new base
        assert_eq!(test_config.min_height, 25);
    }

    #[test]
    fn test_supported_resolutions() {
        assert!(SUPPORTED_RESOLUTIONS.contains(&"1280x720"));
        assert!(SUPPORTED_RESOLUTIONS.contains(&"1920x1080"));
        assert!(SUPPORTED_RESOLUTIONS.contains(&"2560x1440"));
        assert!(SUPPORTED_RESOLUTIONS.contains(&"3840x2160"));
        assert!(SUPPORTED_RESOLUTIONS.contains(&"5120x2880"));
    }

    #[test]
    fn test_downscale_clamping() {
        let mut config = create_test_config();
        config.resolution_downscale = 0.9; // High base value
        config.resolution = "1920x1080".to_string();
        
        let scaler = ResolutionScaler::new(&config);
        let mut scaled_config = config.clone();
        
        // Scale to 4K (2x) - would result in 1.8 downscale without clamping
        scaler.scale_to(&mut scaled_config, "3840x2160").unwrap();
        
        // Should be clamped to 1.0
        assert_eq!(scaled_config.resolution_downscale, 1.0);
    }
}
