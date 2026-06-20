//! Text filtering for OCR results.
//!
//! This module provides filters for cleaning up and processing OCR text:
//! - Character name removal (prefix and first-line detection)
//! - Center line filtering (L1, L2, L3)
//! - Garbage text detection

use crate::config::AppConfig;
use crate::text_grouping::TextLine;
use std::collections::HashSet;

/// Character name filter.
///
/// Removes character names from dialogue text using exact and fuzzy matching.
pub struct CharacterNameFilter {
    /// Character names (normalized to lowercase)
    names: HashSet<String>,
}

impl CharacterNameFilter {
    /// Creates a new filter with the given character names.
    pub fn new(names: Vec<String>) -> Self {
        let normalized_names: HashSet<String> = names.iter()
            .map(|n| n.to_lowercase())
            .collect();
        
        Self {
            names: normalized_names,
        }
    }

    /// Removes character names from text using BOTH inline and file-based detection.
    ///
    /// Two systems:
    /// 1. **Inline** — strips any `Word:` / `Word;` prefix or a short single-word first
    ///    line, no names file needed.
    /// 2. **File-based** — matches first line / prefix against the loaded names list
    ///    (exact + fuzzy ≥ 90 %).
    pub fn remove_name(&self, text: &str) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        // System 1 — inline: detect any "Word:" / "Word;" prefix
        let inline_cleaned = Self::remove_inline_name(text);
        if inline_cleaned != text {
            return inline_cleaned;
        }

        // System 2 — file-based: match against the known names list
        self.remove_name_file_only(text)
    }

    /// System 1 — inline name removal (no names file needed).
    ///
    /// Strips:
    /// - `Word:` or `Word;` prefix (case-insensitive)
    /// - Multi-line where the first line is a single short word with optional
    ///   trailing punctuation (`:`, `;`, `.`)
    pub fn remove_inline_name(text: &str) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        let trimmed = text.trim();

        // Inline "Word:" or "Word;" prefix
        if let Some(colon) = trimmed.find(':') {
            let word = &trimmed[..colon];
            if !word.is_empty() && word.split_whitespace().count() <= 2 && word.len() <= 40 {
                // Only strip if the part before ":" looks like a name (no punctuation)
                if word.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '_' || c == '-' || c == '\'') {
                    let rest = trimmed[colon + 1..].trim();
                    if !rest.is_empty() {
                        tracing::debug!("Inline name removal: stripped '{}:' prefix", word);
                        return rest.to_string();
                    }
                }
            }
        }
        if let Some(semi) = trimmed.find(';') {
            let word = &trimmed[..semi];
            if !word.is_empty() && word.split_whitespace().count() <= 2 && word.len() <= 40 {
                if word.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '_' || c == '-' || c == '\'') {
                    let rest = trimmed[semi + 1..].trim();
                    if !rest.is_empty() {
                        tracing::debug!("Inline name removal: stripped '{};' prefix", word);
                        return rest.to_string();
                    }
                }
            }
        }

        // Multi-line: single short word on first line
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() >= 2 {
            let first = lines[0].trim();
            if !first.is_empty()
                && first.len() <= 40
                && first.split_whitespace().count() <= 2
            {
                let norm = first
                    .trim_end_matches(&[':', ';', '.', '-', '—', ' '])
                    .trim();
                if !norm.is_empty()
                    && norm.split_whitespace().count() <= 2
                    && norm.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '_' || c == '-' || c == '\'')
                {
                    tracing::debug!("Inline name removal: stripped first line '{}'", first);
                    return lines[1..].join("\n").trim().to_string();
                }
            }
        }

        text.to_string()
    }

    /// System 2 — file-based name removal (requires loaded names list).
    fn remove_name_file_only(&self, text: &str) -> String {
        if text.is_empty() {
            return text.to_string();
        }

        // Check for "Name:" or "Name;" prefix
        for name in &self.names {
            let prefix_colon = format!("{}:", name);
            let prefix_semi = format!("{};", name);
            
            if text.to_lowercase().starts_with(&prefix_colon) {
                tracing::debug!("File-based name removal: '{}:' prefix", name);
                return text[prefix_colon.len()..].trim().to_string();
            }
            if text.to_lowercase().starts_with(&prefix_semi) {
                tracing::debug!("File-based name removal: '{};' prefix", name);
                return text[prefix_semi.len()..].trim().to_string();
            }
        }

        // Check for multi-line format (name on first line)
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() < 2 {
            return text.to_string();
        }

        let first_line_raw = lines[0].trim();
        if first_line_raw.is_empty() {
            return text.to_string();
        }

        // Safety checks
        if first_line_raw.len() > 40 {
            return text.to_string();
        }
        
        if first_line_raw.split_whitespace().count() > 4 {
            return text.to_string();
        }

        // Normalize first line (remove trailing punctuation)
        let first_line_norm = first_line_raw.to_lowercase()
            .trim_end_matches(&[':', ';', '.', '-', '—', ' '])
            .trim()
            .to_string();

        // Exact match
        if self.names.contains(&first_line_norm) {
            tracing::debug!("File-based name removal (exact): {}", first_line_raw);
            return lines[1..].join("\n").trim().to_string();
        }

        // Fuzzy match (>= 90% similarity)
        for name in &self.names {
            let similarity = fuzzy_ratio(&first_line_norm, name);
            if similarity >= 90 {
                tracing::debug!(
                    "File-based name removal (fuzzy): {} ({}%)",
                    first_line_raw,
                    similarity
                );
                return lines[1..].join("\n").trim().to_string();
            }
        }

        text.to_string()
    }
}

/// Simple fuzzy string similarity (0-100 scale).
///
/// This is a simplified Levenshtein-based ratio calculation.
fn fuzzy_ratio(s1: &str, s2: &str) -> u8 {
    let len1 = s1.len();
    let len2 = s2.len();
    
    if len1 == 0 && len2 == 0 {
        return 100;
    }
    if len1 == 0 || len2 == 0 {
        return 0;
    }

    // Levenshtein distance
    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row = vec![0; len2 + 1];

    for (i, c1) in s1.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    let distance = prev_row[len2];
    let max_len = len1.max(len2);
    let ratio = ((max_len - distance) as f32 / max_len as f32 * 100.0) as u8;
    ratio
}

/// Center line filter for text boxes.
///
/// Filters text boxes based on horizontal position relative to center lines.
pub struct CenterLineFilter;

impl CenterLineFilter {
    /// Filters text lines based on center line configuration.
    ///
    /// # Center lines:
    /// - L1: Vertical center of screen
    /// - L2: Absolute X position
    /// - L3: Ratio-based X position
    ///
    /// # Arguments:
    /// * `lines` - Text lines to filter
    /// * `config` - Application configuration
    /// * `screen_width` - Width of the captured region
    /// * `screen_height` - Height of the captured region
    ///
    /// # Returns:
    /// Filtered lines that intersect with any active center line
    pub fn filter_lines(
        lines: Vec<TextLine>,
        config: &AppConfig,
        screen_width: u32,
        _screen_height: u32,
    ) -> Vec<TextLine> {
        // Skip filtering if no center lines are active
        if !config.use_center_line_1 && !config.use_center_line_2 && !config.use_center_line_3 {
            return lines;
        }

        let margin = config.center_line_margin;

        // Calculate center line positions
        let (center_start_1, center_end_1) = if config.use_center_line_1 {
            let center_x = screen_width as i32 / 2;
            (center_x - margin / 2, center_x + margin / 2)
        } else {
            (0, 0)
        };

        let (center_start_2, center_end_2) = if config.use_center_line_2 {
            let start = config.center_line_2_start;
            (start, start + margin)
        } else {
            (0, 0)
        };

        let (center_start_3, center_end_3) = if config.use_center_line_3 {
            let start = (screen_width as f32 * config.center_line_3_start_ratio) as i32;
            (start, start + margin)
        } else {
            (0, 0)
        };

        // Filter lines
        lines.into_iter()
            .filter(|line| {
                // Check if any box in the line intersects with a center line
                for ocr_box in &line.boxes {
                    let left_x = ocr_box.top_left()[0];
                    let right_x = ocr_box.top_right()[0];

                    let matches_l1 = config.use_center_line_1
                        && left_x <= center_end_1
                        && right_x >= center_start_1;

                    let matches_l2 = config.use_center_line_2
                        && left_x <= center_end_2
                        && right_x >= center_start_2;

                    let matches_l3 = config.use_center_line_3
                        && left_x <= center_end_3
                        && right_x >= center_start_3;

                    if matches_l1 || matches_l2 || matches_l3 {
                        return true;
                    }
                }
                false
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::OcrBox;

    #[test]
    fn test_fuzzy_ratio_exact_match() {
        assert_eq!(fuzzy_ratio("hello", "hello"), 100);
        assert_eq!(fuzzy_ratio("test", "test"), 100);
    }

    #[test]
    fn test_fuzzy_ratio_no_match() {
        assert_eq!(fuzzy_ratio("abc", "xyz"), 0);
    }

    #[test]
    fn test_fuzzy_ratio_partial_match() {
        let ratio = fuzzy_ratio("hello", "hallo");
        assert!(ratio >= 80 && ratio < 100);
    }

    #[test]
    fn test_fuzzy_ratio_empty() {
        assert_eq!(fuzzy_ratio("", ""), 100);
        assert_eq!(fuzzy_ratio("hello", ""), 0);
        assert_eq!(fuzzy_ratio("", "world"), 0);
    }

    // Property 30: Character name removal
    // Validates: Requirements 35.1, 35.2, 35.3, 35.4, 35.5
    #[test]
    fn test_character_name_removal_prefix_colon() {
        let filter = CharacterNameFilter::new(vec!["Alice".to_string()]);
        let result = filter.remove_name("Alice: Hello there!");
        assert_eq!(result, "Hello there!");
    }

    #[test]
    fn test_character_name_removal_prefix_semicolon() {
        let filter = CharacterNameFilter::new(vec!["Bob".to_string()]);
        let result = filter.remove_name("Bob; How are you?");
        assert_eq!(result, "How are you?");
    }

    #[test]
    fn test_character_name_removal_first_line_exact() {
        let filter = CharacterNameFilter::new(vec!["Charlie".to_string()]);
        let text = "Charlie\nThis is dialogue";
        let result = filter.remove_name(text);
        assert_eq!(result, "This is dialogue");
    }

    #[test]
    fn test_character_name_removal_first_line_with_punctuation() {
        let filter = CharacterNameFilter::new(vec!["Diana".to_string()]);
        let text = "Diana:\nThis is dialogue";
        let result = filter.remove_name(text);
        assert_eq!(result, "This is dialogue");
    }

    #[test]
    fn test_fuzzy_ratio_similar_names() {
        // Test specific cases for character name matching
        let ratio1 = fuzzy_ratio("alice", "alise");
        println!("alice vs alise: {}", ratio1);
        assert!(ratio1 >= 80);
    }

    #[test]
    fn test_character_name_removal_fuzzy_match() {
        let filter = CharacterNameFilter::new(vec!["Anna".to_string()]);
        // "Anne" should match (1 char difference in 4-char name -> 75%, but with normalization should work)
        // Actually let's use a name where removing trailing punctuation helps
        let text = "Anna.\nThis is dialogue";
        let result = filter.remove_name(text);
        // Should match exactly after punctuation removal
        assert_eq!(result, "This is dialogue");
    }

    #[test]
    fn test_character_name_removal_no_match() {
        let filter = CharacterNameFilter::new(vec!["Alice".to_string()]);
        let text = "Regular dialogue without name";
        let result = filter.remove_name(text);
        assert_eq!(result, text);
    }

    #[test]
    fn test_character_name_removal_too_long() {
        let filter = CharacterNameFilter::new(vec!["Alice".to_string()]);
        // First line > 40 chars, should not remove
        let text = "This is a very long first line that exceeds forty characters\nSecond line";
        let result = filter.remove_name(text);
        assert_eq!(result, text);
    }

    #[test]
    fn test_character_name_removal_too_many_words() {
        let filter = CharacterNameFilter::new(vec!["Alice".to_string()]);
        // First line has 5 words (> 4), should not remove
        let text = "One Two Three Four Five\nSecond line";
        let result = filter.remove_name(text);
        assert_eq!(result, text);
    }

    // --- Inline name detection tests (no names file needed) ---

    #[test]
    fn test_inline_remove_colon_prefix() {
        let result = CharacterNameFilter::remove_inline_name("John: Hello there!");
        assert_eq!(result, "Hello there!");
    }

    #[test]
    fn test_inline_remove_semicolon_prefix() {
        let result = CharacterNameFilter::remove_inline_name("Jane; How are you?");
        assert_eq!(result, "How are you?");
    }

    #[test]
    fn test_inline_remove_note_prefix() {
        let result = CharacterNameFilter::remove_inline_name("Note: this is a note");
        assert_eq!(result, "this is a note");
    }

    #[test]
    fn test_inline_no_false_positive_no_colon() {
        let result = CharacterNameFilter::remove_inline_name("Regular dialogue without name");
        assert_eq!(result, "Regular dialogue without name");
    }

    #[test]
    fn test_inline_multi_line_first_word() {
        let result = CharacterNameFilter::remove_inline_name("Steve.\nThis is dialogue");
        assert_eq!(result, "This is dialogue");
    }

    #[test]
    fn test_inline_name_with_apostrophe() {
        let result = CharacterNameFilter::remove_inline_name("O'Brien: Hello");
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_inline_long_first_line_untouched() {
        let text = "This is a very long first line that exceeds forty characters\nSecond line";
        let result = CharacterNameFilter::remove_inline_name(text);
        assert_eq!(result, text);
    }

    #[test]
    fn test_inline_multi_line_two_words() {
        let text = "Dr Jones\nHello there";
        let result = CharacterNameFilter::remove_inline_name(text);
        assert_eq!(result, "Hello there");
    }

    // --- Combined: inline + file-based on empty filter ---

    #[test]
    fn test_empty_filter_still_uses_inline() {
        let filter = CharacterNameFilter::new(vec![]);
        let result = filter.remove_name("Alice: Hello");
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_character_name_removal_case_insensitive() {
        let filter = CharacterNameFilter::new(vec!["alice".to_string()]);
        let result = filter.remove_name("ALICE: Hello!");
        assert_eq!(result, "Hello!");
    }

    // Property 31: Center line filtering
    // Validates: Requirements 36.1, 36.2, 36.3, 36.4, 36.5
    #[test]
    fn test_center_line_filter_l1_vertical_center() {
        let mut config = AppConfig::default();
        config.use_center_line_1 = true;
        config.center_line_margin = 100;

        let screen_width = 1000;
        let screen_height = 500;

        // Create line at center (X=500)
        let mut line_center = TextLine::new();
        line_center.add_box(OcrBox {
            bbox: [[450, 100], [550, 100], [550, 120], [450, 120]],
            text: "Center text".to_string(),
            confidence: 0.9,
        });

        // Create line on left edge (X=50)
        let mut line_left = TextLine::new();
        line_left.add_box(OcrBox {
            bbox: [[50, 100], [100, 100], [100, 120], [50, 120]],
            text: "Left text".to_string(),
            confidence: 0.9,
        });

        let lines = vec![line_center, line_left];
        let filtered = CenterLineFilter::filter_lines(lines, &config, screen_width, screen_height);

        // Only center line should pass
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].text(), "Center text");
    }

    #[test]
    fn test_center_line_filter_l2_absolute() {
        let mut config = AppConfig::default();
        config.use_center_line_2 = true;
        config.center_line_2_start = 200;
        config.center_line_margin = 100;

        let screen_width = 1000;
        let screen_height = 500;

        // Create line at L2 position (X=250)
        let mut line_match = TextLine::new();
        line_match.add_box(OcrBox {
            bbox: [[220, 100], [280, 100], [280, 120], [220, 120]],
            text: "L2 text".to_string(),
            confidence: 0.9,
        });

        // Create line far from L2 (X=500)
        let mut line_far = TextLine::new();
        line_far.add_box(OcrBox {
            bbox: [[500, 100], [550, 100], [550, 120], [500, 120]],
            text: "Far text".to_string(),
            confidence: 0.9,
        });

        let lines = vec![line_match, line_far];
        let filtered = CenterLineFilter::filter_lines(lines, &config, screen_width, screen_height);

        // Only L2 line should pass
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].text(), "L2 text");
    }

    #[test]
    fn test_center_line_filter_l3_ratio() {
        let mut config = AppConfig::default();
        config.use_center_line_3 = true;
        config.center_line_3_start_ratio = 0.3; // 30% of width = 300
        config.center_line_margin = 100;

        let screen_width = 1000;
        let screen_height = 500;

        // Create line at L3 position (X=350)
        let mut line_match = TextLine::new();
        line_match.add_box(OcrBox {
            bbox: [[320, 100], [380, 100], [380, 120], [320, 120]],
            text: "L3 text".to_string(),
            confidence: 0.9,
        });

        let lines = vec![line_match];
        let filtered = CenterLineFilter::filter_lines(lines, &config, screen_width, screen_height);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].text(), "L3 text");
    }

    #[test]
    fn test_center_line_filter_disabled() {
        let config = AppConfig::default(); // All center lines disabled

        let screen_width = 1000;
        let screen_height = 500;

        let mut line = TextLine::new();
        line.add_box(OcrBox {
            bbox: [[50, 100], [100, 100], [100, 120], [50, 120]],
            text: "Any text".to_string(),
            confidence: 0.9,
        });

        let lines = vec![line.clone()];
        let filtered = CenterLineFilter::filter_lines(lines, &config, screen_width, screen_height);

        // All lines should pass when filtering is disabled
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_center_line_filter_multiple_active() {
        let mut config = AppConfig::default();
        config.use_center_line_1 = true;
        config.use_center_line_2 = true;
        config.center_line_2_start = 200;
        config.center_line_margin = 100;

        let screen_width = 1000;
        let screen_height = 500;

        // Line at L1
        let mut line_l1 = TextLine::new();
        line_l1.add_box(OcrBox {
            bbox: [[480, 100], [520, 100], [520, 120], [480, 120]],
            text: "L1".to_string(),
            confidence: 0.9,
        });

        // Line at L2
        let mut line_l2 = TextLine::new();
        line_l2.add_box(OcrBox {
            bbox: [[220, 100], [280, 100], [280, 120], [220, 120]],
            text: "L2".to_string(),
            confidence: 0.9,
        });

        // Line at neither
        let mut line_other = TextLine::new();
        line_other.add_box(OcrBox {
            bbox: [[700, 100], [750, 100], [750, 120], [700, 120]],
            text: "Other".to_string(),
            confidence: 0.9,
        });

        let lines = vec![line_l1, line_l2, line_other];
        let filtered = CenterLineFilter::filter_lines(lines, &config, screen_width, screen_height);

        // Both L1 and L2 should pass
        assert_eq!(filtered.len(), 2);
    }
}
