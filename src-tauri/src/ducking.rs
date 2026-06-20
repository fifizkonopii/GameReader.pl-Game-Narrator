//! Audio volume ducking using Windows WASAPI.
//!
//! This module implements automatic volume reduction for other audio sessions
//! when dialog audio is playing, improving dialog clarity.

use std::collections::HashMap;
use tracing::{debug, warn};

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Media::Audio::*,
    Win32::System::Com::*,
};

/// Volume controller for managing audio session volumes via WASAPI.
#[derive(Debug)]
pub struct VolumeController {
    /// Original volume levels for each session (session_id -> volume)
    original_volumes: HashMap<String, f32>,
    
    /// Current ducking state
    is_ducked: bool,
    
    /// Current process ID (to exclude from ducking)
    current_pid: u32,
    
    /// Target process name to duck (e.g. "GTA-SA.exe")
    target_process_name: Option<String>,
}

impl VolumeController {
    /// Creates a new volume controller.
    ///
    /// # Parameters
    /// - `target_process_name`: Optional process name to duck (e.g. "GTA-SA.exe").
    ///                          If None, ducks all processes except current.
    pub fn new() -> Self {
        Self {
            original_volumes: HashMap::new(),
            is_ducked: false,
            current_pid: std::process::id(),
            target_process_name: None,
        }
    }
    
    /// Creates a new volume controller targeting a specific process.
    pub fn new_with_target(target_process_name: String) -> Self {
        Self {
            original_volumes: HashMap::new(),
            is_ducked: false,
            current_pid: std::process::id(),
            target_process_name: Some(target_process_name),
        }
    }
    
    /// Sets the target process name to duck.
    pub fn set_target_process(&mut self, name: String) {
        self.target_process_name = Some(name);
    }
    
    /// Clears the target process (will duck all processes).
    pub fn clear_target_process(&mut self) {
        self.target_process_name = None;
    }

    /// Reduces volume of all audio sessions (except current process) by the specified amount.
    ///
    /// # Parameters
    /// - `reduction_level`: Volume reduction factor (0.0 = no reduction, 1.0 = mute)
    ///
    /// # Returns
    /// - `Ok(())` on success
    /// - `Err(String)` if ducking fails (non-fatal, audio will play without ducking)
    #[cfg(windows)]
    pub fn duck(&mut self, reduction_level: f32) -> std::result::Result<(), String> {
        if self.is_ducked {
            debug!("Already ducked, skipping");
            return Ok(());
        }

        debug!("Ducking audio sessions with reduction level: {}", reduction_level);

        // Initialize COM for this thread
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|e| format!("Failed to initialize COM: {}", e))?;
        }

        match self.duck_internal(reduction_level) {
            Ok(()) => {
                self.is_ducked = true;
                Ok(())
            }
            Err(e) => {
                // Uninitialize COM on failure
                unsafe { CoUninitialize() };
                Err(e)
            }
        }
    }

    #[cfg(windows)]
    fn duck_internal(&mut self, reduction_level: f32) -> std::result::Result<(), String> {
        unsafe {
            // Create device enumerator
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(
                &MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            )
            .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

            // Get default audio endpoint
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| format!("Failed to get default audio endpoint: {}", e))?;

            // Get session manager
            let session_manager: IAudioSessionManager2 = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Failed to activate session manager: {}", e))?;

            // Get session enumerator
            let session_enum = session_manager
                .GetSessionEnumerator()
                .map_err(|e| format!("Failed to get session enumerator: {}", e))?;

            // Get session count
            let session_count = session_enum
                .GetCount()
                .map_err(|e| format!("Failed to get session count: {}", e))?;

            debug!("Found {} audio sessions", session_count);

            let mut ducked_count = 0;

            // Iterate through sessions
            for i in 0..session_count {
                let session_control = match session_enum.GetSession(i) {
                    Ok(s) => s,
                    Err(e) => {
                        debug!("Failed to get session {}: {}", i, e);
                        continue;
                    }
                };

                // Get session control 2 for process ID
                let session_control2: IAudioSessionControl2 = match session_control.cast() {
                    Ok(s) => s,
                    Err(e) => {
                        debug!("Failed to cast session {} to IAudioSessionControl2: {}", i, e);
                        continue;
                    }
                };

                // Get process ID
                let process_id = match session_control2.GetProcessId() {
                    Ok(pid) => pid,
                    Err(e) => {
                        debug!("Failed to get process ID for session {}: {}", i, e);
                        continue;
                    }
                };

                // Skip current process (don't duck our own audio)
                if process_id == self.current_pid {
                    debug!("Skipping session {} (current process)", i);
                    continue;
                }

                // If target process is specified, only duck that specific process
                if let Some(ref target_name) = self.target_process_name {
                    // Get process name
                    let process_name = match get_process_name(process_id) {
                        Some(name) => name,
                        None => {
                            debug!("Failed to get process name for PID {}, skipping", process_id);
                            continue;
                        }
                    };

                    // Check if this is the target process (case-insensitive comparison)
                    if !process_name.eq_ignore_ascii_case(target_name) {
                        debug!("Skipping session {} (PID {}, process: {}) - not target", i, process_id, process_name);
                        continue;
                    }

                    debug!("Found target process {} (PID {})", process_name, process_id);
                }

                // Get simple audio volume interface
                let simple_volume: ISimpleAudioVolume = match session_control2.cast() {
                    Ok(v) => v,
                    Err(e) => {
                        debug!("Failed to get ISimpleAudioVolume for session {}: {}", i, e);
                        continue;
                    }
                };

                // Get session instance ID for tracking
                let session_id = match session_control2.GetSessionInstanceIdentifier() {
                    Ok(id) => id.to_string().unwrap_or_else(|_| format!("session_{}", i)),
                    Err(_) => format!("session_{}", i),
                };

                // Get current volume
                let current_volume = match simple_volume.GetMasterVolume() {
                    Ok(v) => v,
                    Err(e) => {
                        debug!("Failed to get volume for session {}: {}", i, e);
                        continue;
                    }
                };

                // Store original volume
                self.original_volumes.insert(session_id.clone(), current_volume);

                // Calculate ducked volume
                let ducked_volume = current_volume * (1.0 - reduction_level);

                // Set ducked volume
                match simple_volume.SetMasterVolume(ducked_volume, std::ptr::null()) {
                    Ok(()) => {
                        debug!(
                            "Ducked session {} (PID {}): {} -> {}",
                            session_id, process_id, current_volume, ducked_volume
                        );
                        ducked_count += 1;
                    }
                    Err(e) => {
                        debug!("Failed to set volume for session {}: {}", i, e);
                        continue;
                    }
                }
            }

            debug!("Successfully ducked {} audio sessions", ducked_count);
            Ok(())
        }
    }

    /// Restores original volume levels for all ducked audio sessions.
    #[cfg(windows)]
    pub fn restore(&mut self) -> std::result::Result<(), String> {
        if !self.is_ducked {
            debug!("Not ducked, skipping restore");
            return Ok(());
        }

        debug!("Restoring original audio volumes");

        match self.restore_internal() {
            Ok(()) => {
                self.is_ducked = false;
                self.original_volumes.clear();
                
                // Uninitialize COM
                unsafe { CoUninitialize() };
                
                Ok(())
            }
            Err(e) => {
                // Uninitialize COM even on failure
                unsafe { CoUninitialize() };
                Err(e)
            }
        }
    }

    #[cfg(windows)]
    fn restore_internal(&mut self) -> std::result::Result<(), String> {
        unsafe {
            // Create device enumerator
            let enumerator: IMMDeviceEnumerator = CoCreateInstance(
                &MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            )
            .map_err(|e| format!("Failed to create device enumerator: {}", e))?;

            // Get default audio endpoint
            let device = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .map_err(|e| format!("Failed to get default audio endpoint: {}", e))?;

            // Get session manager
            let session_manager: IAudioSessionManager2 = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| format!("Failed to activate session manager: {}", e))?;

            // Get session enumerator
            let session_enum = session_manager
                .GetSessionEnumerator()
                .map_err(|e| format!("Failed to get session enumerator: {}", e))?;

            // Get session count
            let session_count = session_enum
                .GetCount()
                .map_err(|e| format!("Failed to get session count: {}", e))?;

            let mut restored_count = 0;

            // Iterate through sessions
            for i in 0..session_count {
                let session_control = match session_enum.GetSession(i) {
                    Ok(s) => s,
                    Err(e) => {
                        debug!("Failed to get session {}: {}", i, e);
                        continue;
                    }
                };

                // Get session control 2
                let session_control2: IAudioSessionControl2 = match session_control.cast() {
                    Ok(s) => s,
                    Err(e) => {
                        debug!("Failed to cast session {} to IAudioSessionControl2: {}", i, e);
                        continue;
                    }
                };

                // Get session instance ID
                let session_id = match session_control2.GetSessionInstanceIdentifier() {
                    Ok(id) => id.to_string().unwrap_or_else(|_| format!("session_{}", i)),
                    Err(_) => format!("session_{}", i),
                };

                // Check if we have original volume for this session
                let original_volume = match self.original_volumes.get(&session_id) {
                    Some(v) => *v,
                    None => {
                        debug!("No original volume stored for session {}", session_id);
                        continue;
                    }
                };

                // Get simple audio volume interface
                let simple_volume: ISimpleAudioVolume = match session_control2.cast() {
                    Ok(v) => v,
                    Err(e) => {
                        debug!("Failed to get ISimpleAudioVolume for session {}: {}", i, e);
                        continue;
                    }
                };

                // Restore original volume
                match simple_volume.SetMasterVolume(original_volume, std::ptr::null()) {
                    Ok(()) => {
                        debug!("Restored session {} to volume {}", session_id, original_volume);
                        restored_count += 1;
                    }
                    Err(e) => {
                        debug!("Failed to restore volume for session {}: {}", i, e);
                        continue;
                    }
                }
            }

            debug!("Successfully restored {} audio sessions", restored_count);
            Ok(())
        }
    }

    /// Non-Windows stub for duck
    #[cfg(not(windows))]
    pub fn duck(&mut self, _reduction_level: f32) -> std::result::Result<(), String> {
        warn!("Audio ducking is only supported on Windows");
        Ok(())
    }

    /// Non-Windows stub for restore
    #[cfg(not(windows))]
    pub fn restore(&mut self) -> std::result::Result<(), String> {
        warn!("Audio ducking is only supported on Windows");
        Ok(())
    }

    /// Returns true if currently ducked.
    pub fn is_ducked(&self) -> bool {
        self.is_ducked
    }
}

impl Drop for VolumeController {
    fn drop(&mut self) {
        // Ensure volumes are restored when controller is dropped
        if self.is_ducked {
            if let Err(e) = self.restore() {
                warn!("Failed to restore volumes in Drop: {}", e);
            }
        }
    }
}

/// Helper function to get process name from PID (Windows only).
#[cfg(windows)]
fn get_process_name(pid: u32) -> Option<String> {
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_WIN32};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
    use windows::core::PWSTR;
    
    unsafe {
        // Open process with query permission
        let process_handle: HANDLE = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return None,
        };
        
        // Query process image name
        let mut buffer = vec![0u16; MAX_PATH as usize];
        let mut size = buffer.len() as u32;
        
        let pwstr = PWSTR::from_raw(buffer.as_mut_ptr());
        let result = QueryFullProcessImageNameW(process_handle, PROCESS_NAME_WIN32, pwstr, &mut size);
        
        // Close handle
        let _ = CloseHandle(process_handle);
        
        if result.is_ok() {
            // Convert to String and extract filename
            let path = String::from_utf16_lossy(&buffer[..size as usize]);
            let filename = std::path::Path::new(&path)
                .file_name()?
                .to_str()?
                .to_string();
            Some(filename)
        } else {
            None
        }
    }
}

/// Non-Windows stub for get_process_name
#[cfg(not(windows))]
fn get_process_name(_pid: u32) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_controller_creation() {
        let controller = VolumeController::new();
        assert!(!controller.is_ducked());
        assert_eq!(controller.original_volumes.len(), 0);
    }

    #[test]
    fn test_volume_controller_state() {
        let mut controller = VolumeController::new();
        
        // Initial state
        assert!(!controller.is_ducked());
        
        // Note: We can't fully test duck/restore without real audio sessions
        // These would need integration tests with actual audio playing
    }

    #[test]
    fn test_current_pid() {
        let controller = VolumeController::new();
        assert_eq!(controller.current_pid, std::process::id());
    }
}
