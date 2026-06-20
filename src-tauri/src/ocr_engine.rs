/// OCR Engine implementation - wrapper around ocr-rs (rust-paddle-ocr)
///
/// This module provides the concrete implementation of the OcrEngine trait using
/// the ocr-rs crate which wraps PaddleOCR models with MNN backend.
///
/// **Requirements 4.1, 4.2, 4.3**: Models loaded once, in-memory processing, structured output

use crate::ocr::{ImageBuffer, OcrBox, OcrEngine, OcrError};
use std::path::Path;
use tracing::info;

// ============================================================================
// Real Implementation using ocr-rs
// ============================================================================

/// OCR engine using ocr-rs (rust-paddle-ocr with MNN backend)
///
/// This implementation wraps the ocr-rs::DetModel and ocr-rs::RecModel to provide
/// text detection and recognition capabilities using PaddleOCR v5 models.
pub struct RustPaddleOcrEngine {
    /// Detection model
    det_model: Option<ocr_rs::DetModel>,
    /// Recognition model  
    rec_model: Option<ocr_rs::RecModel>,
    /// Whether the engine is initialized
    initialized: bool,
}

impl RustPaddleOcrEngine {
    /// Creates a new OCR engine
    pub fn new() -> Self {
        Self {
            det_model: None,
            rec_model: None,
            initialized: false,
        }
    }
}

impl Default for RustPaddleOcrEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OcrEngine for RustPaddleOcrEngine {
    fn init(&mut self, det_model: &Path, rec_model: &Path, keys: &Path) -> Result<(), OcrError> {
        info!("Initializing rust-paddle-ocr engine");
        info!("  Detection model: {}", det_model.display());
        info!("  Recognition model: {}", rec_model.display());
        info!("  Keys file: {}", keys.display());

        // Validate that model files exist
        if !det_model.exists() {
            return Err(OcrError::ModelNotFound(format!(
                "Detection model not found: {}",
                det_model.display()
            )));
        }
        if !rec_model.exists() {
            return Err(OcrError::ModelNotFound(format!(
                "Recognition model not found: {}",
                rec_model.display()
            )));
        }
        if !keys.exists() {
            return Err(OcrError::ModelNotFound(format!(
                "Keys file not found: {}",
                keys.display()
            )));
        }

        // Load detection model.
        //
        // Speed tuning: the detection model internally scales the input to
        // `max_side_len`. Lowering it from the default 960 to 736 makes
        // detection ~1.7x faster. This does NOT reduce recognition quality
        // because `detect_and_crop` crops text boxes from the ORIGINAL
        // full-resolution image - only box localization runs at the lower size,
        // which is plenty for large subtitle text.
        //
        // `InferenceConfig` controls the MNN thread count (default 4).
        let det_config = ocr_rs::InferenceConfig::new()
            .with_threads(crate::constants::OCR_THREAD_COUNT)
            .with_precision(ocr_rs::PrecisionMode::Low);
        let det = ocr_rs::DetModel::from_file(det_model.to_str().unwrap(), Some(det_config))
            .map_err(|e| OcrError::InitializationFailed(format!("Failed to load detection model: {}", e)))?
            .with_options(
                ocr_rs::DetOptions::fast().with_max_side_len(crate::constants::OCR_DET_MAX_SIDE_LEN)
            );

        // Load recognition model with keys
        let rec_config = ocr_rs::InferenceConfig::new()
            .with_threads(crate::constants::OCR_THREAD_COUNT)
            .with_precision(ocr_rs::PrecisionMode::Low);
        let rec = ocr_rs::RecModel::from_file(
            rec_model.to_str().unwrap(),
            keys.to_str().unwrap(),
            Some(rec_config)
        ).map_err(|e| OcrError::InitializationFailed(format!("Failed to load recognition model: {}", e)))?;

        self.det_model = Some(det);
        self.rec_model = Some(rec);
        self.initialized = true;

        info!("rust-paddle-ocr engine initialized successfully");

        Ok(())
    }

    fn run(&mut self, img: &ImageBuffer) -> Result<Vec<OcrBox>, OcrError> {
        if !self.initialized {
            return Err(OcrError::InitializationFailed(
                "OCR engine not initialized. Call init() first.".to_string(),
            ));
        }

        // Validate image buffer
        let expected_len = ImageBuffer::expected_len(img.width, img.height);
        if img.data.len() != expected_len {
            return Err(OcrError::InvalidImageBuffer(format!(
                "Image data length {} does not match expected {} ({}x{} BGR)",
                img.data.len(),
                expected_len,
                img.width,
                img.height
            )));
        }

        // Convert BGR buffer to DynamicImage (RGB format)
        // ocr-rs expects image::DynamicImage
        let mut rgb_data = Vec::with_capacity(img.data.len());
        for chunk in img.data.chunks(3) {
            // Convert BGR to RGB
            rgb_data.push(chunk[2]); // R
            rgb_data.push(chunk[1]); // G
            rgb_data.push(chunk[0]); // B
        }

        let image_rgb = image::RgbImage::from_raw(img.width as u32, img.height as u32, rgb_data)
            .ok_or_else(|| OcrError::InvalidImageBuffer("Failed to create RGB image".to_string()))?;
        
        let dynamic_image = image::DynamicImage::ImageRgb8(image_rgb);

        // Get models
        let det_model = self.det_model.as_ref().unwrap();
        let rec_model = self.rec_model.as_ref().unwrap();

        // Detect text regions and crop
        let detections = det_model.detect_and_crop(&dynamic_image)
            .map_err(|e| OcrError::InferenceFailed(format!("Detection failed: {}", e)))?;

        if detections.is_empty() {
            return Ok(Vec::new());
        }

        // Extract cropped images for batch recognition
        let cropped_images: Vec<_> = detections.iter()
            .map(|(img, _)| img.clone())
            .collect();

        // Batch recognize
        let rec_results = rec_model.recognize_batch(&cropped_images)
            .map_err(|e| OcrError::InferenceFailed(format!("Recognition failed: {}", e)))?;

        // Convert results to our OcrBox format
        let mut results = Vec::new();
        for (rec_result, (_, det_box)) in rec_results.iter().zip(detections.iter()) {
            // Convert detection box to our format
            // det_box.points is Option<[Point<f32>; 4]>
            let bbox = if let Some(points) = &det_box.points {
                [
                    [points[0].x as i32, points[0].y as i32],
                    [points[1].x as i32, points[1].y as i32],
                    [points[2].x as i32, points[2].y as i32],
                    [points[3].x as i32, points[3].y as i32],
                ]
            } else {
                // Fallback to rect if points not available
                let rect = &det_box.rect;
                let left = rect.left() as i32;
                let top = rect.top() as i32;
                let right = rect.right() as i32;
                let bottom = rect.bottom() as i32;
                [
                    [left, top],
                    [right, top],
                    [right, bottom],
                    [left, bottom],
                ]
            };

            results.push(OcrBox {
                bbox,
                text: rec_result.text.clone(),
                confidence: rec_result.confidence,
            });
        }

        Ok(results)
    }

    fn detect(&mut self, img: &ImageBuffer) -> Result<Vec<OcrBox>, OcrError> {
        if !self.initialized {
            return Err(OcrError::InitializationFailed(
                "OCR engine not initialized. Call init() first.".to_string(),
            ));
        }

        let expected_len = ImageBuffer::expected_len(img.width, img.height);
        if img.data.len() != expected_len {
            return Err(OcrError::InvalidImageBuffer(format!(
                "Image data length {} does not match expected {} ({}x{} BGR)",
                img.data.len(),
                expected_len,
                img.width,
                img.height
            )));
        }

        // Convert BGR -> RGB DynamicImage.
        let mut rgb_data = Vec::with_capacity(img.data.len());
        for chunk in img.data.chunks(3) {
            rgb_data.push(chunk[2]); // R
            rgb_data.push(chunk[1]); // G
            rgb_data.push(chunk[0]); // B
        }
        let image_rgb = image::RgbImage::from_raw(img.width as u32, img.height as u32, rgb_data)
            .ok_or_else(|| OcrError::InvalidImageBuffer("Failed to create RGB image".to_string()))?;
        let dynamic_image = image::DynamicImage::ImageRgb8(image_rgb);

        let det_model = self.det_model.as_ref().unwrap();

        // Detection only (skip recognition) - just need box positions.
        let detections = det_model
            .detect_and_crop(&dynamic_image)
            .map_err(|e| OcrError::InferenceFailed(format!("Detection failed: {}", e)))?;

        let mut results = Vec::with_capacity(detections.len());
        for (_, det_box) in detections.iter() {
            let bbox = if let Some(points) = &det_box.points {
                [
                    [points[0].x as i32, points[0].y as i32],
                    [points[1].x as i32, points[1].y as i32],
                    [points[2].x as i32, points[2].y as i32],
                    [points[3].x as i32, points[3].y as i32],
                ]
            } else {
                let rect = &det_box.rect;
                let left = rect.left() as i32;
                let top = rect.top() as i32;
                let right = rect.right() as i32;
                let bottom = rect.bottom() as i32;
                [[left, top], [right, top], [right, bottom], [left, bottom]]
            };
            results.push(OcrBox {
                bbox,
                text: String::new(),
                confidence: 1.0,
            });
        }

        Ok(results)
    }
}

// ============================================================================
// Factory Function
// ============================================================================

/// Creates the default OCR engine implementation
///
/// Returns RustPaddleOcrEngine which uses ocr-rs (rust-paddle-ocr) with MNN backend.
pub fn create_ocr_engine() -> Box<dyn OcrEngine> {
    Box::new(RustPaddleOcrEngine::new())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper to create temporary model files for testing
    fn create_test_models() -> (TempDir, String, String, String) {
        let temp_dir = TempDir::new().unwrap();
        
        let det_path = temp_dir.path().join("det.mnn");
        let rec_path = temp_dir.path().join("rec.mnn");
        let keys_path = temp_dir.path().join("keys.txt");
        
        // Create dummy files
        File::create(&det_path).unwrap().write_all(b"dummy det model").unwrap();
        File::create(&rec_path).unwrap().write_all(b"dummy rec model").unwrap();
        File::create(&keys_path).unwrap().write_all(b"dummy keys").unwrap();
        
        (
            temp_dir,
            det_path.to_str().unwrap().to_string(),
            rec_path.to_str().unwrap().to_string(),
            keys_path.to_str().unwrap().to_string(),
        )
    }

    #[test]
    fn test_engine_creation() {
        let engine = RustPaddleOcrEngine::new();
        assert!(!engine.initialized);
    }

    #[test]
    fn test_init_missing_det_model() {
        let (_temp_dir, _det_path, rec_path, keys_path) = create_test_models();
        let mut engine = RustPaddleOcrEngine::new();
        
        let result = engine.init(
            Path::new("nonexistent_det.mnn"),
            Path::new(&rec_path),
            Path::new(&keys_path),
        );
        
        assert!(result.is_err());
        match result {
            Err(OcrError::ModelNotFound(msg)) => {
                assert!(msg.contains("Detection model not found"));
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_init_missing_rec_model() {
        let (_temp_dir, det_path, _rec_path, keys_path) = create_test_models();
        let mut engine = RustPaddleOcrEngine::new();
        
        let result = engine.init(
            Path::new(&det_path),
            Path::new("nonexistent_rec.mnn"),
            Path::new(&keys_path),
        );
        
        assert!(result.is_err());
        match result {
            Err(OcrError::ModelNotFound(msg)) => {
                assert!(msg.contains("Recognition model not found"));
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_init_missing_keys() {
        let (_temp_dir, det_path, rec_path, _keys_path) = create_test_models();
        let mut engine = RustPaddleOcrEngine::new();
        
        let result = engine.init(
            Path::new(&det_path),
            Path::new(&rec_path),
            Path::new("nonexistent_keys.txt"),
        );
        
        assert!(result.is_err());
        match result {
            Err(OcrError::ModelNotFound(msg)) => {
                assert!(msg.contains("Keys file not found"));
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_run_not_initialized() {
        let mut engine = RustPaddleOcrEngine::new();
        let img = ImageBuffer::new(10, 10, vec![0u8; 300]);
        
        let result = engine.run(&img);
        
        assert!(result.is_err());
        match result {
            Err(OcrError::InitializationFailed(msg)) => {
                assert!(msg.contains("not initialized"));
            }
            _ => panic!("Expected InitializationFailed error"),
        }
    }

    #[test]
    fn test_create_ocr_engine() {
        let mut engine = create_ocr_engine();
        // Verify it returns a valid engine by trying to use it
        // (Initialization should fail without real models, but that's expected)
        let result = engine.run(&ImageBuffer::new(10, 10, vec![0u8; 300]));
        assert!(result.is_err()); // Should fail because not initialized
    }

    // Integration test with real models (only runs if models are available)
    #[test]
    #[ignore] // Ignore by default, run with: cargo test -- --ignored
    fn test_real_ocr_integration() {
        // This test requires real model files
        let det_path = "../../PaddleOCR-MNN-main/PP-OCRv5_mobile_det.mnn";
        let rec_path = "../../PaddleOCR-MNN-main/PP-OCRv5_mobile_rec.mnn";
        let keys_path = "../../PaddleOCR-MNN-main/ppocr_keys_v5.txt";

        if !Path::new(det_path).exists() {
            println!("Skipping integration test - models not found");
            return;
        }

        let mut engine = RustPaddleOcrEngine::new();
        
        // Initialize
        let result = engine.init(
            Path::new(det_path),
            Path::new(rec_path),
            Path::new(keys_path),
        );
        
        assert!(result.is_ok(), "Failed to initialize: {:?}", result.err());
        assert!(engine.initialized);

        // Create a test image (simple white background for now)
        let width: u32 = 640;
        let height: u32 = 480;
        let img_data = vec![255u8; (width * height * 3) as usize];
        let img = ImageBuffer::new(width, height, img_data);

        // Run OCR
        let result = engine.run(&img);
        assert!(result.is_ok(), "OCR failed: {:?}", result.err());
        
        // Empty image should return empty or minimal results
        let boxes = result.unwrap();
        println!("Detected {} text boxes", boxes.len());
    }
}
