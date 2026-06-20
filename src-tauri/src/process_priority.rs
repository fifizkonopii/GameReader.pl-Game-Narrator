//! Process priority management for Windows.
//!
//! Sets the process priority to below normal to avoid interfering with
//! game performance and other high-priority applications.

#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};

/// Raise the CURRENT thread's priority (used for the audio worker so playback
/// decoding/time-stretch keeps getting CPU even when OCR saturates the cores).
#[cfg(target_os = "windows")]
pub fn set_current_thread_high() {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_HIGHEST};
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_current_thread_high() {}

/// Sets the current process priority to below normal.
///
/// On Windows, this uses SetPriorityClass with BELOW_NORMAL_PRIORITY_CLASS.
/// On non-Windows platforms, this is a no-op.
///
/// Returns Ok(()) on success, or Err(message) if the operation fails.
/// Failures are logged but not fatal - the application continues with default priority.
#[cfg(target_os = "windows")]
pub fn set_low_priority() -> Result<(), String> {
    use tracing::{info, warn};
    
    unsafe {
        let process_handle = GetCurrentProcess();
        
        let result = SetPriorityClass(process_handle, BELOW_NORMAL_PRIORITY_CLASS);
        
        if result.is_ok() {
            info!("Process priority set to below normal");
            Ok(())
        } else {
            let error_msg = "Failed to set process priority to below normal";
            warn!("{}", error_msg);
            Err(error_msg.to_string())
        }
    }
}

/// Sets the current process priority to below normal (non-Windows platforms).
///
/// This is a no-op on non-Windows platforms as process priority management
/// is platform-specific.
#[cfg(not(target_os = "windows"))]
pub fn set_low_priority() -> Result<(), String> {
    use tracing::info;
    
    info!("Process priority setting skipped (non-Windows platform)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_set_low_priority() {
        // This test verifies that the function doesn't panic
        let result = set_low_priority();
        
        // On Windows, should succeed (unless running in restricted environment)
        // On other platforms, should always succeed (no-op)
        #[cfg(not(target_os = "windows"))]
        assert!(result.is_ok());
        
        // On Windows, we can't assert success because it depends on permissions
        // Just verify it doesn't panic
        #[cfg(target_os = "windows")]
        {
            // The function executed without panicking
            match result {
                Ok(_) => {
                    // Success - priority was set
                }
                Err(e) => {
                    // Failure - but function handled it gracefully
                    println!("Priority setting failed (may be expected): {}", e);
                }
            }
        }
    }
    
    #[test]
    fn test_set_low_priority_multiple_calls() {
        // Calling multiple times should be safe
        let result1 = set_low_priority();
        let result2 = set_low_priority();
        
        // Should not panic on repeated calls
        #[cfg(not(target_os = "windows"))]
        {
            assert!(result1.is_ok());
            assert!(result2.is_ok());
        }
        
        // Just verify no panic on Windows
        #[cfg(target_os = "windows")]
        {
            let _ = result1;
            let _ = result2;
        }
    }
}
