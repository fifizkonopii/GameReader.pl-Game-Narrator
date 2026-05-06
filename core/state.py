
# ============================================================
# IMPORTY
# ============================================================
import queue
import threading
from PySide6.QtGui import QGuiApplication

from . import paths
from . import constants as C

# ============================================================
# ŚCIEŻKI / ŚRODOWISKO
# ============================================================
RESOURCE_DIR = paths.RESOURCE_DIR
APP_DIR = paths.APP_DIR
ICON_PATH = paths.ICON_PATH
sounds_dir = paths.SOUNDS_DIR
easyocr_dir = paths.EASYOCR_DIR


# ============================================================
# ŚCIEŻKI WYBIERANE PRZEZ UŻYTKOWNIKA (RUNTIME)
# ============================================================
audio_dir: str = ""
text_file_path: str = ""
names_file_path: str = ""
screenshot_dir: str = ""


# ============================================================
# USTAWIENIA MONITORA / PRZECHWYTYWANIA
# ============================================================
monitor = {"top": 900, "left": 375, "width": 1170, "height": 120}

monitor2_enabled = False
monitor2_top = 100
monitor2_left = 375
monitor2_width = 1170
monitor2_height = 120

active_monitor = 1
selected_screen_monitor = 1
selected_mss_monitor_rect = None


# ============================================================
# USTAWIENIA AUDIO
# ============================================================
VOLUME_FADE_DURATION = C.VOLUME_FADE_DURATION
VOLUME_REDUCTION_LEVEL = C.VOLUME_REDUCTION_LEVEL
ENABLE_OUTPUT2_SYSTEM = C.ENABLE_OUTPUT2_SYSTEM
ENABLE_DYNAMIC_SPEED = C.ENABLE_DYNAMIC_SPEED
BASE_PLAYBACK_SPEED = C.BASE_PLAYBACK_SPEED
OVERLAP_PLAYBACK_SPEED = C.OVERLAP_PLAYBACK_SPEED
AUDIO_QUEUE_SIZE = C.AUDIO_QUEUE_SIZE
SUPPORTED_AUDIO_FORMATS = C.SUPPORTED_AUDIO_FORMATS

audio_queue = queue.Queue(maxsize=AUDIO_QUEUE_SIZE)
is_audio_playing = False
audio_playing_lock = threading.Lock()


# ============================================================
# USTAWIENIA OCR / PRZETWARZANIA TEKSTU
# ============================================================
RESOLUTION_DOWNSCALE = C.RESOLUTION_DOWNSCALE
MIN_HEIGHT = C.MIN_HEIGHT
MAX_HEIGHT = C.MAX_HEIGHT
CAPTURE_INTERVAL = C.CAPTURE_INTERVAL
LINE_THRESHOLD = C.LINE_THRESHOLD
FRAME_DIFFERENCE_THRESHOLD = C.FRAME_DIFFERENCE_THRESHOLD

SIMILARITY_THRESHOLD = C.SIMILARITY_THRESHOLD
SIMILARITY_THRESHOLD2 = C.SIMILARITY_THRESHOLD2
SHORT_LINE_MAX_LENGTH = C.SHORT_LINE_MAX_LENGTH
OCR_MIN_CONFIDENCE = C.OCR_MIN_CONFIDENCE
TYPEWRITER_MIN_COVERAGE = C.TYPEWRITER_MIN_COVERAGE
TYPEWRITER_STABLE_READS = C.TYPEWRITER_STABLE_READS

ENABLE_REMOVE_CHARACTER_NAME = C.ENABLE_REMOVE_CHARACTER_NAME
ENABLE_SCREENSHOTS = C.ENABLE_SCREENSHOTS
ENABLE_PARAGRAPH_OCR = C.ENABLE_PARAGRAPH_OCR
ENABLE_TYPEWRITER_WAIT = C.ENABLE_TYPEWRITER_WAIT

EMPTY_READS_TO_IDLE = C.EMPTY_READS_TO_IDLE
IDLE_SLEEP_SECONDS = C.IDLE_SLEEP_SECONDS


# ============================================================
# LINIE ŚRODKOWE
# ============================================================
USE_CENTER_LINE_1 = C.USE_CENTER_LINE_1
USE_CENTER_LINE_2 = C.USE_CENTER_LINE_2
USE_CENTER_LINE_3 = C.USE_CENTER_LINE_3

TAB_ANIMATIONS = C.TAB_ANIMATIONS

CENTER_LINE_MARGIN = C.CENTER_LINE_MARGIN
CENTER_LINE_2_START = C.CENTER_LINE_2_START
CENTER_LINE_3_START_RATIO = C.CENTER_LINE_3_START_RATIO


# ============================================================
# LOGIKA POWTARZANIA
# ============================================================
REPLAY_DELAY_SECONDS = C.REPLAY_DELAY_SECONDS


# ============================================================
# STAN RUNTIME (DIALOGI / OCR)
# ============================================================
capture_enabled = False
dialog_lines: list[str] = []
dialogs: dict[str, str] = {}
character_names: set[str] = set()


# ============================================================
# STAN DEBUGA
# ============================================================
debug_console_window = None
debug_console_text = None
debug_enabled = False
debug_log_buffer: list[str] = []


# ============================================================
# STAN GŁÓWNEJ PĘTLI
# ============================================================
main_loop_thread = None


# ============================================================
# SKRÓTY KLAWISZOWE
# ============================================================
DEFAULT_KEY_BINDINGS = C.DEFAULT_KEY_BINDINGS
key_bindings = DEFAULT_KEY_BINDINGS.copy()


# ============================================================
# FLAGI DOSTĘPNOŚCI DYNAMICZNEJ PRĘDKOŚCI
# ============================================================
DYNAMIC_SPEED_AVAILABLE = False
PYDUB_AVAILABLE = False


# ============================================================
# ROZDZIELCZOŚĆ / SKALOWANIE
# ============================================================
resolution = "1920x1080"
lock_scaling = False


# ============================================================
# WARTOŚCI BAZOWE DO SKALOWANIA ROZDZIELCZOŚCI
# ============================================================
base_monitor_from_preset = monitor.copy()
base_monitor2_from_preset = None

base_downscale_from_preset = RESOLUTION_DOWNSCALE
base_min_height_from_preset = MIN_HEIGHT
base_max_height_from_preset = MAX_HEIGHT

base_resolution_from_preset = resolution


# ============================================================
# RUNTIME BAZA DO SKALOWANIA (GDY NIE MA PRESETU)
# ============================================================
runtime_base_resolution = resolution
runtime_base_monitor = None
runtime_base_monitor2 = None

runtime_base_downscale = RESOLUTION_DOWNSCALE
runtime_base_min_height = MIN_HEIGHT
runtime_base_max_height = MAX_HEIGHT


# ============================================================
# OSTATNIO UŻYWANE PRESETY
# ============================================================
recent_presets: list[dict] = []
RECENT_PRESETS_FILE = paths.RECENT_PRESETS_FILE
MAX_RECENT_PRESETS = 10
preset_filename = ""
preset_path = ""


# ============================================================
# FLAGI PRESETÓW LEGACY (TYLKO RUNTIME)
# ============================================================
_legacy_mode = False
_convert_legacy_preset = False


# ============================================================
# HELPERS
# ============================================================
# === FUNKCJA DO USTAWIANIA DOMYŚLNYCH WARTOŚCI (PO PRÓBIE WGRANIA PRESETU) ===
def ensure_defaults():
    global key_bindings

    if not key_bindings or not isinstance(key_bindings, dict):
        key_bindings = C.DEFAULT_KEY_BINDINGS.copy()
    else:
        for action, binding in C.DEFAULT_KEY_BINDINGS.items():
            key_bindings.setdefault(action, binding)

# === FUNKCJA DO ROBIENIA SNAPSHOTU CAŁEGO STANU (PRZED WGRANIEM PRESETU) ===
def snapshot():
    return vars().copy()

# === FUNKCJA DO PRZYWRACANIA SNAPSHOTU CAŁEGO STANU (PO PRÓBIE WGRANIA PRESETU) ===
def restore(snapshot_data: dict):
    current = vars()
    current.clear()
    current.update(snapshot_data)

# === FUNKCJA DO POBIERANIA GEOMETRII AKTYWNEGO MONITORA (Z UWZGLĘDNIENIEM DRUGIEGO MONITORA) ===
def get_active_monitor_rect() -> dict:
    if monitor2_enabled and active_monitor == 2:
        return {
            "top": monitor2_top,
            "left": monitor2_left,
            "width": monitor2_width,
            "height": monitor2_height,
        }

    return monitor

# === FUNKCJA DO POBIERANIA GEOMETRII WYBRANEGO MONITORA (Z UWZGLĘDNIENIEM DRUGIEGO MONITORA) ===
def get_selected_screen_geometry():
    screens = QGuiApplication.screens()
    idx = selected_screen_monitor - 1

    if 0 <= idx < len(screens):
        return screens[idx].geometry()

    return QGuiApplication.primaryScreen().geometry()

# === FUNKCJA DO POBIERANIA MONITORA MSS ODPOWIADAJĄCEGO WYBRANEMU MONITOROWI QT ===
def get_selected_mss_monitor(sct) -> dict:
    screens = QGuiApplication.screens()
    idx = selected_screen_monitor - 1

    if 0 <= idx < len(screens):
        screen = screens[idx]
    else:
        screen = QGuiApplication.primaryScreen()

    geo = screen.geometry()
    scale = screen.devicePixelRatio()

    # === Przeliczenie geometrii QT na skalę MSS ===
    qt_left = int(geo.left() * scale)
    qt_top = int(geo.top() * scale)
    qt_width = int(geo.width() * scale)
    qt_height = int(geo.height() * scale)

    # === Wyszukanie monitora MSS, który najlepiej pasuje do geometrii QT ===
    best = None
    for m in sct.monitors[1:]:
        if (m["left"] == qt_left and m["top"] == qt_top and
            m["width"] == qt_width and m["height"] == qt_height):
            return m

        # === (Fallback) Obliczenie "odległości" między monitorem MSS a geometrią QT ===
        diff = abs(m["left"] - qt_left) + abs(m["top"] - qt_top) + abs(m["width"] - qt_width) + abs(m["height"] - qt_height)
        if best is None or diff < best[0]:
            best = (diff, m)

    return best[1] if best else sct.monitors[1]

# === FUNKCJA DO USTAWIANIA GEOMETRII WYBRANEGO MONITORA MSS (PO ZMIANIE WYBORU MONITORA QT) ===
def update_selected_mss_monitor_rect(sct):
    global selected_mss_monitor_rect
    m = get_selected_mss_monitor(sct)
    selected_mss_monitor_rect = {
        "left": int(m["left"]),
        "top": int(m["top"]),
        "width": int(m["width"]),
        "height": int(m["height"]),
    }

# === FUNKCJA DO ODŚWIEŻANIA GEOMETRII WYBRANEGO MONITORA MSS (PO ZMIANIE WYBORU MONITORA QT) ===
def refresh_selected_mss_monitor_rect():
    import mss
    with mss.mss() as sct:
        update_selected_mss_monitor_rect(sct)