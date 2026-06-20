// ============================================================
// APPLICATION METADATA
// ============================================================
pub const APP_NAME: &str = "GameReader";
pub const APP_VERSION: &str = "1.0.0";
pub const APP_VERSION_TAG: &str = "beta";

// ============================================================
// DEBUG
// ============================================================
pub const DEBUG_MAX_ENTRIES: usize = 1000;

// ============================================================
// SUPPORTED GAME RESOLUTIONS
// ============================================================
pub const SUPPORTED_RESOLUTIONS: &[(&str, (u32, u32))] = &[
    ("1280x720", (1280, 720)),
    ("1280x800", (1280, 800)),
    ("1366x768", (1366, 768)),
    ("1600x900", (1600, 900)),
    ("1920x1080", (1920, 1080)),
    ("1920x1200", (1920, 1200)),
    ("2560x1080", (2560, 1080)),
    ("2560x1440", (2560, 1440)),
    ("3440x1440", (3440, 1440)),
    ("3840x2160", (3840, 2160)),
    ("4096x2160", (4096, 2160)),
    ("5120x2160", (5120, 2160)),
];

// ============================================================
// TTS SPEED LIMITS
// ============================================================
pub const BASE_PLAYBACK_SPEED_MIN: f32 = 0.5;
pub const BASE_PLAYBACK_SPEED_MAX: f32 = 3.0;

pub const OVERLAP_PLAYBACK_SPEED_MIN: f32 = 0.5;
pub const OVERLAP_PLAYBACK_SPEED_MAX: f32 = 3.3;

pub const SPEED_STEP: f32 = 0.01;
pub const SPEED_DECIMALS: u8 = 2;

// ============================================================
// OCR / RESOLUTION SCALING
// ============================================================

// === DOWNSCALE OCR ===
pub const RESOLUTION_DOWNSCALE_MIN: f32 = 0.1;
pub const RESOLUTION_DOWNSCALE_MAX: f32 = 1.0;
pub const RESOLUTION_DOWNSCALE_DECIMALS: u8 = 2;

// === CAPTURE INTERVAL ===
pub const CAPTURE_INTERVAL_MIN: f32 = 0.1;
pub const CAPTURE_INTERVAL_MAX: f32 = 5.0;

// === OCR TEXT HEIGHT ===
pub const MIN_HEIGHT_MIN: i32 = 1;
pub const MIN_HEIGHT_MAX: i32 = 9999;

pub const MAX_HEIGHT_MIN: i32 = 1;
pub const MAX_HEIGHT_MAX: i32 = 9999;

// ============================================================
// CENTER LINES (HELPER LINES)
// ============================================================
pub const CENTER_LINE_MARGIN_MIN: i32 = 1;
pub const CENTER_LINE_MARGIN_MAX: i32 = 9999;

pub const CENTER_LINE_2_START_MIN: i32 = 1;
pub const CENTER_LINE_2_START_MAX: i32 = 9999;

pub const CENTER_LINE_3_START_RATIO_MIN: f32 = 0.1;
pub const CENTER_LINE_3_START_RATIO_MAX: f32 = 1.0;

// ============================================================
// AUDIO / MIXING
// ============================================================
pub const VOLUME_REDUCTION_LEVEL_MIN: f32 = 0.0;
pub const VOLUME_REDUCTION_LEVEL_MAX: f32 = 1.0;

pub const AUDIO_QUEUE_SIZE_ALLOWED: &[u8] = &[1, 2, 3];

// ============================================================
// DEFAULT AUDIO SETTINGS
// ============================================================
pub const VOLUME_FADE_DURATION: f32 = 0.2;
pub const VOLUME_REDUCTION_LEVEL: f32 = 0.2;
pub const ENABLE_OUTPUT2_SYSTEM: bool = true;
pub const ENABLE_DYNAMIC_SPEED: bool = false;
pub const BASE_PLAYBACK_SPEED: f32 = 1.0;
pub const OVERLAP_PLAYBACK_SPEED: f32 = 1.2;
pub const AUDIO_QUEUE_SIZE: u8 = 1;

pub const SUPPORTED_AUDIO_FORMATS: &[&str] = &[".ogg", ".mp3", ".m4a", ".aac", ".flac", ".mp4"];

// ============================================================
// OCR / TEXT PROCESSING
// ============================================================
// Capture mode: Only "window" = WGC window capture (works in fullscreen).
// GDI region mode removed.
pub const CAPTURE_MODE: &str = "window";
// Window query: REQUIRED process exe name (e.g. "GTA-SA.exe") or title substring.
pub const CAPTURE_WINDOW_QUERY: &str = "";

pub const RESOLUTION_DOWNSCALE: f32 = 1.0; // Fixed at 1.0 for best OCR accuracy (no downscaling)
pub const MIN_HEIGHT: i32 = 10;
pub const MAX_HEIGHT: i32 = 100;
pub const CAPTURE_INTERVAL: f32 = 0.25; // Faster OCR checks (was 0.5s) = less lag
pub const FRAME_DIFFERENCE_THRESHOLD: f32 = 0.6; // Skip OCR when frame unchanged (saves CPU)

pub const SIMILARITY_THRESHOLD: u8 = 75;
pub const SIMILARITY_THRESHOLD2: u8 = 90;
pub const SHORT_LINE_MAX_LENGTH: usize = 8;
pub const LINE_THRESHOLD: i32 = 10;
pub const OCR_MIN_CONFIDENCE: f32 = 0.4;
pub const TYPEWRITER_MIN_COVERAGE: f32 = 0.65;
pub const TYPEWRITER_STABLE_READS: u8 = 2;

// Matcher windowing and margins
pub const SCORE_MARGIN: u8 = 3;
pub const FORWARD_WINDOW: usize = 400;
pub const BACK_WINDOW: usize = 30;
pub const GLOBAL_OVERRIDE_MARGIN: u8 = 12;
pub const TYPEWRITER_DISAMBIG_MARGIN: u8 = 20;

pub const ENABLE_REMOVE_CHARACTER_NAME: bool = false;
pub const ENABLE_SCREENSHOTS: bool = false;
pub const ENABLE_PARAGRAPH_OCR: bool = false;
pub const ENABLE_TYPEWRITER_WAIT: bool = false;
// Show a visual on-screen frame marking the OCR capture region
pub const ENABLE_REGION_OVERLAY: bool = false;

// === OCR PERFORMANCE ===
// MNN inference thread count (more = faster OCR, but higher CPU during inference).
pub const OCR_THREAD_COUNT: i32 = 4;
// Detection model max input side length. Lower = faster detection (recognition
// quality is unaffected since text crops come from the full-res image).
pub const OCR_DET_MAX_SIDE_LEN: u32 = 736;
// Otsu binarization: DISABLED. A single global threshold merges bright
// subtitle text into bright backgrounds (e.g. sky/walls), making OCR read
// intermittently. PP-OCRv5 is trained on natural color images, so we feed it
// the original color frame instead.
pub const OCR_BINARIZE: bool = false;

// === OUTLINE TEXT MODE (white subtitles with a dark outline on bright bg) ===
// When enabled, for_ocr() keeps only near-white pixels that touch a dark
// outline, producing clean black-on-white text. This isolates outlined
// subtitles from bright backgrounds (which lack the dark stroke).
pub const ENABLE_OUTLINE_TEXT_MODE: bool = false;
pub const OUTLINE_WHITE_THRESHOLD: u8 = 190; // pixel >= this counts as "white text"
pub const OUTLINE_DARK_THRESHOLD: u8 = 70; // pixel <= this counts as "dark outline"

// === OCR IDLE MODE ===
pub const EMPTY_READS_TO_IDLE: u8 = 3;
pub const IDLE_SLEEP_SECONDS: f32 = 0.15; // Short idle sleep so new dialogue is picked up fast

// ============================================================
// CENTER LINES (DEFAULTS)
// ============================================================
pub const USE_CENTER_LINE_1: bool = false;
pub const USE_CENTER_LINE_2: bool = false;
pub const USE_CENTER_LINE_3: bool = false;

pub const TAB_ANIMATIONS: bool = true;

pub const CENTER_LINE_MARGIN: i32 = 100;
pub const CENTER_LINE_2_START: i32 = 1;
pub const CENTER_LINE_3_START_RATIO: f32 = 0.3;

// ============================================================
// DIALOG REPLAY LOGIC
// ============================================================
pub const REPLAY_DELAY_SECONDS: f32 = 30.0;

// ============================================================
// DEFAULT KEY BINDINGS
// ============================================================
pub fn default_key_bindings() -> std::collections::HashMap<String, String> {
    [
        ("toggle_reader", "home"),
        ("volume_up", "page_up"),
        ("volume_down", "page_down"),
        ("test_sound", "insert"),
        ("open_settings", "alt+'"),
        ("interrupt_audio", "delete"),
        ("base_speed_up", "shift+z"),
        ("base_speed_down", "shift+x"),
        ("overlap_speed_up", "shift+c"),
        ("overlap_speed_down", "shift+v"),
        ("debug_console", "alt+d"),
        ("toggle_areas", "alt+2"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

// ============================================================
// HOTKEYS – ACCESS POLICY
// ============================================================
pub const ALLOWED_HOTKEYS_WHEN_READER_OFF: &[&str] = &[
    "toggle_reader",
    "toggle_areas",
    "open_settings",
    "debug_console",
];

// ============================================================
// HOTKEYS – DEBOUNCE
// ============================================================
pub const ACTION_DEBOUNCE: f32 = 0.25;

// ============================================================
// HOTKEYS – VALIDATION WHITELISTS
// ============================================================
pub const RESERVED_SYSTEM_HOTKEYS: &[&str] = &[
    "alt+tab",
    "alt+f4",
    "ctrl+alt+del",
    "ctrl+shift+esc",
];

pub const ALLOWED_MODIFIERS: &[&str] = &[
    "ctrl",
    "alt",
    "shift",
];

// Whitelist of allowed single keys (alphanumeric, function keys, special keys)
pub const ALLOWED_KEYS: &[&str] = &[
    // Alphanumeric
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
    "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    // Function keys
    "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
    // Special keys
    "home", "end", "insert", "delete", "page_up", "page_down",
    "tab", "backspace", "space",
    // Symbols
    "`", "_", "'",
];

// ============================================================
// RECENT PRESETS
// ============================================================
pub const MAX_RECENT_PRESETS: usize = 10;
