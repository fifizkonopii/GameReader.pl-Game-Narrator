
# ============================================================
# IMPORTY
# ============================================================
from __future__ import annotations

import json
import os
from datetime import datetime

from audio import player as audio
from core import debug, state
from core import constants as C
from core.validation import STRICT, SOFT, validate_state
from presets.schema import PresetValidationError, validate_preset
from utils.system import merge_key_bindings


# ============================================================
# CACHE / INTERNAL STATE
# ============================================================
_last_paths_signature: tuple[str, str, bool, str] | None = None


# ============================================================
# LEGACY SUPPORT
# ============================================================
def apply_legacy_defaults(data: dict) -> None:
    """
    Uzupełnia brakujące pola w presetach legacy (< 0.9.3).

    - NIE zapisuje do pliku.
    - Modyfikuje tylko lokalny dict `data`.
    - Dzięki temu preset legacy przechodzi walidację schematu.
    """
    # --- AUDIO ---
    data.setdefault("ENABLE_OUTPUT2_SYSTEM", C.ENABLE_OUTPUT2_SYSTEM)
    data.setdefault("ENABLE_DYNAMIC_SPEED", C.ENABLE_DYNAMIC_SPEED)
    data.setdefault("ENABLE_PARAGRAPH_OCR", C.ENABLE_PARAGRAPH_OCR)
    data.setdefault("ENABLE_TYPEWRITER_WAIT", C.ENABLE_TYPEWRITER_WAIT)
    data.setdefault("AUDIO_QUEUE_SIZE", C.AUDIO_QUEUE_SIZE)
    data.setdefault("BASE_PLAYBACK_SPEED", C.BASE_PLAYBACK_SPEED)
    data.setdefault("OVERLAP_PLAYBACK_SPEED", C.OVERLAP_PLAYBACK_SPEED)
    data.setdefault("VOLUME_REDUCTION_LEVEL", C.VOLUME_REDUCTION_LEVEL)
    data.setdefault("AUDIO_QUEUE_SIZE", C.AUDIO_QUEUE_SIZE)

    # --- PATHS ---
    data.setdefault("audio_dir", "")
    data.setdefault("text_file_path", "")
    data.setdefault("names_file_path", "")
    data.setdefault("screenshot_dir", "")

    # --- KEY BINDINGS ---
    kb = data.get("key_bindings")
    if not isinstance(kb, dict):
        kb = {}

    for key, default in C.DEFAULT_KEY_BINDINGS.items():
        kb.setdefault(key, default)

    data["key_bindings"] = kb


# ============================================================
# SAVE / LOAD PRESET
# ============================================================
def save_preset(
    file_path: str,
    entry_monitor2_top=None,
    entry_monitor2_left=None,
    entry_monitor2_width=None,
    entry_monitor2_height=None,
    resolution_dropdown=None,
):
    debug.log(debug.INFO, "Preset", f"Rozpoczęto zapis presetu: {os.path.basename(file_path)}")

    # =====================================================
    # WALIDACJA STANU PRZED ZAPISEM (SOFT)
    # =====================================================
    result = validate_state(state, mode=SOFT)

    if result.errors:
        msg = (
            f"Zapis presetu przerwany – błędy walidacji ({len(result.errors)}):\n"
            + "\n".join(f"• {e.field}: {e.message}" for e in result.errors)
        )
        debug.log(debug.ERROR, "Preset", msg)
        return result

    # =====================================================
    # BUDOWANIE DANYCH PRESETU
    # =====================================================
    preset_data = {
        "monitor": {
            "top": int(state.monitor["top"]),
            "left": int(state.monitor["left"]),
            "width": int(state.monitor["width"]),
            "height": int(state.monitor["height"]),
        },
        "resolution": state.resolution,

        "CENTER_LINE_MARGIN": state.CENTER_LINE_MARGIN,
        "CENTER_LINE_2_START": state.CENTER_LINE_2_START,
        "CENTER_LINE_3_START_RATIO": state.CENTER_LINE_3_START_RATIO,

        "RESOLUTION_DOWNSCALE": state.RESOLUTION_DOWNSCALE,
        "CAPTURE_INTERVAL": state.CAPTURE_INTERVAL,
        "MIN_HEIGHT": state.MIN_HEIGHT,
        "MAX_HEIGHT": state.MAX_HEIGHT,

        "ENABLE_REMOVE_CHARACTER_NAME": state.ENABLE_REMOVE_CHARACTER_NAME,
        "ENABLE_SCREENSHOTS": state.ENABLE_SCREENSHOTS,
        "ENABLE_PARAGRAPH_OCR": state.ENABLE_PARAGRAPH_OCR,
        "ENABLE_TYPEWRITER_WAIT": state.ENABLE_TYPEWRITER_WAIT,
        "ENABLE_OUTPUT2_SYSTEM": state.ENABLE_OUTPUT2_SYSTEM,
        "ENABLE_DYNAMIC_SPEED": state.ENABLE_DYNAMIC_SPEED,

        "BASE_PLAYBACK_SPEED": state.BASE_PLAYBACK_SPEED,
        "OVERLAP_PLAYBACK_SPEED": state.OVERLAP_PLAYBACK_SPEED,
        "VOLUME_REDUCTION_LEVEL": state.VOLUME_REDUCTION_LEVEL,
        "AUDIO_QUEUE_SIZE": state.AUDIO_QUEUE_SIZE,

        "USE_CENTER_LINE_1": state.USE_CENTER_LINE_1,
        "USE_CENTER_LINE_2": state.USE_CENTER_LINE_2,
        "USE_CENTER_LINE_3": state.USE_CENTER_LINE_3,

        "audio_dir": state.audio_dir,
        "text_file_path": state.text_file_path,
        "names_file_path": state.names_file_path,
        "screenshot_dir": state.screenshot_dir,

        "key_bindings": state.key_bindings.copy(),

        "monitor2_enabled": state.monitor2_enabled,
        "monitor2_top": int(state.monitor2_top),
        "monitor2_left": int(state.monitor2_left),
        "monitor2_width": int(state.monitor2_width),
        "monitor2_height": int(state.monitor2_height),
    }

    # =====================================================
    # ZAPIS DO PLIKU
    # =====================================================
    with open(file_path, "w", encoding="utf-8") as f:
        json.dump(preset_data, f, indent=4, ensure_ascii=False)

    state.preset_path = file_path
    state.preset_filename = os.path.basename(file_path)

    if result.warnings:
        msg = (
            f"Preset zapisany z ostrzeżeniami ({len(result.warnings)}):\n"
            + "\n".join(f"• {w.field}: {w.message}" for w in result.warnings)
        )
        debug.log(debug.WARNING, "Preset", msg)
    else:
        debug.log(debug.INFO, "Preset", f"Preset zapisany: {state.preset_filename}")

    return result

# =====================================================
# IGNOROWANIE NIEOBSŁUGIWANYCH KEY BINDINGSÓW W PRESECIE
# =====================================================
def _sanitize_key_bindings(kb: dict | None):
    if not isinstance(kb, dict):
        kb = {}

    allowed = set(C.DEFAULT_KEY_BINDINGS.keys())

    sanitized = {}
    skipped = []

    for action, value in kb.items():
        if action in allowed:
            sanitized[action] = value
        else:
            skipped.append(action)

    return sanitized, skipped

# =====================================================
# WCZYTYWANIE PRESETU
# =====================================================
def load_preset(file_path: str):
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        raise PresetValidationError(f"Preset ma nieprawidłowy format JSON.\n\n{e}")
    except Exception as e:
        raise PresetValidationError(f"Nie można wczytać presetu.\n\n{e}")

    return load_preset_from_data(data, file_path)

def load_preset_from_data(data: dict, file_path: str):
    state_snapshot = state.snapshot()
    debug.log(debug.INFO, "Preset", f"Rozpoczęto wczytywanie presetu: {os.path.basename(file_path)}")

    # =====================================================
    # ŚMIECIOWE JSONY
    # =====================================================
    if not isinstance(data, dict):
        debug.log(debug.ERROR, "Preset", f"Nieprawidłowy preset – oczekiwano obiektu JSON, otrzymano {type(data).__name__}")
        raise PresetValidationError(
            "Wybrany plik JSON nie jest presetem GameReadera.\n\n"
            "Preset musi być obiektem JSON { ... }, a nie listą lub inną strukturą."
        )

    # =====================================================
    # LEGACY DETECTION
    # =====================================================
    legacy_type = _detect_legacy_type(data)

    debug.log(debug.INFO, "Preset", f"Rozpoznano typ presetu: {legacy_type} ({_legacy_desc(legacy_type)})")

    # =====================================================
    # APPLY LEGACY DEFAULTS (ONLY < 0.9.3)
    # =====================================================
    if legacy_type == "C":
        apply_legacy_defaults(data)

    # =====================================================
    # WALIDACJA STRUKTURY PRESETU (SCHEMA)
    # =====================================================
    try:
        validate_preset(data)
    except PresetValidationError:
        debug.log(debug.ERROR, "Preset", "Błąd struktury presetu – walidacja schematu nie powiodła się")
        raise

    # =====================================================
    # PRZEPISANIE DO STATE (core wartości)
    # =====================================================
    state.monitor = data["monitor"]
    state.base_monitor_from_preset = state.monitor.copy()

    state.resolution = data.get("resolution", "1920x1080")
    state.base_resolution_from_preset = state.resolution

    state.CENTER_LINE_MARGIN = data["CENTER_LINE_MARGIN"]
    state.CENTER_LINE_2_START = data["CENTER_LINE_2_START"]
    state.CENTER_LINE_3_START_RATIO = data["CENTER_LINE_3_START_RATIO"]

    state.RESOLUTION_DOWNSCALE = data["RESOLUTION_DOWNSCALE"]
    state.base_downscale_from_preset = state.RESOLUTION_DOWNSCALE

    state.CAPTURE_INTERVAL = data["CAPTURE_INTERVAL"]

    state.MIN_HEIGHT = data["MIN_HEIGHT"]
    state.MAX_HEIGHT = data["MAX_HEIGHT"]
    state.base_min_height_from_preset = state.MIN_HEIGHT
    state.base_max_height_from_preset = state.MAX_HEIGHT

    state.ENABLE_REMOVE_CHARACTER_NAME = data["ENABLE_REMOVE_CHARACTER_NAME"]
    state.ENABLE_SCREENSHOTS = data["ENABLE_SCREENSHOTS"]
    state.ENABLE_PARAGRAPH_OCR = data.get("ENABLE_PARAGRAPH_OCR", C.ENABLE_PARAGRAPH_OCR)
    state.ENABLE_TYPEWRITER_WAIT = data.get("ENABLE_TYPEWRITER_WAIT", C.ENABLE_TYPEWRITER_WAIT)

    state.ENABLE_OUTPUT2_SYSTEM = data.get("ENABLE_OUTPUT2_SYSTEM", True)
    state.ENABLE_DYNAMIC_SPEED = data.get("ENABLE_DYNAMIC_SPEED", False)
    state.BASE_PLAYBACK_SPEED = data.get("BASE_PLAYBACK_SPEED", 1.0)
    state.OVERLAP_PLAYBACK_SPEED = data.get("OVERLAP_PLAYBACK_SPEED", 1.2)
    state.VOLUME_REDUCTION_LEVEL = data.get("VOLUME_REDUCTION_LEVEL", 0.2)
    state.AUDIO_QUEUE_SIZE = int(max(1, min(3, data.get("AUDIO_QUEUE_SIZE", 1))))

    state.USE_CENTER_LINE_1 = data["USE_CENTER_LINE_1"]
    state.USE_CENTER_LINE_2 = data["USE_CENTER_LINE_2"]
    state.USE_CENTER_LINE_3 = data.get("USE_CENTER_LINE_3", False)

    # =====================================================
    # KEY BINDINGS
    # =====================================================
    raw_kb = data.get("key_bindings")

    # === IGNOROWANIE NIEOBSŁUGIWANYCH KEY BINDINGSÓW W PRESECIE ===
    if legacy_type == "C" and isinstance(raw_kb, dict):
        raw_kb = raw_kb.copy()
        raw_kb.pop("switch_monitor_prev", None)
        raw_kb.pop("switch_monitor_next", None)

    # === MERGE Z DOMYŚLNYMI BINDINGAMI (NA WYPADKI BRAKU NIEKTÓRYCH) ===
    sanitized_kb, skipped_actions = _sanitize_key_bindings(raw_kb)

    # === LOGOWANIE POMINIĘTYCH BINDINGÓW ===
    if skipped_actions:
        debug.log(
            debug.INFO,
            "Preset",
            "Pominięto nieznane skróty klawiszowe: "
            + ", ".join(sorted(skipped_actions))
        )

    state.key_bindings = merge_key_bindings(sanitized_kb)

    # =====================================================
    # SPRAWDZANIE KTÓRE ŚCIEŻKI SĄ POPRAWNE W PRESECIE
    # =====================================================
    raw_audio = (data.get("audio_dir") or "").strip()
    raw_subs = (data.get("text_file_path") or "").strip()
    raw_names = (data.get("names_file_path") or "").strip()

    audio_ok = bool(raw_audio and os.path.isdir(raw_audio))
    subs_ok = bool(raw_subs and os.path.isfile(raw_subs))
    names_ok = bool(raw_names and os.path.isfile(raw_names)) if raw_names else True

    # =====================================================
    # AUTO-DETECT / AUTO-NAPRAWA ŚCIEŻEK
    # =====================================================
    preset_dir = os.path.dirname(file_path)

    auto_audio = auto_subtitles = auto_names = ""

    if not (audio_ok and subs_ok and names_ok):
        auto_audio, auto_subtitles, auto_names = auto_detect_paths(preset_dir)

    # === LOGOWANIE ===
    if not audio_ok and auto_audio:
        debug.log(
            debug.INFO,
            "Preset",
            f"Auto-detect: naprawiono folder audio ({auto_audio})"
        )

    if not subs_ok and auto_subtitles:
        debug.log(
            debug.INFO,
            "Preset",
            f"Auto-detect: naprawiono plik napisów ({auto_subtitles})"
        )

    if not names_ok and auto_names:
        debug.log(
            debug.INFO,
            "Preset",
            f"Auto-detect: naprawiono plik nazw postaci ({auto_names})"
        )

    state.audio_dir = _repair_audio_dir(data, auto_audio)
    state.text_file_path = _repair_text_file(data, auto_subtitles)
    state.names_file_path = _repair_names_file(data, auto_names)
    state.screenshot_dir = data.get("screenshot_dir", "")

    # =====================================================
    # MONITOR 2
    # =====================================================
    state.monitor2_top = int(data.get("monitor2_top", getattr(state, "monitor2_top", 0)))
    state.monitor2_left = int(data.get("monitor2_left", getattr(state, "monitor2_left", 0)))
    state.monitor2_width = int(data.get("monitor2_width", getattr(state, "monitor2_width", 800)))
    state.monitor2_height = int(data.get("monitor2_height", getattr(state, "monitor2_height", 600)))
    state.monitor2_enabled = bool(data.get("monitor2_enabled", getattr(state, "monitor2_enabled", False)))

    state.base_monitor2_from_preset = {
        "top": state.monitor2_top,
        "left": state.monitor2_left,
        "width": state.monitor2_width,
        "height": state.monitor2_height,
    }

    # =====================================================
    # TWARDA WALIDACJA STANU (STRICT)
    # =====================================================
    result = validate_state(state, mode=STRICT)

    if result.errors:
        msg = (
            f"Błędy walidacji po wczytaniu presetu ({len(result.errors)}):\n"
            + "\n".join(f"• {e.field}: {e.message}" for e in result.errors)
        )
        debug.log(debug.ERROR, "Preset", msg)

        # rollback do snapshotu – stan ma pozostać czysty
        state.restore(state_snapshot)
        return result

    if result.warnings:
        msg = (
            f"Wczytano preset z ostrzeżeniami ({len(result.warnings)}):\n"
            + "\n".join(f"• {w.field}: {w.message}" for w in result.warnings)
        )
        debug.log(debug.WARNING, "Preset", msg)

    # =====================================================
    # SUKCES: HISTORIA + LEGACY FLAG DLA GUI
    # =====================================================
    add_to_recent_presets(file_path)

    state._legacy_mode = (legacy_type == "C")
    result.legacy_type = legacy_type

    debug.log(debug.INFO, "Preset", f"Preset wczytany poprawnie: {os.path.basename(file_path)}")
    return result


# ============================================================
# DETEKCJA WERSJI PRESETU - HELPERS
# ============================================================
def _legacy_desc(legacy_type: str) -> str:
    return {
        "A": "aktualna struktura",
        "B": "struktura v0.9.3",
        "C": "struktura starsza niż v0.9.3",
    }.get(legacy_type, "nieznany")


def _detect_legacy_type(data: dict) -> str:
    LEGACY_KEYS_PRE_093 = {
        "ENABLE_OUTPUT2_SYSTEM",
        "ENABLE_DYNAMIC_SPEED",
        "BASE_PLAYBACK_SPEED",
        "OVERLAP_PLAYBACK_SPEED",
        "VOLUME_REDUCTION_LEVEL",
        "AUDIO_QUEUE_SIZE",
    }

    LEGACY_PATH_KEYS = {
        "audio_dir",
        "text_file_path",
        "names_file_path",
        "screenshot_dir",
    }

    LEGACY_KEYBINDINGS = {
        "switch_monitor_toggle",
        "base_speed_up",
        "base_speed_down",
        "overlap_speed_up",
        "overlap_speed_down",
        "debug_console",
        "toggle_areas",
    }

    missing_core_keys = [k for k in LEGACY_KEYS_PRE_093 if k not in data]

    kb = data.get("key_bindings")
    if isinstance(kb, dict):
        missing_binding_keys = [k for k in LEGACY_KEYBINDINGS if k not in kb]
    else:
        missing_binding_keys = list(LEGACY_KEYBINDINGS)

    # === C (preset starszy niż 0.9.3) ===
    if (
        len(missing_core_keys) == len(LEGACY_KEYS_PRE_093)
        and len(missing_binding_keys) == len(LEGACY_KEYBINDINGS)
    ):
        # === legacy presety mogą nie mieć ścieżek ===
        for k in LEGACY_PATH_KEYS:
            data.setdefault(k, "")
        return "C"

    # === B (preset 0.9.3)
    if "selected_screen_monitor" in data:
        return "B"

    return "A"


# ============================================================
# OSTATNIO UŻYWANE PRESETY
# ============================================================
def has_recent_presets() -> bool:
    path = state.RECENT_PRESETS_FILE
    if not os.path.exists(path):
        return False

    try:
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception:
        return False

    return isinstance(data, list) and len(data) > 0


def get_recent_presets():
    return state.recent_presets


def save_recent_presets() -> None:
    try:
        with open(state.RECENT_PRESETS_FILE, "w", encoding="utf-8") as f:
            json.dump(state.recent_presets, f, indent=2, ensure_ascii=False)
    except Exception as e:
        debug.log(debug.ERROR, "Preset", f"Błąd zapisu historii presetów: {e}")


def load_recent_presets() -> None:
    try:
        if os.path.exists(state.RECENT_PRESETS_FILE):
            with open(state.RECENT_PRESETS_FILE, "r", encoding="utf-8") as f:
                data = json.load(f)

            if isinstance(data, list):
                state.recent_presets = [
                    p for p in data
                    if isinstance(p, dict) and p.get("path")
                ]
            else:
                state.recent_presets = []
        else:
            state.recent_presets = []
    except Exception as e:
        debug.log(debug.ERROR, "Preset", f"Błąd wczytywania historii presetów: {e}")
        state.recent_presets = []


def remove_recent_preset(preset_path: str) -> None:
    state.recent_presets = [
        p for p in state.recent_presets
        if p.get("path") != preset_path
    ]
    save_recent_presets()


def clear_recent_presets() -> None:
    state.recent_presets = []
    save_recent_presets()


def add_to_recent_presets(preset_path: str) -> None:
    if not preset_path or not os.path.exists(preset_path):
        return

    preset_name = os.path.splitext(os.path.basename(preset_path))[0]
    preset_dir = os.path.dirname(preset_path)

    state.recent_presets = [p for p in state.recent_presets if p.get("path") != preset_path]

    state.recent_presets.insert(
        0,
        {
            "name": preset_name,
            "path": preset_path,
            "dir": preset_dir,
            "last_used": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        },
    )

    if len(state.recent_presets) > state.MAX_RECENT_PRESETS:
        state.recent_presets = state.recent_presets[: state.MAX_RECENT_PRESETS]

    save_recent_presets()


# ============================================================
# RELOAD DIALOGÓW / NAZW (CACHE)
# ============================================================
def reload_dialogs_and_names(verbose: bool = False) -> None:
    global _last_paths_signature

    sig = (
        os.path.abspath(state.audio_dir) if state.audio_dir else "",
        os.path.abspath(state.text_file_path) if state.text_file_path else "",
        bool(state.ENABLE_REMOVE_CHARACTER_NAME),
        os.path.abspath(state.names_file_path) if state.names_file_path else "",
    )

    if _last_paths_signature == sig:
        if verbose:
            debug.log(debug.DEBUG, "Preset", "reload_dialogs_and_names: brak zmian – pomijam przeładowanie")
        return

    _last_paths_signature = sig

    state.dialog_lines = []
    state.dialogs = {}

    # === SUBTITLES -> dialog_lines ===
    if state.text_file_path and os.path.exists(state.text_file_path):
        with open(state.text_file_path, "r", encoding="utf-8") as f:
            state.dialog_lines = [line.strip() for line in f.readlines()]

        # === AUDIO MAP (dialog -> plik) ===
        if state.audio_dir and os.path.isdir(state.audio_dir):
            for i, line in enumerate(state.dialog_lines):
                if not line:
                    continue

                base_name = f"output1 ({i+1})"
                found_path = None

                for ext in state.SUPPORTED_AUDIO_FORMATS:
                    path = os.path.join(state.audio_dir, base_name + ext)
                    if os.path.exists(path):
                        found_path = path
                        break

                if found_path:
                    state.dialogs[line] = found_path

    # === NAZWY POSTACI ===
    state.character_names = set()

    if state.ENABLE_REMOVE_CHARACTER_NAME:
        if state.names_file_path and os.path.exists(state.names_file_path):
            try:
                with open(state.names_file_path, "r", encoding="utf-8") as f:
                    state.character_names = {line.strip() for line in f.readlines()}
                debug.log(debug.INFO, "Preset", f"Wczytano nazwy postaci: {state.names_file_path}")
            except FileNotFoundError:
                debug.log(debug.WARNING, "Preset", "Nie znaleziono pliku z nazwami postaci")
        else:
            debug.log(debug.WARNING, "Preset", "Usuwanie nazw postaci włączone, ale brak pliku z nazwami postaci")

    if verbose:
        audio.list_available_audio_files(state.audio_dir)


# ============================================================
# AUTO-DETECT ŚCIEŻEK
# ============================================================
def auto_detect_paths(preset_dir: str):
    def norm(path: str) -> str:
        return os.path.normpath(path).replace("\\", "/")

    detected_audio = ""
    detected_subtitles = ""
    detected_names = ""

    # =========================
    # AUDIO
    # =========================
    audio_candidates = [
        os.path.join(preset_dir, "audio"),
        os.path.join(preset_dir, "Audio"),
    ]

    def has_output_audio_files(folder: str) -> bool:
        try:
            for fname in os.listdir(folder):
                low = fname.lower()
                if "output1" in low and any(low.endswith(ext) for ext in state.SUPPORTED_AUDIO_FORMATS):
                    return True
        except Exception:
            pass
        return False

    for path in audio_candidates:
        if os.path.isdir(path) and has_output_audio_files(path):
            detected_audio = norm(path)
            break

    # =========================
    # SUBTITLES
    # =========================
    subtitle_dirs = [
        preset_dir,
        os.path.join(preset_dir, "subtitles"),
        os.path.join(preset_dir, "Subs"),
        os.path.join(preset_dir, "Subtitle"),
    ]

    subtitle_names = (
        "subtitles.txt",
        "subtitlespl.txt",
        "subtitles_pl.txt",
        "dialogues.txt",
    )

    for base in subtitle_dirs:
        if not os.path.isdir(base) and base != preset_dir:
            continue

        try:
            for fname in os.listdir(base):
                if fname.lower() in subtitle_names:
                    detected_subtitles = norm(os.path.join(base, fname))
                    raise StopIteration
        except StopIteration:
            break
        except Exception:
            pass

    # =========================
    # NAMES
    # =========================
    names_dirs = [
        preset_dir,
        os.path.join(preset_dir, "subtitles"),
    ]

    for base in names_dirs:
        if not os.path.isdir(base) and base != preset_dir:
            continue

        try:
            for fname in os.listdir(base):
                if fname.lower() == "names.txt":
                    detected_names = norm(os.path.join(base, fname))
                    raise StopIteration
        except StopIteration:
            break
        except Exception:
            pass

    return detected_audio, detected_subtitles, detected_names


# ============================================================
# PATH REPAIR HELPERS (AUTO-NAPRAWA)
# ============================================================
def _repair_audio_dir(data: dict, auto_audio: str) -> str:
    if "audio_dir" not in data:
        return ""
    raw = (data.get("audio_dir") or "").strip()
    return raw if raw and os.path.isdir(raw) else auto_audio


def _repair_text_file(data: dict, auto_subtitles: str) -> str:
    if "text_file_path" not in data:
        return ""
    raw = (data.get("text_file_path") or "").strip()
    return raw if raw and os.path.isfile(raw) else auto_subtitles


def _repair_names_file(data: dict, auto_names: str) -> str:
    if "names_file_path" not in data:
        return ""
    raw = (data.get("names_file_path") or "").strip()
    return raw if raw and os.path.isfile(raw) else auto_names


# ============================================================
# LEGACY CONVERSION
# ============================================================
def convert_legacy_preset_in_place(preset_path: str) -> None:
    if not preset_path:
        return

    save_preset(preset_path)
    debug.log(debug.INFO, "Preset", f"Legacy preset przekonwertowany: {preset_path}")