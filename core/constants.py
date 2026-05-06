
# ============================================================
# METADANE APLIKACJI
# ============================================================
APP_NAME = "GameReader"
APP_VERSION = "0.9.5"
APP_VERSION_TAG = "beta"


# ============================================================
# DEBUG
# ============================================================
DEBUG_MAX_ENTRIES = 1000


# ============================================================
# OBSŁUGIWANE ROZDZIELCZOŚCI GIER
# ============================================================
SUPPORTED_RESOLUTIONS = {
    "1280x720": (1280, 720),
    "1280x800": (1280, 800),
    "1366x768": (1366, 768),
    "1600x900": (1600, 900),
    "1920x1080": (1920, 1080),
    "1920x1200": (1920, 1200),
    "2560x1080": (2560, 1080),
    "2560x1440": (2560, 1440),
    "3440x1440": (3440, 1440),
    "3840x2160": (3840, 2160),
    "4096x2160": (4096, 2160),
    "5120x2160": (5120, 2160),
}


# ============================================================
# OGRANICZENIA PRĘDKOŚCI TTS
# ============================================================
BASE_PLAYBACK_SPEED_MIN = 0.8
BASE_PLAYBACK_SPEED_MAX = 1.2

OVERLAP_PLAYBACK_SPEED_MIN = 1.0
OVERLAP_PLAYBACK_SPEED_MAX = 3.0

SPEED_STEP = 0.01
SPEED_DECIMALS = 2


# ============================================================
# OCR / SKALOWANIE ROZDZIELCZOŚCI
# ============================================================

# === DOWNSCALE OCR ===
RESOLUTION_DOWNSCALE_MIN = 0.1
RESOLUTION_DOWNSCALE_MAX = 1.0
RESOLUTION_DOWNSCALE_DECIMALS = 2

# === INTERWAŁ PRZECHWYTYWANIA ===
CAPTURE_INTERVAL_MIN = 0.1
CAPTURE_INTERVAL_MAX = 5.0

# === WYSOKOŚĆ TEKSTU OCR ===
MIN_HEIGHT_MIN = 1
MIN_HEIGHT_MAX = 9999

MAX_HEIGHT_MIN = 1
MAX_HEIGHT_MAX = 9999


# ============================================================
# LINIE POMOCNICZE (CENTER LINES)
# ============================================================
CENTER_LINE_MARGIN_MIN = 1
CENTER_LINE_MARGIN_MAX = 9999

CENTER_LINE_2_START_MIN = 1
CENTER_LINE_2_START_MAX = 9999

CENTER_LINE_3_START_RATIO_MIN = 0.1
CENTER_LINE_3_START_RATIO_MAX = 1.0


# ============================================================
# AUDIO / MIKSOWANIE
# ============================================================
VOLUME_REDUCTION_LEVEL_MIN = 0.0
VOLUME_REDUCTION_LEVEL_MAX = 1.0

AUDIO_QUEUE_SIZE_ALLOWED = (1, 2, 3)


# ============================================================
# DOMYŚLNE USTAWIENIA AUDIO
# ============================================================
VOLUME_FADE_DURATION = 0.2
VOLUME_REDUCTION_LEVEL = 0.2
ENABLE_OUTPUT2_SYSTEM = True
ENABLE_DYNAMIC_SPEED = False
BASE_PLAYBACK_SPEED = 1.0
OVERLAP_PLAYBACK_SPEED = 1.2
AUDIO_QUEUE_SIZE = 1

SUPPORTED_AUDIO_FORMATS = ['.ogg', '.mp3', '.m4a', '.aac', '.flac', '.mp4']


# ============================================================
# OCR / PRZETWARZANIE TEKSTU
# ============================================================
RESOLUTION_DOWNSCALE = 0.45
MIN_HEIGHT = 10
MAX_HEIGHT = 100
CAPTURE_INTERVAL = 0.5
FRAME_DIFFERENCE_THRESHOLD = 1.0

SIMILARITY_THRESHOLD = 60
SIMILARITY_THRESHOLD2 = 80
SHORT_LINE_MAX_LENGTH = 8
LINE_THRESHOLD = 10
OCR_MIN_CONFIDENCE = 0.4
TYPEWRITER_MIN_COVERAGE = 0.65
TYPEWRITER_STABLE_READS = 2

ENABLE_REMOVE_CHARACTER_NAME = False
ENABLE_SCREENSHOTS = False
ENABLE_PARAGRAPH_OCR = False
ENABLE_TYPEWRITER_WAIT = False

# === OCR IDLE MODE ===
EMPTY_READS_TO_IDLE = 3 
IDLE_SLEEP_SECONDS = 0.8


# ============================================================
# LINIE CENTRALNE (DOMYŚLNE)
# ============================================================
USE_CENTER_LINE_1 = False
USE_CENTER_LINE_2 = False
USE_CENTER_LINE_3 = False

TAB_ANIMATIONS = True

CENTER_LINE_MARGIN = 100
CENTER_LINE_2_START = 1
CENTER_LINE_3_START_RATIO = 0.3


# ============================================================
# LOGIKA POWTARZANIA DIALOGÓW
# ============================================================
REPLAY_DELAY_SECONDS = 30


# ============================================================
# DOMYŚLNE SKRÓTY KLAWISZOWE
# ============================================================
DEFAULT_KEY_BINDINGS = {
    'toggle_reader': 'home',
    'volume_up': 'page_up',
    'volume_down': 'page_down',
    'switch_monitor_toggle': 'alt+1',
    'test_sound': 'insert',
    'open_settings': 'alt+`',
    'interrupt_audio': 'delete',
    'base_speed_up': 'shift+z',
    'base_speed_down': 'shift+x',
    'overlap_speed_up': 'shift+c',
    'overlap_speed_down': 'shift+v',
    'debug_console': 'alt+d',
    'toggle_areas': 'alt+2'
}


# ============================================================
# HOTKEYS – POLITYKA DOSTĘPU
# ============================================================
ALLOWED_HOTKEYS_WHEN_READER_OFF = {
    "toggle_reader",
    "toggle_areas",
    "open_settings",
    "debug_console",
}