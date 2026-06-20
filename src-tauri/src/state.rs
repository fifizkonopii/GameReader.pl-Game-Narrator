use parking_lot::RwLock;
use std::sync::Arc;
use crate::config::{AppConfig, RuntimeState};

/// Shared application state
/// Uses Arc<RwLock<...>> for thread-safe shared access
/// Read operations use read() lock, write operations use write() lock
pub struct AppState {
    /// Serializable configuration (saved to/loaded from presets)
    pub config: Arc<RwLock<AppConfig>>,
    
    /// Runtime state (not saved to presets)
    pub runtime: Arc<RwLock<RuntimeState>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: Arc::new(RwLock::new(AppConfig::default())),
            runtime: Arc::new(RwLock::new(RuntimeState::default())),
        }
    }
    
    /// Create state with custom config
    pub fn with_config(config: AppConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            runtime: Arc::new(RwLock::new(RuntimeState::default())),
        }
    }
    
    /// Get a snapshot of the current config
    pub fn get_config_snapshot(&self) -> AppConfig {
        self.config.read().clone()
    }
    
    /// Get a snapshot of the current runtime state
    pub fn get_runtime_snapshot(&self) -> RuntimeState {
        self.runtime.read().clone()
    }
    
    /// Update config with new values
    pub fn update_config<F>(&self, f: F)
    where
        F: FnOnce(&mut AppConfig),
    {
        let mut config = self.config.write();
        f(&mut config);
    }
    
    /// Update runtime state with new values
    pub fn update_runtime<F>(&self, f: F)
    where
        F: FnOnce(&mut RuntimeState),
    {
        let mut runtime = self.runtime.write();
        f(&mut runtime);
    }
    
    /// Replace entire config (used when loading preset)
    pub fn replace_config(&self, new_config: AppConfig) {
        let mut config = self.config.write();
        *config = new_config;
    }
    
    /// Reset to default configuration
    pub fn reset_config(&self) {
        let mut config = self.config.write();
        *config = AppConfig::default();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        let config = state.get_config_snapshot();
        assert_eq!(config.resolution, "1920x1080");
    }
    
    #[test]
    fn test_config_update() {
        let state = AppState::new();
        
        state.update_config(|config| {
            config.resolution = "2560x1440".to_string();
        });
        
        let config = state.get_config_snapshot();
        assert_eq!(config.resolution, "2560x1440");
    }
    
    #[test]
    fn test_runtime_update() {
        let state = AppState::new();
        
        state.update_runtime(|runtime| {
            runtime.capture_enabled = true;
            runtime.active_monitor = 2;
        });
        
        let runtime = state.get_runtime_snapshot();
        assert_eq!(runtime.capture_enabled, true);
        assert_eq!(runtime.active_monitor, 2);
    }
    
    #[test]
    fn test_config_replace() {
        let state = AppState::new();
        
        let mut new_config = AppConfig::default();
        new_config.resolution = "3840x2160".to_string();
        new_config.capture_interval = 1.0;
        
        state.replace_config(new_config);
        
        let config = state.get_config_snapshot();
        assert_eq!(config.resolution, "3840x2160");
        assert_eq!(config.capture_interval, 1.0);
    }
    
    #[test]
    fn test_concurrent_access() {
        use std::thread;
        
        let state = Arc::new(AppState::new());
        let mut handles = vec![];
        
        // Spawn multiple threads that read config
        for _ in 0..5 {
            let state_clone = Arc::clone(&state);
            let handle = thread::spawn(move || {
                let config = state_clone.get_config_snapshot();
                assert!(!config.resolution.is_empty());
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
    }
}
