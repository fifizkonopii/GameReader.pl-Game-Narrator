//! Text box grouping and line detection for OCR results.
//!
//! This module organizes OCR detected text boxes into logical lines and paragraphs
//! based on spatial proximity and vertical alignment.

use crate::config::AppConfig;
use crate::constants::LINE_THRESHOLD;
use crate::ocr::OcrBox;

/// A grouped line of text boxes.
#[derive(Debug, Clone)]
pub struct TextLine {
    /// Text boxes in this line (left to right order)
    pub boxes: Vec<OcrBox>,
    /// Mean Y coordinate of the line
    pub y_mean: i32,
}

impl TextLine {
    /// Creates a new empty text line.
    pub fn new() -> Self {
        Self {
            boxes: Vec::new(),
            y_mean: 0,
        }
    }

    /// Adds a box to the line and updates the mean Y coordinate.
    pub fn add_box(&mut self, ocr_box: OcrBox) {
        self.boxes.push(ocr_box);
        self.recalculate_y_mean();
    }

    /// Recalculates the mean Y coordinate based on all boxes in the line.
    fn recalculate_y_mean(&mut self) {
        if self.boxes.is_empty() {
            self.y_mean = 0;
            return;
        }
        
        let sum: i32 = self.boxes.iter().map(|b| b.center_y()).sum();
        self.y_mean = sum / self.boxes.len() as i32;
    }

    /// Concatenates all text from boxes in the line.
    pub fn text(&self) -> String {
        self.boxes.iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Sorts boxes within the line by X coordinate (left to right).
    pub fn sort_by_x(&mut self) {
        self.boxes.sort_by_key(|b| b.top_left()[0]);
    }

    /// Returns true if the line is empty.
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }
}

/// Groups OCR boxes into lines based on vertical proximity.
///
/// # Algorithm:
/// 1. Sort boxes by (y, x) coordinates
/// 2. Filter by height range and confidence (if typewriter mode)
/// 3. Group boxes into lines by Y proximity (within `line_threshold`)
/// 4. Sort boxes within each line by X coordinate
///
/// # Arguments:
/// * `boxes` - OCR detection results
/// * `config` - Application configuration (for thresholds and filters)
/// * `scale` - Downscale factor used for OCR (to convert heights back to original scale)
///
/// # Returns:
/// Vector of text lines, each containing grouped boxes
pub fn group_into_lines(boxes: Vec<OcrBox>, config: &AppConfig, scale: f32) -> Vec<TextLine> {
    if boxes.is_empty() {
        return Vec::new();
    }

    // Step 1: Sort by (y, x)
    let mut sorted_boxes = boxes;
    sorted_boxes.sort_by_key(|b| (b.top_left()[1], b.top_left()[0]));

    // Step 2: Filter boxes
    let filtered_boxes: Vec<OcrBox> = sorted_boxes.into_iter()
        .filter(|b| {
            // Calculate original height (before downscaling)
            let height_scaled = b.height();
            let height_original = (height_scaled as f32 / scale) as i32;

            // Height filter
            if height_original < config.min_height || height_original > config.max_height {
                return false;
            }

            // Confidence filter (only in typewriter mode)
            if config.enable_typewriter_wait && b.confidence < config.ocr_min_confidence {
                return false;
            }

            true
        })
        .collect();

    if filtered_boxes.is_empty() {
        return Vec::new();
    }

    // Step 3: Group into lines by vertical proximity
    let line_threshold = (LINE_THRESHOLD as f32 * scale).round() as i32;
    let mut lines: Vec<TextLine> = Vec::new();

    for ocr_box in filtered_boxes {
        let box_center_y = ocr_box.center_y();

        // Find existing line within threshold
        let mut found_line = false;
        for line in &mut lines {
            if (box_center_y - line.y_mean).abs() < line_threshold {
                line.add_box(ocr_box.clone());
                found_line = true;
                break;
            }
        }

        // Create new line if no match
        if !found_line {
            let mut new_line = TextLine::new();
            new_line.add_box(ocr_box);
            lines.push(new_line);
        }
    }

    // Step 4: Sort boxes within each line by X
    for line in &mut lines {
        line.sort_by_x();
    }

    lines
}

/// Clusters lines into paragraphs based on vertical gap threshold.
///
/// # Algorithm:
/// - Group consecutive lines into paragraphs using vertical gap threshold (`line_threshold * 4`)
/// - Output depends on `enable_paragraph_ocr` setting:
///   - false: Returns single concatenated text from all lines
///   - true: Returns list of paragraph texts
///
/// # Arguments:
/// * `lines` - Text lines from `group_into_lines`
/// * `config` - Application configuration
/// * `scale` - Downscale factor used for OCR
///
/// # Returns:
/// Vector of paragraph texts (single element if paragraph mode disabled)
pub fn cluster_into_paragraphs(lines: Vec<TextLine>, config: &AppConfig, scale: f32) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    if !config.enable_paragraph_ocr {
        // Single text mode: concatenate all lines
        let all_text = lines.iter()
            .map(|line| line.text())
            .collect::<Vec<_>>()
            .join("\n");
        return vec![all_text];
    }

    // Paragraph mode: cluster by vertical gaps
    let paragraph_gap_threshold = (LINE_THRESHOLD as f32 * scale * 4.0).round() as i32;
    let mut paragraphs: Vec<Vec<String>> = Vec::new();
    let mut current_paragraph: Vec<String> = Vec::new();
    let mut last_y_mean: Option<i32> = None;

    for line in lines {
        if let Some(last_y) = last_y_mean {
            let gap = line.y_mean - last_y;
            
            // Large gap -> start new paragraph
            if gap > paragraph_gap_threshold {
                if !current_paragraph.is_empty() {
                    paragraphs.push(current_paragraph);
                    current_paragraph = Vec::new();
                }
            }
        }

        current_paragraph.push(line.text());
        last_y_mean = Some(line.y_mean);
    }

    // Add last paragraph
    if !current_paragraph.is_empty() {
        paragraphs.push(current_paragraph);
    }

    // Join lines within each paragraph
    paragraphs.iter()
        .map(|para| para.join("\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_box(x: i32, y: i32, width: i32, height: i32, text: &str, confidence: f32) -> OcrBox {
        OcrBox {
            bbox: [
                [x, y],
                [x + width, y],
                [x + width, y + height],
                [x, y + height],
            ],
            text: text.to_string(),
            confidence,
        }
    }

    fn default_test_config() -> AppConfig {
        AppConfig {
            min_height: 10,
            max_height: 100,
            enable_typewriter_wait: false,
            ocr_min_confidence: 0.4,
            ..Default::default()
        }
    }

    #[test]
    fn test_text_line_new() {
        let line = TextLine::new();
        assert!(line.is_empty());
        assert_eq!(line.y_mean, 0);
    }

    #[test]
    fn test_text_line_add_box() {
        let mut line = TextLine::new();
        let box1 = create_test_box(10, 20, 50, 30, "Hello", 0.9);
        
        line.add_box(box1);
        
        assert_eq!(line.boxes.len(), 1);
        assert_eq!(line.y_mean, 35); // (20 + 50) / 2
    }

    #[test]
    fn test_text_line_text_concatenation() {
        let mut line = TextLine::new();
        line.add_box(create_test_box(10, 20, 50, 20, "Hello", 0.9));
        line.add_box(create_test_box(70, 20, 50, 20, "World", 0.9));
        
        assert_eq!(line.text(), "Hello World");
    }

    #[test]
    fn test_text_line_sort_by_x() {
        let mut line = TextLine::new();
        line.add_box(create_test_box(100, 20, 50, 20, "Second", 0.9));
        line.add_box(create_test_box(10, 20, 50, 20, "First", 0.9));
        
        line.sort_by_x();
        
        assert_eq!(line.boxes[0].text, "First");
        assert_eq!(line.boxes[1].text, "Second");
    }

    // Property 14: Text box sorting order
    // Validates: Requirement 6.1
    #[test]
    fn test_group_into_lines_sorting() {
        let config = default_test_config();
        
        // Create boxes in random order
        let boxes = vec![
            create_test_box(100, 50, 50, 20, "C", 0.9),  // Middle Y, right X
            create_test_box(10, 20, 50, 20, "A", 0.9),   // Top Y, left X
            create_test_box(100, 20, 50, 20, "B", 0.9),  // Top Y, right X
            create_test_box(10, 50, 50, 20, "D", 0.9),   // Middle Y, left X
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        
        // Should have 2 lines
        assert_eq!(lines.len(), 2);
        
        // First line (Y ≈ 20): A B
        assert_eq!(lines[0].boxes.len(), 2);
        assert_eq!(lines[0].boxes[0].text, "A");
        assert_eq!(lines[0].boxes[1].text, "B");
        
        // Second line (Y ≈ 50): D C
        assert_eq!(lines[1].boxes.len(), 2);
        assert_eq!(lines[1].boxes[0].text, "D");
        assert_eq!(lines[1].boxes[1].text, "C");
    }

    // Property 10: Height filter correctness
    // Validates: Requirement 6.2
    #[test]
    fn test_height_filter() {
        let mut config = default_test_config();
        config.min_height = 20;
        config.max_height = 50;
        
        let boxes = vec![
            create_test_box(10, 20, 50, 15, "Too small", 0.9),  // height = 15 < 20
            create_test_box(10, 40, 50, 30, "Good", 0.9),        // height = 30 (in range)
            create_test_box(10, 80, 50, 60, "Too large", 0.9),  // height = 60 > 50
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        
        // Only middle box should pass filter
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].boxes.len(), 1);
        assert_eq!(lines[0].boxes[0].text, "Good");
    }

    // Property 26: Confidence filtering in typewriter mode
    // Validates: Requirement 6.4
    #[test]
    fn test_confidence_filter_typewriter_mode() {
        let mut config = default_test_config();
        config.enable_typewriter_wait = true;
        config.ocr_min_confidence = 0.5;
        
        let boxes = vec![
            create_test_box(10, 20, 50, 30, "Low confidence", 0.3),  // Below threshold
            create_test_box(10, 50, 50, 30, "High confidence", 0.8), // Above threshold
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        
        // Only high confidence box should pass
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].boxes[0].text, "High confidence");
    }

    #[test]
    fn test_confidence_not_filtered_when_typewriter_disabled() {
        let mut config = default_test_config();
        config.enable_typewriter_wait = false;
        config.ocr_min_confidence = 0.5;
        
        let boxes = vec![
            create_test_box(10, 20, 50, 30, "Low confidence", 0.3),  // Below threshold but should pass
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        
        // Should pass because typewriter mode is disabled
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].boxes[0].text, "Low confidence");
    }

    // Property 27: Line grouping by vertical proximity
    // Validates: Requirement 6.1
    #[test]
    fn test_line_grouping_by_vertical_proximity() {
        let config = default_test_config();
        
        // LINE_THRESHOLD = 10, so boxes within 10 pixels should group
        let boxes = vec![
            create_test_box(10, 100, 50, 20, "Line1-A", 0.9),  // Y center = 110
            create_test_box(70, 105, 50, 20, "Line1-B", 0.9),  // Y center = 115 (within 10 of 110)
            create_test_box(10, 150, 50, 20, "Line2-A", 0.9),  // Y center = 160 (far from line 1)
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].boxes.len(), 2); // Line 1 has 2 boxes
        assert_eq!(lines[1].boxes.len(), 1); // Line 2 has 1 box
    }

    #[test]
    fn test_empty_boxes() {
        let config = default_test_config();
        let lines = group_into_lines(vec![], &config, 1.0);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_scale_affects_height_filter() {
        let mut config = default_test_config();
        config.min_height = 20;
        config.max_height = 50;
        
        // Box has height 15 at scale 1.0, but at scale 0.5 it represents height 30 in original
        let boxes = vec![
            create_test_box(10, 20, 50, 15, "Scaled", 0.9),
        ];
        
        // At scale 0.5, height 15 -> original height 30 (15 / 0.5)
        let lines = group_into_lines(boxes, &config, 0.5);
        
        // Should pass because original height (30) is in range [20, 50]
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].boxes[0].text, "Scaled");
    }

    // Property 28: Paragraph clustering
    // Validates: Requirements 6.5, 38.1, 38.2, 38.3, 38.4
    #[test]
    fn test_paragraph_clustering_single_text_mode() {
        let mut config = default_test_config();
        config.enable_paragraph_ocr = false;
        
        let boxes = vec![
            create_test_box(10, 20, 50, 20, "Line 1", 0.9),
            create_test_box(10, 100, 50, 20, "Line 2", 0.9),
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        let paragraphs = cluster_into_paragraphs(lines, &config, 1.0);
        
        // Should return single concatenated text
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0], "Line 1\nLine 2");
    }

    #[test]
    fn test_paragraph_clustering_multi_paragraph_mode() {
        let mut config = default_test_config();
        config.enable_paragraph_ocr = true;
        
        // Create boxes with large gap between line 1 and 2 (should split into 2 paragraphs)
        // LINE_THRESHOLD = 10, so gap threshold = 40
        let boxes = vec![
            create_test_box(10, 20, 50, 20, "Para1-Line1", 0.9),  // Y center = 30
            create_test_box(10, 30, 50, 20, "Para1-Line2", 0.9),  // Y center = 40 (gap = 10, same paragraph)
            create_test_box(10, 100, 50, 20, "Para2-Line1", 0.9), // Y center = 110 (gap = 70 > 40, new paragraph)
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        let paragraphs = cluster_into_paragraphs(lines, &config, 1.0);
        
        // Should return 2 paragraphs
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0], "Para1-Line1\nPara1-Line2");
        assert_eq!(paragraphs[1], "Para2-Line1");
    }

    #[test]
    fn test_paragraph_clustering_empty_lines() {
        let config = default_test_config();
        let lines = Vec::new();
        let paragraphs = cluster_into_paragraphs(lines, &config, 1.0);
        
        assert!(paragraphs.is_empty());
    }

    #[test]
    fn test_paragraph_clustering_single_line() {
        let mut config = default_test_config();
        config.enable_paragraph_ocr = true;
        
        let boxes = vec![
            create_test_box(10, 20, 50, 20, "Only line", 0.9),
        ];
        
        let lines = group_into_lines(boxes, &config, 1.0);
        let paragraphs = cluster_into_paragraphs(lines, &config, 1.0);
        
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0], "Only line");
    }
}
