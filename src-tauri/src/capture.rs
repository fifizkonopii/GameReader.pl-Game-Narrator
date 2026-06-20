//! Screen capture module with Windows Graphics Capture (WGC) and DXGI fallback.
//!
//! This module provides a unified interface for capturing screen regions on Windows,
//! with support for both Windows Graphics Capture (preferred) and DXGI Desktop
//! Duplication (fallback) backends.

use thiserror::Error;

/// Represents a rectangular region on the screen to capture.
///
/// Coordinates are in pixels, with (0,0) at the top-left of the monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRegion {
    /// X coordinate of the top-left corner
    pub left: u32,
    /// Y coordinate of the top-left corner
    pub top: u32,
    /// Width of the region in pixels
    pub width: u32,
    /// Height of the region in pixels
    pub height: u32,
}

impl CaptureRegion {
    /// Creates a new capture region.
    pub fn new(left: u32, top: u32, width: u32, height: u32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    /// Returns true if the region has non-zero dimensions.
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Returns the area of the region in pixels.
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// A captured frame with BGRA pixel data.
///
/// Pixels are stored in row-major order with 4 bytes per pixel (BGRA format).
/// The stride may be larger than width * 4 due to alignment requirements.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Width of the frame in pixels
    pub width: u32,
    /// Height of the frame in pixels
    pub height: u32,
    /// Number of bytes per row (may be > width * 4 due to alignment)
    pub stride: usize,
    /// Pixel data in BGRA format (4 bytes per pixel)
    pub bgra: Vec<u8>,
}

impl Frame {
    /// Creates a new frame with pre-allocated pixel buffer.
    pub fn new(width: u32, height: u32, stride: usize) -> Self {
        let buffer_size = stride * height as usize;
        Self {
            width,
            height,
            stride,
            bgra: vec![0u8; buffer_size],
        }
    }

    /// Creates a frame from existing pixel data.
    pub fn from_data(width: u32, height: u32, stride: usize, bgra: Vec<u8>) -> Result<Self, CaptureError> {
        let expected_size = stride * height as usize;
        if bgra.len() != expected_size {
            return Err(CaptureError::InvalidFrameSize {
                expected: expected_size,
                actual: bgra.len(),
            });
        }
        Ok(Self {
            width,
            height,
            stride,
            bgra,
        })
    }

    /// Validates frame structure invariants.
    pub fn validate(&self) -> Result<(), CaptureError> {
        let min_stride = self.width as usize * 4;
        if self.stride < min_stride {
            return Err(CaptureError::InvalidStride {
                width: self.width,
                stride: self.stride,
                min_stride,
            });
        }

        let expected_size = self.stride * self.height as usize;
        if self.bgra.len() != expected_size {
            return Err(CaptureError::InvalidFrameSize {
                expected: expected_size,
                actual: self.bgra.len(),
            });
        }

        Ok(())
    }

    /// Returns a slice of pixel data for a specific row.
    ///
    /// # Panics
    /// Panics if y >= height.
    pub fn row(&self, y: u32) -> &[u8] {
        assert!(y < self.height, "Row index out of bounds");
        let start = y as usize * self.stride;
        let end = start + (self.width as usize * 4);
        &self.bgra[start..end]
    }

    /// Returns total number of pixels.
    pub fn pixel_count(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// Errors that can occur during screen capture operations.
#[derive(Debug, Error)]
pub enum CaptureError {
    /// The capture device was lost (e.g., monitor disconnected, graphics context lost).
    #[error("Capture device lost - reinitialization required")]
    DeviceLost,

    /// The requested capture region is invalid (e.g., out of monitor bounds, zero size).
    #[error("Invalid capture region: {details}")]
    InvalidRegion { details: String },

    /// The monitor/display could not be found or accessed.
    #[error("Monitor not found or inaccessible: {monitor_index}")]
    MonitorNotFound { monitor_index: usize },

    /// Windows Graphics Capture initialization failed.
    #[error("WGC initialization failed: {reason}")]
    WgcInitFailed { reason: String },

    /// DXGI initialization failed.
    #[error("DXGI initialization failed: {reason}")]
    DxgiInitFailed { reason: String },

    /// Frame buffer size mismatch.
    #[error("Invalid frame size: expected {expected} bytes, got {actual} bytes")]
    InvalidFrameSize { expected: usize, actual: usize },

    /// Frame stride is too small for the given width.
    #[error("Invalid stride {stride} for width {width} (minimum {min_stride})")]
    InvalidStride {
        width: u32,
        stride: usize,
        min_stride: usize,
    },

    /// GPU to CPU memory copy failed.
    #[error("Failed to copy frame from GPU: {reason}")]
    GpuCopyFailed { reason: String },

    /// Generic capture error.
    #[error("Capture failed: {0}")]
    Other(String),
}

/// Trait for screen capture backends.
///
/// Implementors provide platform-specific capture functionality with a unified interface.
/// Multiple backends (WGC, DXGI) can implement this trait with automatic fallback.
pub trait CaptureSource: Send {
    /// Binds the capture source to a specific monitor and prepares for capture.
    ///
    /// # Arguments
    /// * `monitor_index` - Zero-based monitor index (0 = primary monitor)
    ///
    /// # Errors
    /// Returns `CaptureError::MonitorNotFound` if the monitor doesn't exist,
    /// or initialization-specific errors for WGC/DXGI setup failures.
    fn bind_monitor(&mut self, monitor_index: usize) -> Result<(), CaptureError>;

    /// Captures a frame from the specified region.
    ///
    /// # Arguments
    /// * `region` - The rectangular region to capture
    ///
    /// # Returns
    /// A `Frame` containing BGRA pixel data for the captured region.
    ///
    /// # Errors
    /// * `CaptureError::InvalidRegion` - Region is outside monitor bounds or has zero size
    /// * `CaptureError::DeviceLost` - Capture device was lost, caller should reinitialize
    /// * `CaptureError::GpuCopyFailed` - Failed to copy frame from GPU to CPU
    fn grab(&mut self, region: &CaptureRegion) -> Result<Frame, CaptureError>;

    /// Returns true if the capture source supports the current system configuration.
    ///
    /// This can be used to check if a backend is available before attempting to use it.
    fn is_supported(&self) -> bool;

    /// Returns a human-readable name for this capture backend.
    fn backend_name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_region_new() {
        let region = CaptureRegion::new(100, 200, 800, 600);
        assert_eq!(region.left, 100);
        assert_eq!(region.top, 200);
        assert_eq!(region.width, 800);
        assert_eq!(region.height, 600);
    }

    #[test]
    fn test_capture_region_is_valid() {
        assert!(CaptureRegion::new(0, 0, 100, 100).is_valid());
        assert!(CaptureRegion::new(100, 100, 1, 1).is_valid());
        assert!(!CaptureRegion::new(0, 0, 0, 100).is_valid());
        assert!(!CaptureRegion::new(0, 0, 100, 0).is_valid());
        assert!(!CaptureRegion::new(0, 0, 0, 0).is_valid());
    }

    #[test]
    fn test_capture_region_area() {
        assert_eq!(CaptureRegion::new(0, 0, 10, 10).area(), 100);
        assert_eq!(CaptureRegion::new(50, 50, 100, 200).area(), 20000);
        assert_eq!(CaptureRegion::new(0, 0, 0, 0).area(), 0);
    }

    #[test]
    fn test_frame_new() {
        let frame = Frame::new(100, 100, 400);
        assert_eq!(frame.width, 100);
        assert_eq!(frame.height, 100);
        assert_eq!(frame.stride, 400);
        assert_eq!(frame.bgra.len(), 40000); // stride * height
    }

    #[test]
    fn test_frame_from_data_valid() {
        let width: u32 = 10;
        let height: u32 = 10;
        let stride: usize = 40; // width * 4
        let data = vec![0u8; stride * height as usize];

        let frame = Frame::from_data(width, height, stride, data).unwrap();
        assert_eq!(frame.width, width);
        assert_eq!(frame.height, height);
        assert_eq!(frame.stride, stride);
        assert_eq!(frame.bgra.len(), stride * height as usize);
    }

    #[test]
    fn test_frame_from_data_invalid_size() {
        let width = 10;
        let height = 10;
        let stride = 40;
        let data = vec![0u8; 100]; // Too small

        let result = Frame::from_data(width, height, stride, data);
        assert!(matches!(result, Err(CaptureError::InvalidFrameSize { .. })));
    }

    #[test]
    fn test_frame_validate_correct() {
        let frame = Frame::new(100, 100, 400);
        assert!(frame.validate().is_ok());
    }

    #[test]
    fn test_frame_validate_invalid_stride() {
        let mut frame = Frame::new(100, 100, 400);
        frame.stride = 300; // Too small for width 100 (needs >= 400)

        let result = frame.validate();
        assert!(matches!(result, Err(CaptureError::InvalidStride { .. })));
    }

    #[test]
    fn test_frame_validate_invalid_buffer_size() {
        let mut frame = Frame::new(100, 100, 400);
        frame.bgra.truncate(1000); // Make buffer too small

        let result = frame.validate();
        assert!(matches!(result, Err(CaptureError::InvalidFrameSize { .. })));
    }

    #[test]
    fn test_frame_row() {
        let width: u32 = 2;
        let height: u32 = 3;
        let stride: usize = 8; // width * 4
        let mut data = vec![0u8; stride * height as usize];

        // Set first pixel of row 0 to red
        data[0] = 0;   // B
        data[1] = 0;   // G
        data[2] = 255; // R
        data[3] = 255; // A

        // Set first pixel of row 1 to green
        data[8] = 0;   // B
        data[9] = 255; // G
        data[10] = 0;  // R
        data[11] = 255; // A

        let frame = Frame::from_data(width, height, stride, data).unwrap();

        let row0 = frame.row(0);
        assert_eq!(row0.len(), 8); // width * 4
        assert_eq!(row0[0..4], [0, 0, 255, 255]); // Red pixel

        let row1 = frame.row(1);
        assert_eq!(row1[0..4], [0, 255, 0, 255]); // Green pixel
    }

    #[test]
    fn test_frame_pixel_count() {
        let frame = Frame::new(100, 50, 400);
        assert_eq!(frame.pixel_count(), 5000);
    }

    #[test]
    #[should_panic(expected = "Row index out of bounds")]
    fn test_frame_row_out_of_bounds() {
        let frame = Frame::new(10, 10, 40);
        frame.row(10); // height is 10, so max index is 9
    }

    // Property 13: Capture frame structure
    // Validates: Requirements 1.2, 1.3
    // For all valid frames, bgra.len() == stride * height
    #[test]
    fn test_property_frame_buffer_size_invariant() {
        let test_cases = vec![
            (10, 10, 40),
            (100, 100, 400),
            (1920, 1080, 7680),
            (640, 480, 2560),
            (1, 1, 4),
        ];

        for (width, height, stride) in test_cases {
            let frame = Frame::new(width, height, stride);
            assert_eq!(
                frame.bgra.len(),
                stride * height as usize,
                "Frame buffer size must equal stride * height for {}x{} stride {}",
                width,
                height,
                stride
            );
            assert!(frame.validate().is_ok());
        }
    }
}
