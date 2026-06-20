//! Audio playback queue with FIFO ordering and capacity management.
//!
//! This module implements the audio playback queue for dialogue lines and system sounds,
//! with support for dynamic speed adjustment and bounded queue capacity.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Commands that can be sent to the audio player.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    /// Skip to the next line with a speed boost
    SkipToNextWithBoost,
    
    /// Stop all audio playback
    StopAll,
}

/// Audio playback item types.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayItem {
    /// Dialogue line audio with file path and playback speed
    Line { path: PathBuf, speed: f32 },
    
    /// System sound audio with file path (no speed adjustment)
    System { path: PathBuf },
}

/// Audio playback queue with bounded capacity.
#[derive(Debug)]
pub struct AudioQueue {
    /// Queue items (FIFO order)
    items: VecDeque<PlayItem>,
    
    /// Maximum queue capacity
    capacity: usize,
    
    /// Currently playing flag
    is_busy: bool,
}

impl AudioQueue {
    /// Creates a new audio queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::with_capacity(capacity),
            capacity,
            is_busy: false,
        }
    }

    /// Updates the maximum queue capacity at runtime, trimming the oldest
    /// items if the new capacity is smaller.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.max(1);
        while self.items.len() > self.capacity {
            self.items.pop_front();
        }
    }

    /// Enqueues a new item, dropping oldest if queue is full.
    pub fn enqueue(&mut self, item: PlayItem) {
        if self.items.len() >= self.capacity {
            self.items.pop_front(); // Drop oldest
        }
        self.items.push_back(item);
    }

    /// Dequeues the next item (FIFO order).
    pub fn dequeue(&mut self) -> Option<PlayItem> {
        self.items.pop_front()
    }

    /// Returns the current queue length.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns true if audio is currently playing or queue is not empty.
    pub fn is_busy(&self) -> bool {
        self.is_busy || !self.items.is_empty()
    }

    /// Sets the busy state.
    pub fn set_busy(&mut self, busy: bool) {
        self.is_busy = busy;
    }

    /// Clears all items from the queue.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// Thread-safe audio queue wrapper.
#[derive(Debug, Clone)]
pub struct SharedAudioQueue {
    inner: Arc<Mutex<AudioQueue>>,
}

impl SharedAudioQueue {
    /// Creates a new shared audio queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AudioQueue::new(capacity))),
        }
    }

    /// Enqueues a new item.
    pub fn enqueue(&self, item: PlayItem) {
        let mut queue = self.inner.lock().unwrap();
        queue.enqueue(item);
    }

    /// Updates the maximum queue capacity at runtime.
    pub fn set_capacity(&self, capacity: usize) {
        self.inner.lock().unwrap().set_capacity(capacity);
    }

    /// Dequeues the next item.
    pub fn dequeue(&self) -> Option<PlayItem> {
        let mut queue = self.inner.lock().unwrap();
        queue.dequeue()
    }

    /// Returns the current queue length.
    pub fn len(&self) -> usize {
        let queue = self.inner.lock().unwrap();
        queue.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        let queue = self.inner.lock().unwrap();
        queue.is_empty()
    }

    /// Returns true if audio is busy.
    pub fn is_busy(&self) -> bool {
        let queue = self.inner.lock().unwrap();
        queue.is_busy()
    }

    /// Sets the busy state.
    pub fn set_busy(&self, busy: bool) {
        let mut queue = self.inner.lock().unwrap();
        queue.set_busy(busy);
    }

    /// Clears the queue.
    pub fn clear(&self) {
        let mut queue = self.inner.lock().unwrap();
        queue.clear();
    }
}

/// Audio file search helper - finds audio files with format fallback.
///
/// Searches for audio files in the format "output1 (N).ext" where N is 1-based line index.
pub fn find_audio_file(
    audio_dir: &std::path::Path,
    line_index: usize,
    enable_output2: bool,
    enable_dynamic_speed: bool,
) -> Option<PathBuf> {
    const SUPPORTED_FORMATS: &[&str] = &["ogg", "mp3", "m4a", "aac", "flac", "mp4"];
    
    // Try output1 first
    for ext in SUPPORTED_FORMATS {
        let path = audio_dir.join(format!("output1 ({}).{}", line_index, ext));
        if path.exists() {
            return Some(path);
        }
    }
    
    // Try output2 if enabled and dynamic speed is disabled
    if enable_output2 && !enable_dynamic_speed {
        for ext in SUPPORTED_FORMATS {
            let path = audio_dir.join(format!("output2 ({}).{}", line_index, ext));
            if path.exists() {
                return Some(path);
            }
        }
    }
    
    None
}

/// Finds a system sound file.
pub fn find_system_sound(audio_dir: &std::path::Path, sound_name: &str) -> Option<PathBuf> {
    const SUPPORTED_FORMATS: &[&str] = &["ogg", "mp3", "m4a", "aac", "flac", "mp4"];
    
    for ext in SUPPORTED_FORMATS {
        let path = audio_dir.join(format!("{}.{}", sound_name, ext));
        if path.exists() {
            return Some(path);
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_queue_new() {
        let queue = AudioQueue::new(3);
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
        assert!(!queue.is_busy());
    }

    #[test]
    fn test_audio_queue_enqueue_dequeue() {
        let mut queue = AudioQueue::new(3);
        
        let item1 = PlayItem::Line {
            path: PathBuf::from("test1.ogg"),
            speed: 1.0,
        };
        let item2 = PlayItem::System {
            path: PathBuf::from("test2.ogg"),
        };
        
        queue.enqueue(item1.clone());
        queue.enqueue(item2.clone());
        
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());
        
        // FIFO order
        assert_eq!(queue.dequeue(), Some(item1));
        assert_eq!(queue.dequeue(), Some(item2));
        assert_eq!(queue.dequeue(), None);
    }

    // Property 6: Queue capacity constraint
    // Validates: Requirement 13.1, 13.2
    #[test]
    fn test_queue_capacity_constraint() {
        let mut queue = AudioQueue::new(2);
        
        let item1 = PlayItem::Line {
            path: PathBuf::from("1.ogg"),
            speed: 1.0,
        };
        let item2 = PlayItem::Line {
            path: PathBuf::from("2.ogg"),
            speed: 1.0,
        };
        let item3 = PlayItem::Line {
            path: PathBuf::from("3.ogg"),
            speed: 1.0,
        };
        
        queue.enqueue(item1.clone());
        queue.enqueue(item2.clone());
        assert_eq!(queue.len(), 2);
        
        // Adding 3rd item should drop oldest (item1)
        queue.enqueue(item3.clone());
        assert_eq!(queue.len(), 2);
        
        // Should get item2 and item3, not item1
        assert_eq!(queue.dequeue(), Some(item2));
        assert_eq!(queue.dequeue(), Some(item3));
        assert_eq!(queue.dequeue(), None);
    }

    // Property 24: FIFO ordering
    // Validates: Requirement 13.3
    #[test]
    fn test_fifo_ordering() {
        let mut queue = AudioQueue::new(10);
        
        for i in 1..=5 {
            queue.enqueue(PlayItem::Line {
                path: PathBuf::from(format!("{}.ogg", i)),
                speed: 1.0,
            });
        }
        
        // Should dequeue in same order as enqueued
        for i in 1..=5 {
            let item = queue.dequeue().unwrap();
            match item {
                PlayItem::Line { path, .. } => {
                    assert_eq!(path, PathBuf::from(format!("{}.ogg", i)));
                }
                _ => panic!("Expected Line item"),
            }
        }
    }

    #[test]
    fn test_is_busy_flag() {
        let mut queue = AudioQueue::new(3);
        
        assert!(!queue.is_busy());
        
        queue.set_busy(true);
        assert!(queue.is_busy());
        
        queue.set_busy(false);
        assert!(!queue.is_busy());
        
        // is_busy() should return true if queue has items
        queue.enqueue(PlayItem::System {
            path: PathBuf::from("test.ogg"),
        });
        assert!(queue.is_busy()); // Even though is_busy flag is false, queue not empty
    }

    #[test]
    fn test_clear_queue() {
        let mut queue = AudioQueue::new(3);
        
        queue.enqueue(PlayItem::Line {
            path: PathBuf::from("1.ogg"),
            speed: 1.0,
        });
        queue.enqueue(PlayItem::Line {
            path: PathBuf::from("2.ogg"),
            speed: 1.0,
        });
        
        assert_eq!(queue.len(), 2);
        
        queue.clear();
        
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_shared_audio_queue() {
        let queue = SharedAudioQueue::new(3);
        
        let item = PlayItem::Line {
            path: PathBuf::from("test.ogg"),
            speed: 1.0,
        };
        
        queue.enqueue(item.clone());
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        
        let dequeued = queue.dequeue();
        assert_eq!(dequeued, Some(item));
        
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_shared_queue_thread_safety() {
        use std::thread;
        
        let queue = SharedAudioQueue::new(100);
        let queue_clone = queue.clone();
        
        // Spawn producer thread
        let producer = thread::spawn(move || {
            for i in 0..50 {
                queue_clone.enqueue(PlayItem::Line {
                    path: PathBuf::from(format!("{}.ogg", i)),
                    speed: 1.0,
                });
            }
        });
        
        producer.join().unwrap();
        
        // Should have all 50 items
        assert_eq!(queue.len(), 50);
    }

    #[test]
    fn test_play_item_types() {
        let line_item = PlayItem::Line {
            path: PathBuf::from("dialogue.ogg"),
            speed: 1.5,
        };
        
        let system_item = PlayItem::System {
            path: PathBuf::from("on.ogg"),
        };
        
        match line_item {
            PlayItem::Line { path, speed } => {
                assert_eq!(path, PathBuf::from("dialogue.ogg"));
                assert_eq!(speed, 1.5);
            }
            _ => panic!("Expected Line"),
        }
        
        match system_item {
            PlayItem::System { path } => {
                assert_eq!(path, PathBuf::from("on.ogg"));
            }
            _ => panic!("Expected System"),
        }
    }

    #[test]
    fn test_find_audio_file() {
        // This test would need actual files, so we just test the logic
        // In real usage, this would find files with format fallback
        
        use std::fs;
        use tempfile::TempDir;
        
        let temp_dir = TempDir::new().unwrap();
        let audio_dir = temp_dir.path();
        
        // Create a test file
        let test_file = audio_dir.join("output1 (1).ogg");
        fs::write(&test_file, b"test").unwrap();
        
        // Should find the file
        let result = find_audio_file(audio_dir, 1, false, false);
        assert_eq!(result, Some(test_file));
        
        // Should not find non-existent file
        let result = find_audio_file(audio_dir, 999, false, false);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_system_sound() {
        use std::fs;
        use tempfile::TempDir;
        
        let temp_dir = TempDir::new().unwrap();
        let audio_dir = temp_dir.path();
        
        // Create a test system sound
        let test_file = audio_dir.join("on.mp3");
        fs::write(&test_file, b"test").unwrap();
        
        // Should find the file
        let result = find_system_sound(audio_dir, "on");
        assert_eq!(result, Some(test_file));
        
        // Should not find non-existent sound
        let result = find_system_sound(audio_dir, "nonexistent");
        assert_eq!(result, None);
    }
}

/// Audio player with playback engine.
pub struct AudioPlayer {
    /// Shared audio queue
    queue: SharedAudioQueue,
    
    /// Rodio output stream handle
    _stream: rodio::OutputStream,
    
    /// Rodio stream handle for playback
    stream_handle: rodio::OutputStreamHandle,
    
    /// Current sink (for controlling playback)
    current_sink: Arc<Mutex<Option<rodio::Sink>>>,
    
    /// Enable dynamic speed adjustment
    enable_dynamic_speed: bool,
    
    /// Base playback speed (when idle)
    base_playback_speed: f32,
    
    /// Overlap playback speed (when busy)
    overlap_playback_speed: f32,
    
    /// Volume controller for ducking (optional)
    volume_controller: Option<crate::ducking::VolumeController>,
    
    /// Volume reduction level for ducking (0.0 to 1.0)
    volume_reduction_level: f32,

    /// Reader (TTS) playback volume (0.0 to 1.0), applied to each played sink.
    reader_volume: f32,
}

/// Appends an audio file to the sink at the given playback `speed`, preserving
/// pitch (no "chipmunk" effect).
///
/// Always decodes the full file into memory first, then time-stretches if needed
/// and appends the pre-decoded buffer. This eliminates buffer underruns that
/// occur with rodio's lazy decoder when the CPU is under load (e.g. OCR running).
fn append_audio(sink: &rodio::Sink, path: &std::path::Path, speed: f32) -> Result<(), String> {
    use rodio::Source;

    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open audio file {:?}: {}", path, e))?;
    let decoder = rodio::Decoder::new(std::io::BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio file {:?}: {}", path, e))?;

    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples: Vec<f32> = decoder.convert_samples().collect();

    let audio = if (speed - 1.0).abs() < 0.01 {
        samples
    } else {
        crate::time_stretch::time_stretch(&samples, channels, sample_rate, speed)
    };

    let buffer = rodio::buffer::SamplesBuffer::new(channels, sample_rate, audio);
    sink.append(buffer);
    Ok(())
}

impl AudioPlayer {
    /// Creates a new audio player with the given queue capacity.
    pub fn new(
        queue_capacity: usize,
        enable_dynamic_speed: bool,
        base_playback_speed: f32,
        overlap_playback_speed: f32,
        volume_reduction_level: f32,
        ducking_target_process: Option<String>,
    ) -> Result<Self, String> {
        let (_stream, stream_handle) = rodio::OutputStream::try_default()
            .map_err(|e| format!("Failed to initialize audio output: {}", e))?;
        
        // Create volume controller with target process
        let volume_controller = if let Some(target) = ducking_target_process {
            if !target.trim().is_empty() {
                Some(crate::ducking::VolumeController::new_with_target(target))
            } else {
                Some(crate::ducking::VolumeController::new())
            }
        } else {
            Some(crate::ducking::VolumeController::new())
        };
        
        Ok(Self {
            queue: SharedAudioQueue::new(queue_capacity),
            _stream,
            stream_handle,
            current_sink: Arc::new(Mutex::new(None)),
            enable_dynamic_speed,
            base_playback_speed,
            overlap_playback_speed,
            volume_controller,
            volume_reduction_level,
            reader_volume: 1.0,
        })
    }

    /// Enqueues an audio item for playback.
    pub fn enqueue(&self, item: PlayItem) {
        self.queue.enqueue(item);
    }

    /// Update the playback speeds (and dynamic-speed flag) at runtime, so the
    /// speed sliders take effect on the next line without restarting the reader.
    pub fn set_speeds(&mut self, base: f32, overlap: f32, dynamic: bool) {
        self.base_playback_speed = base;
        self.overlap_playback_speed = overlap;
        self.enable_dynamic_speed = dynamic;
    }

    /// Update the queue capacity at runtime, so the "audio queue size" setting
    /// takes effect immediately without restarting the reader.
    pub fn set_queue_capacity(&self, capacity: usize) {
        self.queue.set_capacity(capacity);
    }

    /// Update the game-ducking reduction level (0.0-1.0) at runtime.
    pub fn set_volume_reduction(&mut self, level: f32) {
        self.volume_reduction_level = level.clamp(0.0, 1.0);
    }

    /// Update the reader (TTS) playback volume (0.0-1.0) at runtime. Applies to
    /// the currently playing sink and all subsequent ones.
    pub fn set_reader_volume(&mut self, level: f32) {
        self.reader_volume = level.clamp(0.0, 1.0);
        if let Ok(guard) = self.current_sink.lock() {
            if let Some(sink) = guard.as_ref() {
                sink.set_volume(self.reader_volume);
            }
        }
    }

    /// Enqueues a dialogue line, automatically deciding the playback speed.
    ///
    /// The speed is LOCKED IN at enqueue time:
    /// - If audio is currently busy (playing or queued), the line will play at
    ///   `overlap_playback_speed` (faster, because it overlapped with another line).
    /// - If nothing is playing, the line plays at `base_playback_speed` (normal).
    ///
    /// This ensures a line queued during playback plays sped-up even after the
    /// queue has been drained by the time it actually starts.
    pub fn enqueue_line_auto_speed(&self, path: PathBuf) {
        let speed = if self.enable_dynamic_speed {
            if self.is_busy() {
                self.overlap_playback_speed
            } else {
                self.base_playback_speed
            }
        } else {
            1.0
        };
        tracing::debug!("Enqueue line at locked speed {} (busy={})", speed, self.is_busy());
        self.queue.enqueue(PlayItem::Line { path, speed });
    }

    /// Returns the current queue length.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Returns true if audio is currently playing or queue is not empty.
    pub fn is_busy(&self) -> bool {
        self.queue.is_busy()
    }

    /// Determines the playback speed based on current state.
    ///
    /// Property 25: Dynamic speed selection
    #[allow(dead_code)]
    fn get_playback_speed(&self) -> f32 {
        if !self.enable_dynamic_speed {
            return 1.0;
        }
        
        // Check if currently playing or queue has items
        let is_busy = {
            let sink_opt = self.current_sink.lock().unwrap();
            sink_opt.as_ref().map(|s| !s.empty()).unwrap_or(false)
        };
        
        if is_busy || !self.queue.is_empty() {
            self.overlap_playback_speed
        } else {
            self.base_playback_speed
        }
    }

    /// Plays the next item from the queue.
    pub fn play_next(&mut self) -> Result<bool, String> {
        let item = match self.queue.dequeue() {
            Some(item) => item,
            None => return Ok(false), // No items to play
        };

        self.play_item_internal(item)?;
        Ok(true)
    }

    /// Internal method to play a specific item immediately (respects dynamic speed).
    fn play_item_internal(&mut self, item: PlayItem) -> Result<(), String> {
        self.queue.set_busy(true);

        // Duck volume for dialog audio (not for system sounds)
        if matches!(item, PlayItem::Line { .. }) {
            if let Some(ref mut controller) = self.volume_controller {
                if let Err(e) = controller.duck(self.volume_reduction_level) {
                    tracing::warn!("Failed to duck audio: {}", e);
                }
            }
        }

        // Create new sink
        let sink = rodio::Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Failed to create sink: {}", e))?;
        sink.set_volume(self.reader_volume);

        // Use the speed locked into the item (decided at enqueue time via
        // enqueue_line_auto_speed). System sounds always play at 1.0.
        let speed = match &item {
            PlayItem::Line { speed, .. } => *speed,
            PlayItem::System { .. } => 1.0,
        };

        // Load and decode audio file
        let path = match &item {
            PlayItem::Line { path, .. } => path,
            PlayItem::System { path } => path,
        };

        append_audio(&sink, path, speed)?;
        sink.play();

        // Log which file is being played
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        tracing::info!("▶ Playing audio: '{}' at speed {:.2}x", file_name, speed);
        crate::logging::user_log(format!("🔊 Odtwarzam audio: {} (prędkość {:.2}x)", file_name, speed));

        // Store sink
        {
            let mut current_sink = self.current_sink.lock().unwrap();
            *current_sink = Some(sink);
        }

        Ok(())
    }

    /// Interrupts the currently playing audio and plays the next item.
    ///
    /// Implements fadeout and immediate transition to next item.
    pub fn interrupt(&mut self) -> Result<(), String> {
        // Stop current playback
        {
            let mut sink_opt = self.current_sink.lock().unwrap();
            if let Some(sink) = sink_opt.take() {
                sink.stop();
            }
        }

        // Restore volume on interrupt
        if let Some(ref mut controller) = self.volume_controller {
            if controller.is_ducked() {
                if let Err(e) = controller.restore() {
                    tracing::warn!("Failed to restore audio on interrupt: {}", e);
                }
            }
        }

        self.queue.set_busy(false);

        // Play next item if available
        self.play_next()?;

        Ok(())
    }

    /// Skips to the next line with increased playback speed (+10%).
    ///
    /// This is typically used when the user manually skips to the next dialogue line
    /// via hotkey, and we want to play it faster to catch up with the game state.
    ///
    /// # Returns
    /// - `Ok(())` if skip was successful or queue is empty
    /// - `Err(String)` if playback fails
    pub fn skip_to_next_with_speed_boost(&mut self) -> Result<(), String> {
        // Stop current playback
        {
            let mut sink_opt = self.current_sink.lock().unwrap();
            if let Some(sink) = sink_opt.take() {
                sink.stop();
            }
        }

        // Restore volume on skip
        if let Some(ref mut controller) = self.volume_controller {
            if controller.is_ducked() {
                if let Err(e) = controller.restore() {
                    tracing::warn!("Failed to restore audio on skip: {}", e);
                }
            }
        }

        self.queue.set_busy(false);

        // Get next item from queue
        let item = match self.queue.dequeue() {
            Some(item) => item,
            None => {
                // No items to play
                return Ok(());
            }
        };

        // Apply +10% speed boost for Line items
        let boosted_item = match item {
            PlayItem::Line { path, speed } => {
                PlayItem::Line {
                    path,
                    speed: speed * 1.1, // +10% speed boost
                }
            }
            system_item => system_item, // Keep system sounds unchanged
        };

        // Play the boosted item immediately with FORCED speed (disable dynamic speed for this play)
        self.play_item_with_fixed_speed(boosted_item)?;

        Ok(())
    }

    /// Internal method to play a specific item immediately with fixed speed (ignores dynamic speed).
    fn play_item_with_fixed_speed(&mut self, item: PlayItem) -> Result<(), String> {
        self.queue.set_busy(true);

        // Duck volume for dialog audio (not for system sounds)
        if matches!(item, PlayItem::Line { .. }) {
            if let Some(ref mut controller) = self.volume_controller {
                if let Err(e) = controller.duck(self.volume_reduction_level) {
                    tracing::warn!("Failed to duck audio: {}", e);
                }
            }
        }

        // Create new sink
        let sink = rodio::Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Failed to create sink: {}", e))?;
        sink.set_volume(self.reader_volume);

        // Get playback speed - ALWAYS use the speed from item (no dynamic speed override)
        let speed = match &item {
            PlayItem::Line { speed, .. } => *speed,
            PlayItem::System { .. } => 1.0,
        };

        // Load and decode audio file
        let path = match &item {
            PlayItem::Line { path, .. } => path,
            PlayItem::System { path } => path,
        };

        append_audio(&sink, path, speed)?;
        sink.play();

        // Log which file is being played (skip / boosted)
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        tracing::info!("▶ Playing audio (skip +10%): '{}' at speed {:.2}x", file_name, speed);
        crate::logging::user_log(format!("⏭️ Pomijam do następnego: {} (prędkość {:.2}x)", file_name, speed));

        // Store sink
        {
            let mut current_sink = self.current_sink.lock().unwrap();
            *current_sink = Some(sink);
        }

        Ok(())
    }

    /// Checks if current playback is finished and starts next item.
    pub fn update(&mut self) -> Result<(), String> {
        // Check if current sink is done
        let is_done = {
            let sink_opt = self.current_sink.lock().unwrap();
            sink_opt.as_ref().map(|s| s.empty()).unwrap_or(true)
        };

        if is_done {
            // Restore volume when audio finishes
            // Requirement 15.4: Restore original game volume when dialog finishes
            if let Some(ref mut controller) = self.volume_controller {
                if controller.is_ducked() {
                    if let Err(e) = controller.restore() {
                        tracing::warn!("Failed to restore audio: {}", e);
                    }
                }
            }
            
            self.queue.set_busy(false);
            
            // Clear current sink
            {
                let mut current_sink = self.current_sink.lock().unwrap();
                *current_sink = None;
            }

            // Try to play next item
            self.play_next()?;
        }

        Ok(())
    }

    /// Clears the queue and stops playback.
    pub fn stop(&mut self) {
        self.queue.clear();
        
        let mut sink_opt = self.current_sink.lock().unwrap();
        if let Some(sink) = sink_opt.take() {
            sink.stop();
        }
        
        // Restore volume on stop
        if let Some(ref mut controller) = self.volume_controller {
            if controller.is_ducked() {
                if let Err(e) = controller.restore() {
                    tracing::warn!("Failed to restore audio on stop: {}", e);
                }
            }
        }
        
        self.queue.set_busy(false);
    }
}

#[cfg(test)]
mod player_tests {
    use super::*;

    // Note: These tests are limited because rodio requires actual audio hardware.
    // In a real environment, we'd use integration tests with test audio files.

    #[test]
    fn test_audio_player_creation() {
        // This may fail in CI/headless environments without audio devices
        let result = AudioPlayer::new(3, false, 1.0, 1.5, 0.2, None);
        
        // We can't assert success because it depends on audio hardware availability
        // Just check that the function doesn't panic
        match result {
            Ok(player) => {
                assert_eq!(player.queue_len(), 0);
                assert!(!player.is_busy());
            }
            Err(_) => {
                // Audio device not available, skip test
                eprintln!("Audio device not available, skipping test");
            }
        }
    }

    // Property 25: Dynamic speed selection
    // Validates: Requirement 14.2, 14.3
    #[test]
    fn test_dynamic_speed_selection() {
        let result = AudioPlayer::new(3, true, 1.0, 1.5, 0.2, None);
        
        match result {
            Ok(player) => {
                // When queue is empty and not playing, should use base speed
                let speed = player.get_playback_speed();
                assert_eq!(speed, 1.0);

                // When queue has items, should use overlap speed
                player.enqueue(PlayItem::Line {
                    path: PathBuf::from("test.ogg"),
                    speed: 1.0,
                });
                
                let speed = player.get_playback_speed();
                assert_eq!(speed, 1.5);
            }
            Err(_) => {
                eprintln!("Audio device not available, skipping test");
            }
        }
    }

    #[test]
    fn test_dynamic_speed_disabled() {
        let result = AudioPlayer::new(3, false, 1.0, 1.5, 0.2, None);
        
        match result {
            Ok(player) => {
                // When dynamic speed is disabled, always return 1.0
                let speed = player.get_playback_speed();
                assert_eq!(speed, 1.0);

                player.enqueue(PlayItem::Line {
                    path: PathBuf::from("test.ogg"),
                    speed: 1.0,
                });
                
                let speed = player.get_playback_speed();
                assert_eq!(speed, 1.0);
            }
            Err(_) => {
                eprintln!("Audio device not available, skipping test");
            }
        }
    }

    #[test]
    fn test_player_enqueue() {
        let result = AudioPlayer::new(3, false, 1.0, 1.5, 0.2, None);
        
        match result {
            Ok(player) => {
                player.enqueue(PlayItem::System {
                    path: PathBuf::from("test.ogg"),
                });
                
                assert_eq!(player.queue_len(), 1);
            }
            Err(_) => {
                eprintln!("Audio device not available, skipping test");
            }
        }
    }

    #[test]
    fn test_player_stop() {
        let result = AudioPlayer::new(3, false, 1.0, 1.5, 0.2, None);
        
        match result {
            Ok(mut player) => {
                player.enqueue(PlayItem::Line {
                    path: PathBuf::from("test1.ogg"),
                    speed: 1.0,
                });
                player.enqueue(PlayItem::Line {
                    path: PathBuf::from("test2.ogg"),
                    speed: 1.0,
                });
                
                assert_eq!(player.queue_len(), 2);
                
                player.stop();
                
                assert_eq!(player.queue_len(), 0);
                assert!(!player.is_busy());
            }
            Err(_) => {
                eprintln!("Audio device not available, skipping test");
            }
        }
    }
}
