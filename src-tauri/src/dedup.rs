//! Match deduplication and garbage text filtering.
//!
//! This module implements deduplication logic to prevent replaying the same dialogue
//! line multiple times, and garbage filtering to reject OCR artifacts and noise.

use std::collections::{HashMap, VecDeque};

/// Deduplication state for tracking match history.
#[derive(Debug, Clone)]
pub struct DedupState {
    /// History of last 2 matched line indices (1-based)
    index_history: VecDeque<usize>,
    
    /// Last matched line text for fuzzy similarity check
    last_text: Option<String>,
    
    /// Per-paragraph typewriter deduplication: paragraph_index -> line_index
    typewriter_matched: HashMap<usize, usize>,
    
    /// Empty frame counter for idle detection
    empty_reads: u32,
    
    /// Stable frame counter for typewriter disambiguation
    stable_count: u32,
    
    /// Last text fingerprint for stable frame detection
    last_fingerprint: Option<String>,
}

impl Default for DedupState {
    fn default() -> Self {
        Self::new()
    }
}

impl DedupState {
    /// Creates a new deduplication state.
    pub fn new() -> Self {
        Self {
            index_history: VecDeque::with_capacity(2),
            last_text: None,
            typewriter_matched: HashMap::new(),
            empty_reads: 0,
            stable_count: 0,
            last_fingerprint: None,
        }
    }

    /// Records a matched line index in history.
    pub fn record_match(&mut self, index: usize, text: String) {
        // Maintain history of last 2 indices
        if self.index_history.len() >= 2 {
            self.index_history.pop_front();
        }
        self.index_history.push_back(index);
        
        // Store last matched text
        self.last_text = Some(text);
        
        // Reset empty reads on successful match
        self.empty_reads = 0;
    }

    /// Records a typewriter match for a specific paragraph.
    pub fn record_typewriter_match(&mut self, paragraph_idx: usize, line_idx: usize) {
        self.typewriter_matched.insert(paragraph_idx, line_idx);
    }

    /// Checks if a line index is in recent history.
    pub fn is_duplicate_index(&self, index: usize) -> bool {
        self.index_history.contains(&index)
    }

    /// Checks if matched text is similar to last match using fuzzy similarity.
    pub fn is_duplicate_text(&self, text: &str, similarity_threshold: u8) -> bool {
        if let Some(last) = &self.last_text {
            let ratio = fuzzy_ratio(text, last);
            ratio >= similarity_threshold
        } else {
            false
        }
    }

    /// Checks if a line was already matched in this paragraph (typewriter mode).
    pub fn is_typewriter_duplicate(&self, paragraph_idx: usize, line_idx: usize) -> bool {
        self.typewriter_matched
            .get(&paragraph_idx)
            .map(|&matched_idx| matched_idx == line_idx)
            .unwrap_or(false)
    }

    /// Increments empty reads counter.
    pub fn increment_empty_reads(&mut self) {
        self.empty_reads += 1;
    }

    /// Resets empty reads counter.
    pub fn reset_empty_reads(&mut self) {
        self.empty_reads = 0;
    }

    /// Returns the current empty reads count.
    pub fn empty_reads(&self) -> u32 {
        self.empty_reads
    }

    /// Clears the typewriter matched cache.
    pub fn clear_typewriter_cache(&mut self) {
        self.typewriter_matched.clear();
    }

    /// Updates stable frame detection state.
    ///
    /// Returns true if frame is stable (same text for N consecutive frames).
    pub fn update_stable_detection(&mut self, text: &str, stable_threshold: u32) -> bool {
        let fingerprint = text.to_lowercase();
        
        if let Some(last) = &self.last_fingerprint {
            if &fingerprint == last {
                self.stable_count += 1;
            } else {
                self.stable_count = 1;
                self.last_fingerprint = Some(fingerprint);
            }
        } else {
            self.stable_count = 1;
            self.last_fingerprint = Some(fingerprint);
        }
        
        self.stable_count >= stable_threshold
    }

    /// Resets all state.
    pub fn reset(&mut self) {
        self.index_history.clear();
        self.last_text = None;
        self.typewriter_matched.clear();
        self.empty_reads = 0;
        self.stable_count = 0;
        self.last_fingerprint = None;
    }
}

/// Checks if text is garbage (too many non-letter characters or too short).
///
/// # Arguments
/// * `text` - Input text to validate
/// * `max_non_letter_ratio` - Maximum allowed proportion of non-letter chars (0.0-1.0)
/// * `min_length` - Minimum required text length
///
/// # Returns
/// * `true` if text is garbage and should be rejected
/// * `false` if text is valid
pub fn is_garbage_text(text: &str, max_non_letter_ratio: f32, min_length: usize) -> bool {
    let trimmed = text.trim();
    
    // Length check (count characters, not bytes, for correct multibyte handling)
    if trimmed.chars().count() < min_length {
        return true;
    }
    
    // Count letter vs non-letter characters
    let mut letter_count = 0;
    let mut total_count = 0;
    
    for ch in trimmed.chars() {
        // Skip spaces and common punctuation
        if ch.is_whitespace() || ".,!?-–—'\":".contains(ch) {
            continue;
        }
        
        total_count += 1;
        if ch.is_alphabetic() {
            letter_count += 1;
        }
    }
    
    // If all chars were whitespace/punctuation, reject
    if total_count == 0 {
        return true;
    }
    
    // Calculate non-letter ratio
    let non_letter_count = total_count - letter_count;
    let non_letter_ratio = non_letter_count as f32 / total_count as f32;
    
    non_letter_ratio > max_non_letter_ratio
}

/// Computes fuzzy similarity ratio between two strings (0-100 scale).
///
/// Uses the same Indel-based normalized similarity as Python's
/// `rapidfuzz.fuzz.ratio`, so the 92% dedup threshold behaves identically to
/// the original implementation. Operates on chars (Unicode scalar values), so
/// Polish diacritics (ą, ę, ó, ż, ł) are handled correctly.
fn fuzzy_ratio(s1: &str, s2: &str) -> u8 {
    use rapidfuzz::distance::indel;

    let chars1: Vec<char> = s1.to_lowercase().chars().collect();
    let chars2: Vec<char> = s2.to_lowercase().chars().collect();

    if chars1.is_empty() && chars2.is_empty() {
        return 100;
    }
    if chars1.is_empty() || chars2.is_empty() {
        return 0;
    }

    (indel::normalized_similarity(chars1.iter().copied(), chars2.iter().copied()) * 100.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_state_new() {
        let state = DedupState::new();
        assert_eq!(state.index_history.len(), 0);
        assert!(state.last_text.is_none());
        assert_eq!(state.empty_reads, 0);
    }

    #[test]
    fn test_record_match() {
        let mut state = DedupState::new();
        
        state.record_match(1, "Hello".to_string());
        assert_eq!(state.index_history.len(), 1);
        assert_eq!(state.last_text, Some("Hello".to_string()));
        assert_eq!(state.empty_reads, 0);
        
        state.record_match(2, "World".to_string());
        assert_eq!(state.index_history.len(), 2);
        assert_eq!(state.last_text, Some("World".to_string()));
    }

    #[test]
    fn test_record_match_history_limit() {
        let mut state = DedupState::new();
        
        state.record_match(1, "One".to_string());
        state.record_match(2, "Two".to_string());
        state.record_match(3, "Three".to_string());
        
        // Should keep only last 2
        assert_eq!(state.index_history.len(), 2);
        assert!(!state.index_history.contains(&1));
        assert!(state.index_history.contains(&2));
        assert!(state.index_history.contains(&3));
    }

    // Property 5: Deduplication correctness
    // Validates: Requirement 10.1, 10.2
    #[test]
    fn test_is_duplicate_index() {
        let mut state = DedupState::new();
        
        state.record_match(5, "Test".to_string());
        
        assert!(state.is_duplicate_index(5));
        assert!(!state.is_duplicate_index(6));
    }

    // Property 5: Deduplication correctness (text similarity)
    // Validates: Requirement 10.4
    #[test]
    fn test_is_duplicate_text() {
        let mut state = DedupState::new();
        
        state.record_match(1, "Hello world".to_string());
        
        // Exact match (100% similar)
        assert!(state.is_duplicate_text("Hello world", 92));
        
        // Very similar text (should be >= 92%)
        state.record_match(2, "This is a longer test sentence".to_string()); // 31 chars
        assert!(state.is_duplicate_text("This is a longer test sentence", 92)); // 100%
        
        // 1 char different in 31 chars = 96.8% similar
        assert!(state.is_duplicate_text("This is a longer test sentencx", 92));
        
        // Different text (should be < 92%)
        assert!(!state.is_duplicate_text("Completely different text", 92));
        assert!(!state.is_duplicate_text("Hello", 92)); // Much shorter
    }

    // Property 33: Per-paragraph typewriter dedup
    // Validates: Requirement 10.5
    #[test]
    fn test_typewriter_deduplication() {
        let mut state = DedupState::new();
        
        state.record_typewriter_match(0, 5);
        state.record_typewriter_match(1, 10);
        
        assert!(state.is_typewriter_duplicate(0, 5));
        assert!(!state.is_typewriter_duplicate(0, 6));
        assert!(state.is_typewriter_duplicate(1, 10));
        assert!(!state.is_typewriter_duplicate(2, 5));
    }

    #[test]
    fn test_empty_reads_counter() {
        let mut state = DedupState::new();
        
        assert_eq!(state.empty_reads(), 0);
        
        state.increment_empty_reads();
        assert_eq!(state.empty_reads(), 1);
        
        state.increment_empty_reads();
        assert_eq!(state.empty_reads(), 2);
        
        state.reset_empty_reads();
        assert_eq!(state.empty_reads(), 0);
    }

    #[test]
    fn test_clear_typewriter_cache() {
        let mut state = DedupState::new();
        
        state.record_typewriter_match(0, 5);
        state.record_typewriter_match(1, 10);
        
        state.clear_typewriter_cache();
        
        assert!(!state.is_typewriter_duplicate(0, 5));
        assert!(!state.is_typewriter_duplicate(1, 10));
    }

    #[test]
    fn test_stable_frame_detection() {
        let mut state = DedupState::new();
        
        // First frame
        assert!(!state.update_stable_detection("Hello", 3));
        
        // Same text, count = 2
        assert!(!state.update_stable_detection("Hello", 3));
        
        // Same text, count = 3 (threshold reached)
        assert!(state.update_stable_detection("Hello", 3));
        
        // Different text, count resets to 1
        assert!(!state.update_stable_detection("World", 3));
    }

    // Property 3: Empty/garbage text rejection
    // Validates: Requirement 11.5, 12.1, 12.2, 12.3
    #[test]
    fn test_is_garbage_text_length() {
        // Too short
        assert!(is_garbage_text("ab", 0.3, 3));
        
        // Exactly min length
        assert!(!is_garbage_text("abc", 0.3, 3));
        
        // Longer
        assert!(!is_garbage_text("abcd", 0.3, 3));
    }

    // Property 3: Garbage text rejection (non-letter ratio)
    // Validates: Requirement 12.1, 12.2
    #[test]
    fn test_is_garbage_text_ratio() {
        // All letters - OK
        assert!(!is_garbage_text("Hello world", 0.3, 3));
        
        // With punctuation (ignored) - OK
        assert!(!is_garbage_text("Hello, world!", 0.3, 3));
        
        // Too many non-letters (>30%)
        assert!(is_garbage_text("abc123xyz", 0.3, 3)); // 6 letters, 3 numbers = 33% non-letters
        
        // At threshold (30% non-letters) - should pass (not garbage)
        assert!(!is_garbage_text("abcdefg123", 0.3, 3)); // 7 letters, 3 numbers = 30% exactly
        
        // Below threshold - OK
        assert!(!is_garbage_text("abcdefghij", 0.3, 3)); // 10 letters, 0 numbers = 0%
    }

    // Property 29: Whitespace trimming
    // Validates: Requirement 12.4
    #[test]
    fn test_is_garbage_text_trimming() {
        // Leading/trailing whitespace should be trimmed
        assert!(is_garbage_text("  ab  ", 0.3, 3)); // After trim: "ab" (length 2)
        assert!(!is_garbage_text("  abc  ", 0.3, 3)); // After trim: "abc" (length 3)
    }

    #[test]
    fn test_is_garbage_text_only_whitespace() {
        // Only whitespace/punctuation should be rejected
        assert!(is_garbage_text("   ", 0.3, 3));
        assert!(is_garbage_text(".,!?", 0.3, 3));
        assert!(is_garbage_text("  .,!?  ", 0.3, 3));
    }

    #[test]
    fn test_fuzzy_ratio() {
        // Identical
        assert_eq!(fuzzy_ratio("hello", "hello"), 100);
        
        // Similar: Indel ratio for "hello" vs "helo" = 1 - 1/9 = ~89%
        let ratio = fuzzy_ratio("hello", "helo");
        assert!(ratio >= 85 && ratio <= 92);
        
        // Different
        let ratio = fuzzy_ratio("hello", "goodbye");
        assert!(ratio < 50);
        
        // Case insensitive
        assert_eq!(fuzzy_ratio("Hello", "HELLO"), 100);
    }

    #[test]
    fn test_dedup_state_reset() {
        let mut state = DedupState::new();
        
        state.record_match(5, "Test".to_string());
        state.record_typewriter_match(0, 10);
        state.increment_empty_reads();
        state.update_stable_detection("Hello", 3);
        
        state.reset();
        
        assert_eq!(state.index_history.len(), 0);
        assert!(state.last_text.is_none());
        assert_eq!(state.typewriter_matched.len(), 0);
        assert_eq!(state.empty_reads, 0);
        assert_eq!(state.stable_count, 0);
        assert!(state.last_fingerprint.is_none());
    }
}
