//! Single-instance enforcement using Windows named mutex.
//!
//! This module ensures only one instance of the application can run at a time.
//! Uses a Windows named mutex to detect existing instances.

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::CreateMutexW;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::GetLastError;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

/// Single instance guard - holds the mutex handle
/// The inner HANDLE is wrapped to ensure it's properly cleaned up
pub struct SingleInstanceGuard {
    #[cfg(target_os = "windows")]
    mutex_handle: Option<MutexHandle>,
}

#[cfg(target_os = "windows")]
struct MutexHandle(HANDLE);

#[cfg(target_os = "windows")]
unsafe impl Send for MutexHandle {}
#[cfg(target_os = "windows")]
unsafe impl Sync for MutexHandle {}

#[cfg(target_os = "windows")]
impl Drop for MutexHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

impl SingleInstanceGuard {
    /// Creates a new single instance guard.
    /// 
    /// Returns Ok(guard) if this is the only instance.
    /// Returns Err(message) if another instance is already running.
    #[cfg(target_os = "windows")]
    pub fn new(mutex_name: &str) -> Result<Self, String> {
        use tracing::{info, error};
        
        // Convert mutex name to wide string
        let wide_name: Vec<u16> = mutex_name.encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        
        unsafe {
            // Try to create the named mutex
            let handle = CreateMutexW(
                None,
                true, // bInitialOwner - we want to own it
                PCWSTR(wide_name.as_ptr()),
            );
            
            match handle {
                Ok(h) if !h.is_invalid() => {
                    // Check if the mutex already existed
                    let last_error = GetLastError();
                    
                    if last_error.0 == 183 { // ERROR_ALREADY_EXISTS
                        // Another instance is already running
                        error!("Another instance of the application is already running");
                        let _ = CloseHandle(h);
                        return Err("Another instance is already running".to_string());
                    }
                    
                    info!("Single-instance mutex acquired");
                    Ok(Self {
                        mutex_handle: Some(MutexHandle(h)),
                    })
                }
                _ => {
                    error!("Failed to create single-instance mutex");
                    Err("Failed to create single-instance mutex".to_string())
                }
            }
        }
    }
    
    /// Creates a new single instance guard (non-Windows platforms).
    /// 
    /// Always succeeds on non-Windows platforms as single-instance enforcement
    /// is Windows-specific.
    #[cfg(not(target_os = "windows"))]
    pub fn new(_mutex_name: &str) -> Result<Self, String> {
        Ok(Self {})
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            use tracing::info;
            
            if let Some(_handle) = self.mutex_handle.take() {
                // MutexHandle's Drop will handle CloseHandle
                info!("Single-instance mutex released");
            }
        }
    }
}

/// Checks if another instance is running and acquires the single-instance lock.
/// 
/// This should be called at application startup before initializing other components.
/// Returns Ok(()) if this is the only instance, Err(message) if another instance exists.
/// 
/// The guard is stored in a static variable to live for the entire application lifetime.
pub fn check_single_instance() -> Result<(), String> {
    use std::sync::Mutex;
    use std::sync::OnceLock;
    
    static INSTANCE_GUARD: OnceLock<Mutex<Option<SingleInstanceGuard>>> = OnceLock::new();
    
    let mutex_name = "Global\\GameReader_SingleInstance_Mutex";
    let guard = SingleInstanceGuard::new(mutex_name)?;
    
    // Store the guard in the static OnceLock
    let mutex = INSTANCE_GUARD.get_or_init(|| Mutex::new(None));
    let mut guard_opt = mutex.lock().unwrap();
    *guard_opt = Some(guard);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_single_instance_guard_creation() {
        // First instance should succeed
        let guard1 = SingleInstanceGuard::new("GameReader_Test_Mutex_1");
        assert!(guard1.is_ok());
    }
    
    #[test]
    #[cfg(target_os = "windows")]
    fn test_second_instance_fails() {
        // First instance should succeed
        let guard1 = SingleInstanceGuard::new("GameReader_Test_Mutex_2");
        assert!(guard1.is_ok());
        
        // Second instance should fail
        let guard2 = SingleInstanceGuard::new("GameReader_Test_Mutex_2");
        assert!(guard2.is_err());
        
        // Drop first guard
        drop(guard1);
        
        // Now a new instance should succeed
        let guard3 = SingleInstanceGuard::new("GameReader_Test_Mutex_2");
        assert!(guard3.is_ok());
    }
    
    #[test]
    fn test_check_single_instance() {
        // This test is more of a smoke test since it uses a global mutex
        // Real testing would require multiple processes
        let _result = check_single_instance();
        
        // On non-Windows platforms, it should always succeed
        #[cfg(not(target_os = "windows"))]
        assert!(_result.is_ok());
        
        // We can't reliably assert on Windows because it depends on whether another instance is running
    }
}
