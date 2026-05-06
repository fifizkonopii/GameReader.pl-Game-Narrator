
# ============================================================
# IMPORTY
# ============================================================
import os
import sys

# ============================================================
# TRYB URUCHOMIENIA
# ============================================================
def is_compiled() -> bool:
    # Nuitka ustawia __compiled__, PyInstaller ustawia sys.frozen
    return "__compiled__" in globals() or bool(getattr(sys, "frozen", False))


# ============================================================
# ROOT / APLIKACJA
# ============================================================
PROJECT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

if is_compiled():
    # === URUCHAMIANIE Z PLIKU .exe ===
    APP_DIR = os.path.dirname(sys.argv[0])
else:
    # === URUCHAMIANIE Z PYTHONA ===
    APP_DIR = PROJECT_DIR


# ============================================================
# ZASOBY RUNTIME (CORE)
# ============================================================
RESOURCE_DIR = os.path.join(APP_DIR, "resources")
if not os.path.isdir(RESOURCE_DIR):
    RESOURCE_DIR = APP_DIR

ICON_PATH = os.path.join(RESOURCE_DIR, "icon.ico")

SOUNDS_DIR = os.path.join(RESOURCE_DIR, "sounds")
BIN_DIR = os.path.join(RESOURCE_DIR, "bin")

FFMPEG_PATH = os.path.join(BIN_DIR, "ffmpeg.exe")
FFPROBE_PATH = os.path.join(BIN_DIR, "ffprobe.exe")


# ============================================================
# DANE APLIKACJI (RUNTIME / UŻYTKOWNIK)
# ============================================================
EASYOCR_DIR = os.path.join(APP_DIR, ".EasyOCR")
RECENT_PRESETS_FILE = os.path.join(APP_DIR, "recent_presets.json")

def recent_presets_path() -> str:
    return RECENT_PRESETS_FILE


# ============================================================
# GUI ASSETS
# ============================================================
ASSETS_DIR = os.path.join(PROJECT_DIR, "assets")
IMAGES_DIR = os.path.join(ASSETS_DIR, "images")
FONTS_DIR = os.path.join(ASSETS_DIR, "fonts")

def asset_path(*parts: str) -> str:
    return os.path.join(ASSETS_DIR, *parts)
