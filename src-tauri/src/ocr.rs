/// OCR module - defines the OcrEngine trait and related types for text detection and recognition
///
/// This module provides abstractions for OCR (Optical Character Recognition) using PaddleOCR v5
/// with MNN backend. It defines the core types for passing image data, receiving OCR results,
/// and handling errors.

use std::path::Path;
use thiserror::Error;

// ============================================================================
// Type Definitions
// ============================================================================

/// Image buffer for passing BGR pixel data to OCR engine
///
/// Represents a raw image buffer in BGR format (Blue-Green-Red, 8-bit per channel).
/// This is the standard format expected by the OCR preprocessing pipeline.
///
/// # Layout
/// - Pixel data is stored row-by-row in BGR order
/// - Each pixel occupies 3 bytes: [B, G, R]
/// - Total size: width × height × 3 bytes
#[derive(Debug, Clone)]
pub struct ImageBuffer {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Raw pixel data in BGR format (interleaved)
    pub data: Vec<u8>,
}

impl ImageBuffer {
    /// Creates a new ImageBuffer with the given dimensions and data
    ///
    /// # Arguments
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `data` - Raw BGR pixel data (length must be width × height × 3)
    ///
    /// # Panics
    /// Panics if data length doesn't match width × height × 3
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            (width * height * 3) as usize,
            "ImageBuffer data length must match width × height × 3"
        );
        Self {
            width,
            height,
            data,
        }
    }

    /// Returns the expected data length for the given dimensions
    pub fn expected_len(width: u32, height: u32) -> usize {
        (width * height * 3) as usize
    }
}

/// OCR detection result - a single text box with bounding box, text, and confidence
///
/// Represents a detected text region from OCR processing. The bounding box is defined
/// by 4 corner points (quadrilateral) to handle rotated or perspective-distorted text.
///
/// # Coordinate System
/// - Origin (0,0) is top-left corner of the image
/// - X-axis increases to the right
/// - Y-axis increases downward
/// - Points are ordered: top-left, top-right, bottom-right, bottom-left
#[derive(Debug, Clone)]
pub struct OcrBox {
    /// Bounding box as 4 corner points: [[x, y]; 4]
    /// Order: [top-left, top-right, bottom-right, bottom-left]
    pub bbox: [[i32; 2]; 4],
    /// Recognized text content
    pub text: String,
    /// Recognition confidence score (0.0 to 1.0)
    pub confidence: f32,
}

impl OcrBox {
    /// Creates a new OcrBox with the given parameters
    pub fn new(bbox: [[i32; 2]; 4], text: String, confidence: f32) -> Self {
        Self {
            bbox,
            text,
            confidence,
        }
    }

    /// Returns the top-left corner of the bounding box
    pub fn top_left(&self) -> [i32; 2] {
        self.bbox[0]
    }

    /// Returns the top-right corner of the bounding box
    pub fn top_right(&self) -> [i32; 2] {
        self.bbox[1]
    }

    /// Returns the bottom-right corner of the bounding box
    pub fn bottom_right(&self) -> [i32; 2] {
        self.bbox[2]
    }

    /// Returns the bottom-left corner of the bounding box
    pub fn bottom_left(&self) -> [i32; 2] {
        self.bbox[3]
    }

    /// Calculates the center Y coordinate of the bounding box
    pub fn center_y(&self) -> i32 {
        (self.bbox[0][1] + self.bbox[2][1]) / 2
    }

    /// Calculates the height of the bounding box (approximate, using vertical extent)
    pub fn height(&self) -> i32 {
        (self.bbox[2][1] - self.bbox[0][1]).abs()
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// OCR error type - represents all possible OCR operation failures
///
/// This error type covers initialization errors (model loading), runtime errors
/// (inference failures), and input validation errors.
#[derive(Error, Debug)]
pub enum OcrError {
    /// Model file not found or inaccessible
    #[error("Model file not found: {0}")]
    ModelNotFound(String),

    /// Failed to load or initialize OCR models
    #[error("Failed to initialize OCR engine: {0}")]
    InitializationFailed(String),

    /// Invalid image buffer (wrong dimensions, empty data, etc.)
    #[error("Invalid image buffer: {0}")]
    InvalidImageBuffer(String),

    /// OCR inference failed during processing
    #[error("OCR inference failed: {0}")]
    InferenceFailed(String),

    /// I/O error while accessing model files
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic error for unexpected failures
    #[error("OCR error: {0}")]
    Other(String),
}

// ============================================================================
// OcrEngine Trait
// ============================================================================

/// OCR Engine trait - defines the interface for text detection and recognition
///
/// This trait abstracts the OCR engine implementation, allowing different backends
/// (e.g., rust-paddle-ocr, custom FFI to C++) to be used interchangeably.
///
/// # Lifecycle
/// 1. Create engine instance
/// 2. Call `init()` once with model paths to load models into memory
/// 3. Call `run()` multiple times with different images for OCR processing
/// 4. Models remain in memory for the lifetime of the engine
///
/// # Thread Safety
/// Implementations must be `Send` to allow moving between threads, but not required
/// to be `Sync` (engine can be single-threaded, used via channels).
///
/// # Requirements
/// - **4.1**: Load detection and recognition models once at initialization
/// - **4.2**: Perform text detection and recognition without disk I/O (in-memory)
/// - **4.3**: Return list of text boxes with bbox, text content, and confidence
/// - **4.5**: Operate in-process via FFI without spawning external processes
pub trait OcrEngine: Send {
    /// Initializes the OCR engine by loading models
    ///
    /// This method loads the PaddleOCR detection model, recognition model, and
    /// character dictionary into memory. Models remain loaded for the lifetime
    /// of the engine instance, eliminating per-frame loading overhead.
    ///
    /// # Arguments
    /// * `det_model` - Path to detection model file (.mnn)
    /// * `rec_model` - Path to recognition model file (.mnn)
    /// * `keys` - Path to character dictionary file (.txt)
    ///
    /// # Returns
    /// * `Ok(())` if models loaded successfully
    /// * `Err(OcrError)` if model loading failed
    ///
    /// # Errors
    /// - `OcrError::ModelNotFound` if any model file doesn't exist
    /// - `OcrError::InitializationFailed` if model loading or parsing fails
    /// - `OcrError::IoError` if file reading fails
    ///
    /// # Example
    /// ```ignore
    /// let mut engine = create_ocr_engine();
    /// engine.init(
    ///     Path::new("models/det.mnn"),
    ///     Path::new("models/rec.mnn"),
    ///     Path::new("models/keys.txt")
    /// )?;
    /// ```
    fn init(&mut self, det_model: &Path, rec_model: &Path, keys: &Path) -> Result<(), OcrError>;

    /// Runs OCR on the provided image buffer
    ///
    /// Performs text detection and recognition on the input image. The image should
    /// be preprocessed (scaled, enhanced) before passing to this method.
    ///
    /// # Arguments
    /// * `img` - Image buffer in BGR format
    ///
    /// # Returns
    /// * `Ok(Vec<OcrBox>)` - List of detected text boxes (may be empty if no text found)
    /// * `Err(OcrError)` - If OCR processing fails
    ///
    /// # Errors
    /// - `OcrError::InvalidImageBuffer` if image dimensions are invalid or data is corrupt
    /// - `OcrError::InferenceFailed` if OCR processing fails
    ///
    /// # Behavior
    /// - Returns empty vector if no text is detected (not an error)
    /// - All processing happens in-memory (no disk I/O)
    /// - Thread-safe when used from a single thread or via message passing
    ///
    /// # Example
    /// ```ignore
    /// let image = ImageBuffer::new(640, 480, pixels);
    /// let results = engine.run(&image)?;
    /// for ocr_box in results {
    ///     println!("Found text: {} (confidence: {})", ocr_box.text, ocr_box.confidence);
    /// }
    /// ```
    fn run(&mut self, img: &ImageBuffer) -> Result<Vec<OcrBox>, OcrError>;

    /// Detects text regions only (no recognition).
    ///
    /// Much faster than `run` when only box *positions* are needed (e.g. to
    /// auto-locate the subtitle region on a full-window scan). Returned boxes
    /// have empty `text` and confidence `1.0`. Default falls back to `run`.
    fn detect(&mut self, img: &ImageBuffer) -> Result<Vec<OcrBox>, OcrError> {
        self.run(img)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_buffer_new() {
        let width = 10;
        let height = 5;
        let data = vec![0u8; (width * height * 3) as usize];
        let buffer = ImageBuffer::new(width, height, data.clone());

        assert_eq!(buffer.width, width);
        assert_eq!(buffer.height, height);
        assert_eq!(buffer.data.len(), (width * height * 3) as usize);
    }

    #[test]
    #[should_panic(expected = "ImageBuffer data length must match width × height × 3")]
    fn test_image_buffer_wrong_size() {
        let width = 10;
        let height = 5;
        let data = vec![0u8; 100]; // Wrong size
        ImageBuffer::new(width, height, data);
    }

    #[test]
    fn test_image_buffer_expected_len() {
        assert_eq!(ImageBuffer::expected_len(10, 5), 150);
        assert_eq!(ImageBuffer::expected_len(640, 480), 921600);
    }

    #[test]
    fn test_ocr_box_coordinates() {
        let bbox = [[10, 20], [50, 20], [50, 40], [10, 40]];
        let ocr_box = OcrBox::new(bbox, "test".to_string(), 0.95);

        assert_eq!(ocr_box.top_left(), [10, 20]);
        assert_eq!(ocr_box.top_right(), [50, 20]);
        assert_eq!(ocr_box.bottom_right(), [50, 40]);
        assert_eq!(ocr_box.bottom_left(), [10, 40]);
        assert_eq!(ocr_box.center_y(), 30);
        assert_eq!(ocr_box.height(), 20);
    }

    #[test]
    fn test_ocr_error_display() {
        let err = OcrError::ModelNotFound("test.mnn".to_string());
        assert_eq!(err.to_string(), "Model file not found: test.mnn");

        let err = OcrError::InvalidImageBuffer("empty data".to_string());
        assert_eq!(err.to_string(), "Invalid image buffer: empty data");
    }
}
