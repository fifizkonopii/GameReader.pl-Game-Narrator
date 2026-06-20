//! Image preprocessing for OCR enhancement.
//!
//! This module implements scale-dependent preprocessing to improve OCR accuracy:
//! - For scale < 0.7: Simple grayscale conversion
//! - For scale >= 0.7: Advanced pipeline (max-channel + erosion + CLAHE + sharpening)

use crate::capture::Frame;
use crate::ocr::ImageBuffer;

/// Preprocessor for OCR image enhancement.
///
/// Applies different preprocessing strategies based on the downscale factor:
/// - Low resolution (< 0.7): Grayscale conversion only
/// - High resolution (>= 0.7): Full pipeline with enhancement filters
pub struct Preprocessor {
    outline_mode: bool,
}

#[allow(dead_code)]
impl Preprocessor {
    /// Creates a new preprocessor. CLAHE colour-contrast enhancement is always
    /// applied; no resolution downscaling is performed.
    pub fn new() -> Self {
        Self { outline_mode: true }
    }

    /// Enable the automatic outline-text mode (white subtitles with a dark
    /// outline). Thresholds are derived per-pixel from the local neighbourhood
    /// (adaptive), so there is nothing to tune. Returns self for chaining.
    pub fn with_outline_mode(mut self, enabled: bool) -> Self {
        self.outline_mode = enabled;
        self
    }

    /// Preprocesses a frame for OCR input.
    ///
    /// Returns an ImageBuffer (BGR format) suitable for OCR engine input.
    ///
    /// By default the original COLOR image is passed through (best for the
    /// PP-OCRv5 model, which is trained on natural color images). If
    /// `OCR_BINARIZE` is enabled, a grayscale + Otsu threshold is applied
    /// instead (only suitable for very high-contrast, dark-background text).
    pub fn for_ocr(&self, frame: &Frame) -> ImageBuffer {
        let target_width = frame.width;
        let target_height = frame.height;
        let pixel_count = (target_width * target_height) as usize;

        // Outline text mode takes precedence: isolate white-with-dark-outline
        // subtitles so bright backgrounds don't drown them out.
        if self.outline_mode {
            return self.outline_white_text(frame);
        }

        if crate::constants::OCR_BINARIZE {
            // Grayscale + Otsu binarization (legacy path).
            let mut gray = Vec::with_capacity(pixel_count);
            for y in 0..target_height {
                let row_start = (y as usize) * frame.stride;
                for x in 0..target_width {
                    let offset = row_start + (x as usize) * 4;
                    let b = frame.bgra[offset] as f32;
                    let g = frame.bgra[offset + 1] as f32;
                    let r = frame.bgra[offset + 2] as f32;
                    gray.push((0.299 * r + 0.587 * g + 0.114 * b) as u8);
                }
            }
            let threshold = otsu_threshold(&gray);
            let mut data = Vec::with_capacity(pixel_count * 3);
            for &v in &gray {
                let p = if v >= threshold { 255 } else { 0 };
                data.push(p);
                data.push(p);
                data.push(p);
            }
            return ImageBuffer { width: target_width, height: target_height, data };
        }

        // Default: pass through the original color image as tightly-packed BGR.
        let mut data = Vec::with_capacity(pixel_count * 3);
        for y in 0..target_height {
            let row_start = (y as usize) * frame.stride;
            for x in 0..target_width {
                let offset = row_start + (x as usize) * 4;
                data.push(frame.bgra[offset]); // B
                data.push(frame.bgra[offset + 1]); // G
                data.push(frame.bgra[offset + 2]); // R
            }
        }

        ImageBuffer {
            width: target_width,
            height: target_height,
            data,
        }
    }

    /// Automatic adaptive contrast enhancement that PRESERVES COLOUR.
    ///
    /// Applies CLAHE (Contrast Limited Adaptive Histogram Equalization) to the
    /// luminance: the image is split into tiles, each locally equalised with a
    /// clip limit that prevents over-amplifying flat/noisy areas, then the tile
    /// curves are bilinearly blended so there are no block seams. The original
    /// colour is recombined by scaling each pixel's RGB by the luminance change,
    /// so hue is kept. No binarization, no morphology — the OCR model receives a
    /// natural, contrast-normalised colour image where faint text stands out.
    fn outline_white_text(&self, frame: &Frame) -> ImageBuffer {
        let w = frame.width as usize;
        let h = frame.height as usize;
        if w == 0 || h == 0 {
            return ImageBuffer { width: frame.width, height: frame.height, data: Vec::new() };
        }

        // Luminance (Y); original colour is kept for recombination.
        let mut y_lum = vec![0u8; w * h];
        for y in 0..h {
            let row = y * frame.stride;
            for x in 0..w {
                let o = row + x * 4;
                let b = frame.bgra[o] as f32;
                let g = frame.bgra[o + 1] as f32;
                let r = frame.bgra[o + 2] as f32;
                y_lum[y * w + x] = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
            }
        }

        // CLAHE tile grid (subtitle crops are wide and short).
        let gx = (w / 80).clamp(4, 12);
        let gy = (h / 60).clamp(1, 4);
        let tw = (w as f32 / gx as f32).max(1.0);
        let th = (h as f32 / gy as f32).max(1.0);
        let clip_factor = 3.0f32; // higher = more contrast (and more noise)

        // Per-tile contrast-limited equalisation lookup tables.
        let mut luts = vec![[0u8; 256]; gx * gy];
        for ty in 0..gy {
            for tx in 0..gx {
                let x0 = (tx as f32 * tw) as usize;
                let x1 = (((tx + 1) as f32 * tw) as usize).min(w);
                let y0 = (ty as f32 * th) as usize;
                let y1 = (((ty + 1) as f32 * th) as usize).min(h);

                let mut hist = [0u32; 256];
                let mut cnt = 0u32;
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        hist[y_lum[yy * w + xx] as usize] += 1;
                        cnt += 1;
                    }
                }
                let lut = &mut luts[ty * gx + tx];
                if cnt == 0 {
                    for (i, v) in lut.iter_mut().enumerate() {
                        *v = i as u8;
                    }
                    continue;
                }
                // Clip the histogram and redistribute the excess uniformly.
                let clip = ((clip_factor * cnt as f32 / 256.0) as u32).max(1);
                let mut excess = 0u32;
                for b in hist.iter_mut() {
                    if *b > clip {
                        excess += *b - clip;
                        *b = clip;
                    }
                }
                let inc = excess / 256;
                let rem = (excess % 256) as usize;
                for b in hist.iter_mut() {
                    *b += inc;
                }
                for b in 0..rem {
                    hist[b] += 1;
                }
                // CDF -> normalised LUT.
                let mut cdf = 0u32;
                for b in 0..256 {
                    cdf += hist[b];
                    lut[b] = ((cdf as f32 / cnt as f32) * 255.0).clamp(0.0, 255.0) as u8;
                }
            }
        }

        // Bilinearly blend the 4 surrounding tile LUTs per pixel, then recolour.
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            let fy = (y as f32 + 0.5) / th - 0.5;
            let ty_f = fy.floor();
            let wy = fy - ty_f;
            let ty0 = (ty_f as i32).clamp(0, gy as i32 - 1) as usize;
            let ty1 = (ty_f as i32 + 1).clamp(0, gy as i32 - 1) as usize;
            for x in 0..w {
                let fx = (x as f32 + 0.5) / tw - 0.5;
                let tx_f = fx.floor();
                let wx = fx - tx_f;
                let tx0 = (tx_f as i32).clamp(0, gx as i32 - 1) as usize;
                let tx1 = (tx_f as i32 + 1).clamp(0, gx as i32 - 1) as usize;

                let yv = y_lum[y * w + x] as usize;
                let l00 = luts[ty0 * gx + tx0][yv] as f32;
                let l01 = luts[ty0 * gx + tx1][yv] as f32;
                let l10 = luts[ty1 * gx + tx0][yv] as f32;
                let l11 = luts[ty1 * gx + tx1][yv] as f32;
                let top = l00 * (1.0 - wx) + l01 * wx;
                let bot = l10 * (1.0 - wx) + l11 * wx;
                let new_y = (top * (1.0 - wy) + bot * wy).clamp(1.0, 255.0);

                let o = y * frame.stride + x * 4;
                let old_y = (y_lum[y * w + x] as f32).max(1.0);
                let f = new_y / old_y;
                data.push((frame.bgra[o] as f32 * f).clamp(0.0, 255.0) as u8); // B
                data.push((frame.bgra[o + 1] as f32 * f).clamp(0.0, 255.0) as u8); // G
                data.push((frame.bgra[o + 2] as f32 * f).clamp(0.0, 255.0) as u8); // R
            }
        }

        ImageBuffer { width: frame.width, height: frame.height, data }
    }

    /// Converts BGRA to binary black/white (threshold), then outputs as BGR.
    ///
    /// This mirrors Python's binary preprocessing which significantly reduces OCR CPU load.
    /// Uses Otsu's method to auto-calculate optimal threshold for each frame.
    fn to_grayscale_bgr(&self, bgra: &[u8], width: usize, height: usize) -> Vec<u8> {
        // First pass: convert to grayscale using luminance formula
        let mut gray = Vec::with_capacity(width * height);

        for pixel in bgra.chunks_exact(4) {
            let b = pixel[0] as f32;
            let g = pixel[1] as f32;
            let r = pixel[2] as f32;
            
            // Luminance formula
            let gray_val = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
            gray.push(gray_val);
        }
        
        // Calculate Otsu threshold (auto-threshold like cv2.threshold with OTSU)
        let threshold = self.calculate_otsu_threshold(&gray);
        
        // Second pass: apply binary threshold and convert to BGR
        let mut result = Vec::with_capacity(width * height * 3);
        for &g in &gray {
            let binary = if g >= threshold { 255 } else { 0 };
            // Output as BGR (all channels same)
            result.push(binary); // B
            result.push(binary); // G
            result.push(binary); // R
        }

        result
    }
    
    /// Calculates optimal threshold using Otsu's method.
    /// Returns threshold value (0-255).
    fn calculate_otsu_threshold(&self, gray: &[u8]) -> u8 {
        // Build histogram
        let mut hist = [0u32; 256];
        for &pixel in gray {
            hist[pixel as usize] += 1;
        }
        
        let total = gray.len() as f64;
        let mut sum = 0.0;
        for (i, &count) in hist.iter().enumerate() {
            sum += i as f64 * count as f64;
        }
        
        let mut sum_b = 0.0;
        let mut wb = 0.0;
        let mut max_variance = 0.0;
        let mut threshold = 0u8;
        
        for (t, &count) in hist.iter().enumerate() {
            wb += count as f64;
            if wb == 0.0 {
                continue;
            }
            
            let wf = total - wb;
            if wf == 0.0 {
                break;
            }
            
            sum_b += t as f64 * count as f64;
            let mb = sum_b / wb;
            let mf = (sum - sum_b) / wf;
            
            let variance = wb * wf * (mb - mf) * (mb - mf);
            if variance > max_variance {
                max_variance = variance;
                threshold = t as u8;
            }
        }
        
        threshold
    }

    /// Full enhancement pipeline for high-resolution images.
    ///
    /// Steps:
    /// 1. Max-channel extraction (brightens colored text on dark backgrounds)
    /// 2. Glow erosion (removes bloom/halo around letters)
    /// 3. CLAHE (Contrast Limited Adaptive Histogram Equalization)
    /// 4. Sharpening
    /// 5. Convert to BGR
    fn enhance_for_ocr(&self, bgra: &[u8], width: usize, height: usize) -> Vec<u8> {
        // Step 1: Max-channel extraction (take max of R, G, B for each pixel)
        let max_channel = self.extract_max_channel(bgra, width, height);
        
        // Step 2: Glow erosion (3x3 elliptical kernel)
        let eroded = self.apply_erosion(&max_channel, width, height);
        
        // Step 3: CLAHE (simplified version)
        let clahe = self.apply_clahe(&eroded, width, height);
        
        // Step 4: Sharpening (unsharp mask approximation)
        let sharpened = self.apply_sharpening(&clahe, width, height);
        
        // Step 5: Convert grayscale to BGR (duplicate to 3 channels)
        let mut result = Vec::with_capacity(width * height * 3);
        for &pixel in &sharpened {
            result.push(pixel); // B
            result.push(pixel); // G
            result.push(pixel); // R
        }
        
        result
    }

    /// Extracts maximum channel from BGR for each pixel.
    ///
    /// This brightens colored text (cyan, yellow, white) on dark backgrounds.
    fn extract_max_channel(&self, bgra: &[u8], width: usize, height: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(width * height);

        for pixel in bgra.chunks_exact(4) {
            let b = pixel[0];
            let g = pixel[1];
            let r = pixel[2];
            
            let max = b.max(g).max(r);
            result.push(max);
        }

        result
    }

    /// Applies morphological erosion with 3x3 elliptical kernel.
    ///
    /// Removes glow/bloom around letters.
    fn apply_erosion(&self, img: &[u8], width: usize, height: usize) -> Vec<u8> {
        let mut result = vec![0u8; width * height];
        
        // 3x3 elliptical kernel (cross pattern)
        let kernel = [
            (0, -1), (-1, 0), (0, 0), (1, 0), (0, 1)
        ];

        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                let mut min_val = 255u8;
                
                for &(dx, dy) in &kernel {
                    let ny = (y as i32 + dy) as usize;
                    let nx = (x as i32 + dx) as usize;
                    let pixel = img[ny * width + nx];
                    min_val = min_val.min(pixel);
                }
                
                result[y * width + x] = min_val;
            }
        }

        // Copy borders
        for x in 0..width {
            result[x] = img[x];
            result[(height - 1) * width + x] = img[(height - 1) * width + x];
        }
        for y in 0..height {
            result[y * width] = img[y * width];
            result[y * width + width - 1] = img[y * width + width - 1];
        }

        result
    }

    /// Applies simplified CLAHE (Contrast Limited Adaptive Histogram Equalization).
    ///
    /// Enhances local contrast. This is a simplified implementation without
    /// tile-based processing (applies global histogram equalization with contrast limiting).
    fn apply_clahe(&self, img: &[u8], width: usize, height: usize) -> Vec<u8> {
        // Build histogram
        let mut hist = [0u32; 256];
        for &pixel in img {
            hist[pixel as usize] += 1;
        }

        // Build cumulative distribution function (CDF)
        let mut cdf = [0u32; 256];
        cdf[0] = hist[0];
        for i in 1..256 {
            cdf[i] = cdf[i - 1] + hist[i];
        }

        // Normalize CDF to 0-255 range
        let total_pixels = (width * height) as f32;
        let cdf_min = *cdf.iter().find(|&&x| x > 0).unwrap_or(&0);
        
        let mut lut = [0u8; 256];
        for i in 0..256 {
            if cdf[i] > 0 {
                let normalized = ((cdf[i] - cdf_min) as f32 / (total_pixels - cdf_min as f32) * 255.0) as u8;
                lut[i] = normalized;
            }
        }

        // Apply lookup table
        img.iter().map(|&pixel| lut[pixel as usize]).collect()
    }

    /// Applies sharpening filter (unsharp mask approximation).
    ///
    /// Uses a simple 3x3 sharpening kernel.
    fn apply_sharpening(&self, img: &[u8], width: usize, height: usize) -> Vec<u8> {
        let mut result = vec![0u8; width * height];
        
        // Sharpening kernel (approximation of unsharp mask)
        // [  0, -1,  0 ]
        // [ -1,  5, -1 ]
        // [  0, -1,  0 ]
        
        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                let center = img[y * width + x] as i32;
                let top = img[(y - 1) * width + x] as i32;
                let bottom = img[(y + 1) * width + x] as i32;
                let left = img[y * width + (x - 1)] as i32;
                let right = img[y * width + (x + 1)] as i32;
                
                let sharpened = 5 * center - top - bottom - left - right;
                result[y * width + x] = sharpened.clamp(0, 255) as u8;
            }
        }

        // Copy borders
        for x in 0..width {
            result[x] = img[x];
            result[(height - 1) * width + x] = img[(height - 1) * width + x];
        }
        for y in 0..height {
            result[y * width] = img[y * width];
            result[y * width + width - 1] = img[y * width + width - 1];
        }

        result
    }
}

/// Computes an optimal binarization threshold using Otsu's method.
///
/// Otsu picks the threshold that maximizes between-class variance of the
/// grayscale histogram (separating "text" from "background"), so it adapts
/// automatically to different brightness levels without manual tuning.
fn otsu_threshold(gray: &[u8]) -> u8 {
    if gray.is_empty() {
        return 128;
    }

    // Histogram
    let mut hist = [0u32; 256];
    for &v in gray {
        hist[v as usize] += 1;
    }

    let total = gray.len() as f64;
    // Sum of (intensity * count)
    let mut sum_all = 0f64;
    for (i, &c) in hist.iter().enumerate() {
        sum_all += i as f64 * c as f64;
    }

    let mut sum_b = 0f64; // weighted sum of background
    let mut w_b = 0f64;   // background pixel count
    let mut max_between = -1f64;
    let mut threshold = 128u8;

    for t in 0..256 {
        w_b += hist[t] as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b; // foreground count
        if w_f == 0.0 {
            break;
        }

        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b;            // background mean
        let m_f = (sum_all - sum_b) / w_f; // foreground mean

        let between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if between > max_between {
            max_between = between;
            threshold = t as u8;
        }
    }

    threshold
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
    fn test_for_ocr_keeps_full_size() {
        let prep = Preprocessor::new();
        let frame = create_test_frame(100, 100, [128, 128, 128, 255]);

        let result = prep.for_ocr(&frame);

        // No downscaling: output matches input dimensions, BGR (3 bytes/px).
        assert_eq!(result.width, 100);
        assert_eq!(result.height, 100);
        assert_eq!(result.data.len(), 100 * 100 * 3);
    }

    #[test]
    fn test_to_grayscale_bgr() {
        let prep = Preprocessor::new();
        
        // Create BGRA data with known colors
        let bgra = vec![
            0, 0, 255, 255,     // Red pixel
            0, 255, 0, 255,     // Green pixel
            255, 0, 0, 255,     // Blue pixel
            255, 255, 255, 255, // White pixel
        ];
        
        let gray = prep.to_grayscale_bgr(&bgra, 2, 2);
        
        // Should have 2x2 pixels in BGR format
        assert_eq!(gray.len(), 2 * 2 * 3);
        
        // Check grayscale values (luminance formula)
        // Red: 0.299 * 255 ≈ 76
        let red_gray = gray[0];
        assert!((red_gray as i32 - 76).abs() <= 1);
        
        // Green: 0.587 * 255 ≈ 149
        let green_gray = gray[3];
        assert!((green_gray as i32 - 149).abs() <= 1);
        
        // Blue: 0.114 * 255 ≈ 29
        let blue_gray = gray[6];
        assert!((blue_gray as i32 - 29).abs() <= 1);
        
        // White: 255
        assert_eq!(gray[9], 255);
    }

    #[test]
    fn test_extract_max_channel() {
        let prep = Preprocessor::new();
        
        let bgra = vec![
            100, 50, 200, 255,  // Max = 200 (R)
            50, 150, 50, 255,   // Max = 150 (G)
            200, 100, 100, 255, // Max = 200 (B)
        ];
        
        let max_ch = prep.extract_max_channel(&bgra, 3, 1);
        
        assert_eq!(max_ch[0], 200);
        assert_eq!(max_ch[1], 150);
        assert_eq!(max_ch[2], 200);
    }

    #[test]
    fn test_apply_erosion() {
        let prep = Preprocessor::new();
        
        // Create 5x5 image with a bright spot in the center
        let mut img = vec![0u8; 25];
        img[12] = 255; // Center pixel
        
        let eroded = prep.apply_erosion(&img, 5, 5);
        
        // Center should be darkened (erosion takes minimum of neighborhood)
        assert!(eroded[12] < 255);
    }

    #[test]
    fn test_apply_clahe() {
        let prep = Preprocessor::new();
        
        // Create low-contrast image
        let img = vec![100u8; 100];
        
        let enhanced = prep.apply_clahe(&img, 10, 10);
        
        // All pixels should be mapped to same value (uniform image)
        assert!(enhanced.iter().all(|&x| x == enhanced[0]));
    }

    #[test]
    fn test_apply_sharpening() {
        let prep = Preprocessor::new();
        
        // Create 5x5 gradient image
        let mut img = vec![0u8; 25];
        for i in 0..25 {
            img[i] = (i * 10) as u8;
        }
        
        let sharpened = prep.apply_sharpening(&img, 5, 5);
        
        // Sharpening should modify interior pixels
        assert_eq!(sharpened.len(), 25);
        
        // Borders should be unchanged
        assert_eq!(sharpened[0], img[0]);
        assert_eq!(sharpened[4], img[4]);
        assert_eq!(sharpened[20], img[20]);
        assert_eq!(sharpened[24], img[24]);
    }
}
