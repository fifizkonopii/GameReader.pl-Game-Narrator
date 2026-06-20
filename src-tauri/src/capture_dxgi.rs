//! DXGI Desktop Duplication API backend implementation.
//!
//! DXGI Desktop Duplication is the legacy API for screen capture on Windows 8+.
//! It's used as a fallback when WGC is not available or fails to initialize.

use crate::capture::{CaptureError, CaptureRegion, CaptureSource, Frame};
use std::sync::{Arc, Mutex};

/// DXGI Desktop Duplication backend using xcap library.
///
/// This implementation uses the xcap library which wraps platform-specific
/// capture APIs including DXGI on Windows.
///
/// # Platform Requirements
/// - Windows 8 or later
/// - DirectX 11 compatible graphics adapter
pub struct DxgiCapture {
    inner: Arc<Mutex<DxgiCaptureInner>>,
}

struct DxgiCaptureInner {
    monitor: Option<xcap::Monitor>,
    monitor_index: Option<usize>,
}

impl DxgiCapture {
    /// Creates a new DXGI capture instance.
    pub fn new() -> Result<Self, CaptureError> {
        Ok(Self {
            inner: Arc::new(Mutex::new(DxgiCaptureInner {
                monitor: None,
                monitor_index: None,
            })),
        })
    }

    /// Checks if DXGI Desktop Duplication is supported.
    fn check_support() -> bool {
        // xcap supports Windows, macOS, and Linux
        // On Windows it uses DXGI under the hood
        cfg!(target_os = "windows")
    }
}

impl Default for DxgiCapture {
    fn default() -> Self {
        Self::new().expect("Failed to create DXGI capture")
    }
}

impl CaptureSource for DxgiCapture {
    fn bind_monitor(&mut self, monitor_index: usize) -> Result<(), CaptureError> {
        let mut inner = self.inner.lock().unwrap();
        
        // Get all monitors
        let monitors = xcap::Monitor::all()
            .map_err(|e| CaptureError::DxgiInitFailed { 
                reason: format!("Failed to enumerate monitors: {}", e) 
            })?;
        
        // Find monitor by index
        let monitor = monitors.into_iter()
            .nth(monitor_index)
            .ok_or_else(|| CaptureError::MonitorNotFound { monitor_index })?;
        
        tracing::info!(
            "DXGI bound to monitor {} ({}x{} at {},{}) - {}", 
            monitor_index,
            monitor.width(),
            monitor.height(),
            monitor.x(),
            monitor.y(),
            monitor.name()
        );
        
        inner.monitor = Some(monitor);
        inner.monitor_index = Some(monitor_index);
        
        Ok(())
    }

    fn grab(&mut self, region: &CaptureRegion) -> Result<Frame, CaptureError> {
        let inner = self.inner.lock().unwrap();
        
        let monitor = inner.monitor.as_ref()
            .ok_or_else(|| CaptureError::Other("DXGI not initialized - call bind_monitor first".into()))?;
        
        if !region.is_valid() {
            return Err(CaptureError::InvalidRegion {
                details: format!("Invalid region: {}x{} at ({}, {})", 
                    region.width, region.height, region.left, region.top),
            });
        }
        
        // Validate region is within monitor bounds
        let monitor_width = monitor.width() as u32;
        let monitor_height = monitor.height() as u32;
        
        if region.left + region.width > monitor_width || 
           region.top + region.height > monitor_height {
            return Err(CaptureError::InvalidRegion {
                details: format!(
                    "Region {}x{} at ({},{}) exceeds monitor bounds {}x{}",
                    region.width, region.height, region.left, region.top,
                    monitor_width, monitor_height
                ),
            });
        }
        
        // Capture full monitor screen
        let image = monitor.capture_image()
            .map_err(|e| {
                // Check for device lost errors
                let err_str = e.to_string();
                if err_str.contains("ACCESS_LOST") || err_str.contains("DEVICE_REMOVED") {
                    CaptureError::DeviceLost
                } else {
                    CaptureError::GpuCopyFailed { 
                        reason: format!("Failed to capture screen: {}", e) 
                    }
                }
            })?;
        
        // Extract the requested region from the captured image
        // xcap returns image::RgbaImage
        let width = image.width();
        let height = image.height();
        
        // Validate captured image size matches monitor
        if width != monitor_width || height != monitor_height {
            return Err(CaptureError::GpuCopyFailed {
                reason: format!(
                    "Captured image size {}x{} doesn't match monitor {}x{}",
                    width, height, monitor_width, monitor_height
                ),
            });
        }
        
        // Crop to requested region
        let cropped = image::imageops::crop_imm(
            &image,
            region.left,
            region.top,
            region.width,
            region.height
        ).to_image();
        
        // Convert RGBA to BGRA
        let mut bgra_data = Vec::with_capacity((region.width * region.height * 4) as usize);
        for pixel in cropped.pixels() {
            bgra_data.push(pixel[2]); // B
            bgra_data.push(pixel[1]); // G  
            bgra_data.push(pixel[0]); // R
            bgra_data.push(pixel[3]); // A
        }
        
        let stride = region.width as usize * 4;
        Frame::from_data(region.width, region.height, stride, bgra_data)
    }

    fn is_supported(&self) -> bool {
        Self::check_support()
    }

    fn backend_name(&self) -> &'static str {
        "DXGI Desktop Duplication (via xcap)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dxgi_creation() {
        let capture = DxgiCapture::new();
        assert!(capture.is_ok());
    }

    #[test]
    fn test_dxgi_is_supported() {
        let capture = DxgiCapture::new().unwrap();
        // Should be true on Windows, false elsewhere
        assert_eq!(capture.is_supported(), cfg!(target_os = "windows"));
    }

    #[test]
    fn test_dxgi_backend_name() {
        let capture = DxgiCapture::new().unwrap();
        assert!(capture.backend_name().contains("DXGI"));
    }

    #[test]
    #[ignore] // Requires actual display
    fn test_dxgi_bind_monitor() {
        let mut capture = DxgiCapture::new().unwrap();
        let result = capture.bind_monitor(0);
        
        if result.is_ok() {
            println!("Successfully bound to monitor 0");
        } else {
            println!("Failed to bind monitor (expected if no display): {:?}", result);
        }
    }

    #[test]
    fn test_dxgi_bind_invalid_monitor() {
        let mut capture = DxgiCapture::new().unwrap();
        let result = capture.bind_monitor(999);
        // Should fail - no such monitor
        assert!(result.is_err());
    }

    #[test]
    fn test_dxgi_grab_without_bind() {
        let mut capture = DxgiCapture::new().unwrap();
        let region = CaptureRegion::new(0, 0, 100, 100);
        let result = capture.grab(&region);
        assert!(result.is_err());
    }

    #[test]
    #[ignore] // Requires actual display
    fn test_dxgi_grab_real() {
        let mut capture = DxgiCapture::new().unwrap();
        
        if capture.bind_monitor(0).is_err() {
            println!("No display available, skipping test");
            return;
        }
        
        // Try to capture a small region
        let region = CaptureRegion::new(0, 0, 100, 100);
        let result = capture.grab(&region);
        
        match result {
            Ok(frame) => {
                assert_eq!(frame.width, 100);
                assert_eq!(frame.height, 100);
                assert_eq!(frame.bgra.len(), 100 * 100 * 4);
                println!("Successfully captured 100x100 frame");
            }
            Err(e) => {
                println!("Capture failed: {:?}", e);
            }
        }
    }
}
