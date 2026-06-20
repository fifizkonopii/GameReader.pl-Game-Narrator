//! Dual capture region management.
//!
//! This module handles switching between two capture regions (monitor and monitor2)
//! and manages the active region state.

use crate::config::{AppConfig, MonitorRect};

/// Active capture region selector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveRegion {
    /// Primary monitor region
    Monitor1,
    /// Secondary monitor region (monitor2)
    Monitor2,
}

/// Dual region manager
#[derive(Debug, Clone)]
pub struct RegionManager {
    /// Currently active region
    active_region: ActiveRegion,
    
    /// Monitor2 enabled flag
    monitor2_enabled: bool,
}

impl RegionManager {
    /// Creates a new region manager
    pub fn new(monitor2_enabled: bool) -> Self {
        Self {
            active_region: ActiveRegion::Monitor1,
            monitor2_enabled,
        }
    }

    /// Returns the currently active region
    pub fn active_region(&self) -> ActiveRegion {
        self.active_region
    }

    /// Checks if monitor2 is enabled
    pub fn is_monitor2_enabled(&self) -> bool {
        self.monitor2_enabled
    }

    /// Updates monitor2 enabled state
    pub fn set_monitor2_enabled(&mut self, enabled: bool) {
        self.monitor2_enabled = enabled;
        
        // If disabling monitor2 while it's active, switch to monitor1
        if !enabled && self.active_region == ActiveRegion::Monitor2 {
            self.active_region = ActiveRegion::Monitor1;
        }
    }

    /// Switches to the specified region
    pub fn switch_to(&mut self, region: ActiveRegion) -> Result<(), String> {
        if region == ActiveRegion::Monitor2 && !self.monitor2_enabled {
            return Err("Cannot switch to Monitor2: it is not enabled".to_string());
        }
        
        self.active_region = region;
        Ok(())
    }

    /// Toggles between monitor1 and monitor2
    pub fn toggle(&mut self) -> Result<ActiveRegion, String> {
        if !self.monitor2_enabled {
            return Err("Cannot toggle: Monitor2 is not enabled".to_string());
        }
        
        self.active_region = match self.active_region {
            ActiveRegion::Monitor1 => ActiveRegion::Monitor2,
            ActiveRegion::Monitor2 => ActiveRegion::Monitor1,
        };
        
        Ok(self.active_region)
    }

    /// Gets the active monitor rect from config
    pub fn get_active_rect(&self, config: &AppConfig) -> MonitorRect {
        match self.active_region {
            ActiveRegion::Monitor1 => config.monitor,
            ActiveRegion::Monitor2 => MonitorRect {
                top: config.monitor2_top,
                left: config.monitor2_left,
                width: config.monitor2_width,
                height: config.monitor2_height,
            },
        }
    }

    /// Resets to monitor1 (used on initialization or error recovery)
    pub fn reset(&mut self) {
        self.active_region = ActiveRegion::Monitor1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_manager_new() {
        let manager = RegionManager::new(false);
        assert_eq!(manager.active_region(), ActiveRegion::Monitor1);
        assert!(!manager.is_monitor2_enabled());
    }

    #[test]
    fn test_switch_to_monitor2() {
        let mut manager = RegionManager::new(true);
        
        manager.switch_to(ActiveRegion::Monitor2).unwrap();
        assert_eq!(manager.active_region(), ActiveRegion::Monitor2);
    }

    #[test]
    fn test_switch_to_disabled_monitor2() {
        let mut manager = RegionManager::new(false);
        
        let result = manager.switch_to(ActiveRegion::Monitor2);
        assert!(result.is_err());
        assert_eq!(manager.active_region(), ActiveRegion::Monitor1);
    }

    #[test]
    fn test_toggle_between_regions() {
        let mut manager = RegionManager::new(true);
        
        // Start at monitor1
        assert_eq!(manager.active_region(), ActiveRegion::Monitor1);
        
        // Toggle to monitor2
        let region = manager.toggle().unwrap();
        assert_eq!(region, ActiveRegion::Monitor2);
        assert_eq!(manager.active_region(), ActiveRegion::Monitor2);
        
        // Toggle back to monitor1
        let region = manager.toggle().unwrap();
        assert_eq!(region, ActiveRegion::Monitor1);
        assert_eq!(manager.active_region(), ActiveRegion::Monitor1);
    }

    #[test]
    fn test_toggle_when_monitor2_disabled() {
        let mut manager = RegionManager::new(false);
        
        let result = manager.toggle();
        assert!(result.is_err());
        assert_eq!(manager.active_region(), ActiveRegion::Monitor1);
    }

    #[test]
    fn test_disable_monitor2_while_active() {
        let mut manager = RegionManager::new(true);
        
        // Switch to monitor2
        manager.switch_to(ActiveRegion::Monitor2).unwrap();
        assert_eq!(manager.active_region(), ActiveRegion::Monitor2);
        
        // Disable monitor2 - should auto-switch to monitor1
        manager.set_monitor2_enabled(false);
        assert_eq!(manager.active_region(), ActiveRegion::Monitor1);
        assert!(!manager.is_monitor2_enabled());
    }

    #[test]
    fn test_get_active_rect() {
        let mut config = AppConfig::default();
        config.monitor = MonitorRect {
            top: 100,
            left: 200,
            width: 1600,
            height: 900,
        };
        config.monitor2_top = 50;
        config.monitor2_left = 100;
        config.monitor2_width = 800;
        config.monitor2_height = 450;
        
        let mut manager = RegionManager::new(true);
        
        // Monitor1 active
        let rect = manager.get_active_rect(&config);
        assert_eq!(rect.top, 100);
        assert_eq!(rect.left, 200);
        assert_eq!(rect.width, 1600);
        assert_eq!(rect.height, 900);
        
        // Switch to monitor2
        manager.switch_to(ActiveRegion::Monitor2).unwrap();
        let rect = manager.get_active_rect(&config);
        assert_eq!(rect.top, 50);
        assert_eq!(rect.left, 100);
        assert_eq!(rect.width, 800);
        assert_eq!(rect.height, 450);
    }

    #[test]
    fn test_reset() {
        let mut manager = RegionManager::new(true);
        
        manager.switch_to(ActiveRegion::Monitor2).unwrap();
        assert_eq!(manager.active_region(), ActiveRegion::Monitor2);
        
        manager.reset();
        assert_eq!(manager.active_region(), ActiveRegion::Monitor1);
    }

    #[test]
    fn test_enable_monitor2_after_creation() {
        let mut manager = RegionManager::new(false);
        assert!(!manager.is_monitor2_enabled());
        
        manager.set_monitor2_enabled(true);
        assert!(manager.is_monitor2_enabled());
        
        // Now should be able to switch
        manager.switch_to(ActiveRegion::Monitor2).unwrap();
        assert_eq!(manager.active_region(), ActiveRegion::Monitor2);
    }
}
