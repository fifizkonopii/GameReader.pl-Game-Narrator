//! Frame difference detection for skipping OCR on static content.
//!
//! This module implements efficient frame comparison to detect when screen content
//! hasn't changed, allowing OCR to be skipped and reducing CPU usage.

use crate::capture::{CaptureRegion, Frame};
use crate::constants::FRAME_DIFFERENCE_THRESHOLD;

/// Frame differencing state tracker.
///
/// Maintains a cache of the previous frame for comparison and tracks
/// when the capture region changes (requiring cache reset).
pub struct FrameDiffer {
    /// Cached previous frame (downscaled grayscale)
    cached_frame: Option<Vec<u8>>,
    /// Width of cached frame
    cached_width: u32,
    /// Height of cached frame  
    cached_height: u32,
    /// Last region used (for detecting region changes)
    last_region: Option<CaptureRegion>,
    /// Downscale factor for frame comparison (from config)
    downscale: f32,
}

impl FrameDiffer {
    /// Creates a new frame differ with the specified downscale factor.
    ///
    /// # Arguments
    /// * `downscale` - Resolution downscale factor (0.1-1.0)
    pub fn new(downscale: f32) -> Self {
        Self {
            cached_frame: None,
            cached_width: 0,
            cached_height: 0,
            last_region: None,
            downscale: downscale.clamp(0.1, 1.0),
        }
    }

    /// Computes the difference score between current frame and cached frame.
    ///
    /// Returns the mean absolute difference per pixel (0-255 scale).
    /// Returns `f32::MAX` if there's no cached frame (first frame always differs).
    ///
    /// # Arguments
    /// * `frame` - The current frame (BGRA format)
    /// * `region` - The capture region (used to detect region changes)
    ///
    /// # Returns
    /// Difference score where:
    /// - 0.0 = identical frames
    /// - Higher values = more different
    /// - `f32::MAX` = no cached frame or region changed
    pub fn compute_difference(&mut self, frame: &Frame, region: &CaptureRegion) -> f32 {
        // Check if region changed - invalidate cache
        if let Some(last_region) = self.last_region {
            if last_region != *region {
                tracing::debug!("Capture region changed - resetting frame cache");
                self.reset();
            }
        }

        // Convert current frame to downscaled grayscale
        let gray = self.to_grayscale_downscaled(frame);
        let width = (frame.width as f32 * self.downscale) as u32;
        let height = (frame.height as f32 * self.downscale) as u32;

        // First frame or no cache - always different
        if self.cached_frame.is_none() {
            self.cached_frame = Some(gray);
            self.cached_width = width;
            self.cached_height = height;
            self.last_region = Some(*region);
            return f32::MAX;
        }

        // Compute mean absolute difference
        let cached = self.cached_frame.as_ref().unwrap();
        let diff_sum: u32 = gray.iter()
            .zip(cached.iter())
            .map(|(&a, &b)| (a as i32 - b as i32).unsigned_abs())
            .sum();

        let pixel_count = gray.len() as f32;
        let mean_diff = diff_sum as f32 / pixel_count;

        // Update cache with current frame
        self.cached_frame = Some(gray);
        self.cached_width = width;
        self.cached_height = height;
        self.last_region = Some(*region);

        mean_diff
    }

    /// Checks if OCR should be skipped based on frame difference.
    ///
    /// Returns `true` if the difference is below the threshold (frames are similar).
    pub fn should_skip_ocr(&mut self, frame: &Frame, region: &CaptureRegion) -> bool {
        let diff = self.compute_difference(frame, region);
        diff < FRAME_DIFFERENCE_THRESHOLD
    }

    /// Resets the cached frame (e.g., when capture region or monitor changes).
    pub fn reset(&mut self) {
        self.cached_frame = None;
        self.cached_width = 0;
        self.cached_height = 0;
        self.last_region = None;
    }

    /// Converts a BGRA frame to downscaled grayscale.
    ///
    /// Uses luminance formula: Y = 0.299*R + 0.587*G + 0.114*B
    fn to_grayscale_downscaled(&self, frame: &Frame) -> Vec<u8> {
        let new_width = (frame.width as f32 * self.downscale) as usize;
        let new_height = (frame.height as f32 * self.downscale) as usize;
        
        let mut result = Vec::with_capacity(new_width * new_height);

        for y in 0..new_height {
            for x in 0..new_width {
                // Map to source coordinates
                let src_x = ((x as f32 / self.downscale) as usize).min(frame.width as usize - 1);
                let src_y = ((y as f32 / self.downscale) as usize).min(frame.height as usize - 1);
                
                let offset = src_y * frame.stride + src_x * 4;
                
                // BGRA format
                let b = frame.bgra[offset] as f32;
                let g = frame.bgra[offset + 1] as f32;
                let r = frame.bgra[offset + 2] as f32;
                
                // Luminance formula
                let gray = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
                result.push(gray);
            }
        }

        result
    }

    /// Returns the downscale factor used for comparison.
    pub fn downscale(&self) -> f32 {
        self.downscale
    }

    /// Updates the downscale factor and resets the cache.
    pub fn set_downscale(&mut self, downscale: f32) {
        self.downscale = downscale.clamp(0.1, 1.0);
        self.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(width: u32, height: u32, color: [u8; 4]) -> Frame {
        let stride = width as usize * 4;
        let mut bgra = vec![0u8; stride * height as usize];
        
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
        
        Frame::from_data(width, height, stride, bgra).unwrap()
    }

    #[test]
    fn test_frame_differ_new() {
        let differ = FrameDiffer::new(0.5);
        assert_eq!(differ.downscale(), 0.5);
        assert!(differ.cached_frame.is_none());
    }

    #[test]
    fn test_frame_differ_downscale_clamping() {
        let differ = FrameDiffer::new(2.0); // Too high
        assert_eq!(differ.downscale(), 1.0);
        
        let differ = FrameDiffer::new(0.05); // Too low
        assert_eq!(differ.downscale(), 0.1);
    }

    // Property 11: Frame-diff skip on identical frames
    // Validates: Requirements 3.2, 3.3
    #[test]
    fn test_identical_frames_below_threshold() {
        let mut differ = FrameDiffer::new(0.5);
        let region = CaptureRegion::new(0, 0, 100, 100);
        
        // Create identical frames (white)
        let frame1 = create_test_frame(100, 100, [255, 255, 255, 255]);
        let frame2 = create_test_frame(100, 100, [255, 255, 255, 255]);
        
        // First frame - always different
        let diff1 = differ.compute_difference(&frame1, &region);
        assert_eq!(diff1, f32::MAX);
        
        // Second identical frame - should have 0 difference
        let diff2 = differ.compute_difference(&frame2, &region);
        assert_eq!(diff2, 0.0);
        assert!(diff2 < FRAME_DIFFERENCE_THRESHOLD);
    }

    #[test]
    fn test_different_frames_above_threshold() {
        let mut differ = FrameDiffer::new(0.5);
        let region = CaptureRegion::new(0, 0, 100, 100);
        
        // Create different frames (white vs black)
        let frame1 = create_test_frame(100, 100, [255, 255, 255, 255]);
        let frame2 = create_test_frame(100, 100, [0, 0, 0, 255]);
        
        differ.compute_difference(&frame1, &region);
        let diff = differ.compute_difference(&frame2, &region);
        
        // Should be significantly different
        assert!(diff > FRAME_DIFFERENCE_THRESHOLD);
        assert!(diff > 100.0); // Expect large difference for white vs black
    }

    #[test]
    fn test_should_skip_ocr_identical() {
        let mut differ = FrameDiffer::new(0.5);
        let region = CaptureRegion::new(0, 0, 100, 100);
        
        let frame = create_test_frame(100, 100, [128, 128, 128, 255]);
        
        // First frame - never skip
        assert!(!differ.should_skip_ocr(&frame, &region));
        
        // Second identical frame - should skip
        assert!(differ.should_skip_ocr(&frame, &region));
    }

    #[test]
    fn test_should_skip_ocr_different() {
        let mut differ = FrameDiffer::new(0.5);
        let region = CaptureRegion::new(0, 0, 100, 100);
        
        let frame1 = create_test_frame(100, 100, [255, 255, 255, 255]);
        let frame2 = create_test_frame(100, 100, [0, 0, 0, 255]);
        
        differ.should_skip_ocr(&frame1, &region);
        
        // Very different frame - should not skip
        assert!(!differ.should_skip_ocr(&frame2, &region));
    }

    // Property: Cache reset on region change
    // Validates: Requirements 3.4, 3.5
    #[test]
    fn test_cache_reset_on_region_change() {
        let mut differ = FrameDiffer::new(0.5);
        
        let region1 = CaptureRegion::new(0, 0, 100, 100);
        let region2 = CaptureRegion::new(50, 50, 100, 100);
        
        let frame = create_test_frame(100, 100, [128, 128, 128, 255]);
        
        // Process with first region
        let diff1 = differ.compute_difference(&frame, &region1);
        assert_eq!(diff1, f32::MAX); // First frame
        
        // Same frame, same region - should be 0
        let diff2 = differ.compute_difference(&frame, &region1);
        assert_eq!(diff2, 0.0);
        
        // Same frame, different region - cache reset, returns MAX
        let diff3 = differ.compute_difference(&frame, &region2);
        assert_eq!(diff3, f32::MAX);
    }

    #[test]
    fn test_reset() {
        let mut differ = FrameDiffer::new(0.5);
        let region = CaptureRegion::new(0, 0, 100, 100);
        let frame = create_test_frame(100, 100, [128, 128, 128, 255]);
        
        differ.compute_difference(&frame, &region);
        assert!(differ.cached_frame.is_some());
        
        differ.reset();
        assert!(differ.cached_frame.is_none());
        assert!(differ.last_region.is_none());
    }

    #[test]
    fn test_set_downscale() {
        let mut differ = FrameDiffer::new(0.5);
        assert_eq!(differ.downscale(), 0.5);
        
        differ.set_downscale(0.3);
        assert_eq!(differ.downscale(), 0.3);
        
        // Setting downscale resets cache
        let region = CaptureRegion::new(0, 0, 100, 100);
        let frame = create_test_frame(100, 100, [128, 128, 128, 255]);
        differ.compute_difference(&frame, &region);
        
        assert!(differ.cached_frame.is_some());
        differ.set_downscale(0.7);
        assert!(differ.cached_frame.is_none());
    }

    #[test]
    fn test_grayscale_conversion() {
        let differ = FrameDiffer::new(1.0); // No downscaling
        
        // Create frame with known colors
        let width = 2;
        let height = 2;
        let stride = 8;
        let mut bgra = vec![0u8; stride * 2];
        
        // Pixel 0,0: Red (0, 0, 255, 255)
        bgra[0] = 0;
        bgra[1] = 0;
        bgra[2] = 255;
        bgra[3] = 255;
        
        // Pixel 1,0: Green (0, 255, 0, 255)
        bgra[4] = 0;
        bgra[5] = 255;
        bgra[6] = 0;
        bgra[7] = 255;
        
        // Pixel 0,1: Blue (255, 0, 0, 255)
        bgra[8] = 255;
        bgra[9] = 0;
        bgra[10] = 0;
        bgra[11] = 255;
        
        // Pixel 1,1: White (255, 255, 255, 255)
        bgra[12] = 255;
        bgra[13] = 255;
        bgra[14] = 255;
        bgra[15] = 255;
        
        let frame = Frame::from_data(width, height, stride, bgra).unwrap();
        let gray = differ.to_grayscale_downscaled(&frame);
        
        // Check grayscale values (luminance formula)
        // Red: 0.299 * 255 = 76
        assert!((gray[0] as i32 - 76).abs() <= 1);
        
        // Green: 0.587 * 255 = 149
        assert!((gray[1] as i32 - 149).abs() <= 1);
        
        // Blue: 0.114 * 255 = 29
        assert!((gray[2] as i32 - 29).abs() <= 1);
        
        // White: 255
        assert_eq!(gray[3], 255);
    }
}
