
# ============================================================
# IMPORTY
# ============================================================
import os
import re
from dataclasses import dataclass
from typing import List

from core import constants as C
from core.constants import SUPPORTED_RESOLUTIONS
from ui.keymap import normalize_qt_sequence

# ============================================================
# MODELE WYNIKU
# ============================================================
@dataclass
class ValidationIssue:
    field: str
    message: str
    level: str  # "error" | "warning"


@dataclass
class ValidationResult:
    errors: List[ValidationIssue]
    warnings: List[ValidationIssue]

    @property
    def is_valid(self) -> bool:
        return not self.errors


# ============================================================
# TRYBY WALIDACJI
# ============================================================
STRICT = "strict"
NORMAL = "normal"
SOFT = "soft"

# ============================================================
# NIESYTANDARDOWE KOMUNIKATY
# ============================================================
_PATH_LABELS = {
    "audio_dir": "Ścieżka do plików audio",
    "screenshot_dir": "Ścieżka do zapisu zrzutów ekranu",
    "text_file_path": "Ścieżka do pliku dialogów",
    "names_file_path": "Ścieżka do pliku nazw postaci",
}

_FILE_LABELS = {
    "text_file_path": "Plik dialogów",
    "names_file_path": "Plik nazw postaci",
}

def _path_label(field: str) -> str:
    return _PATH_LABELS.get(field, field)

def _file_label(field: str) -> str:
    return _FILE_LABELS.get(field, field)

# ============================================================
# API PUBLICZNE
# ============================================================
def validate_state(state, *, mode=STRICT) -> ValidationResult:
    errors: List[ValidationIssue] = []
    warnings: List[ValidationIssue] = []

    # === OCR / WYDAJNOŚĆ ===
    _float_range(
        errors,
        "RESOLUTION_DOWNSCALE",
        state.RESOLUTION_DOWNSCALE,
        C.RESOLUTION_DOWNSCALE_MIN,
        C.RESOLUTION_DOWNSCALE_MAX,
    )
    _float_range(
        errors,
        "CAPTURE_INTERVAL",
        state.CAPTURE_INTERVAL,
        C.CAPTURE_INTERVAL_MIN,
        C.CAPTURE_INTERVAL_MAX,
    )

    # === ROZDZIELCZOŚĆ GRY ===
    _validate_resolution(errors, "resolution", getattr(state, "resolution", None))

    _int_range(
        errors,
        "MIN_HEIGHT",
        state.MIN_HEIGHT,
        C.MIN_HEIGHT_MIN,
        C.MIN_HEIGHT_MAX,
    )
    _int_range(
        errors,
        "MAX_HEIGHT",
        state.MAX_HEIGHT,
        C.MAX_HEIGHT_MIN,
        C.MAX_HEIGHT_MAX,
    )

    if isinstance(state.MIN_HEIGHT, int) and isinstance(state.MAX_HEIGHT, int):
        if state.MIN_HEIGHT > state.MAX_HEIGHT:
            errors.append(
                ValidationIssue(
                    "MIN_HEIGHT/MAX_HEIGHT",
                    "Wysokość minimalna nie może być większa niż maksymalna",
                    "error",
                )
            )

    # === LINIE POMOCNICZE ===
    _int_range(
        errors,
        "CENTER_LINE_MARGIN",
        state.CENTER_LINE_MARGIN,
        C.CENTER_LINE_MARGIN_MIN,
        C.CENTER_LINE_MARGIN_MAX,
    )
    _int_range(
        errors,
        "CENTER_LINE_2_START",
        state.CENTER_LINE_2_START,
        C.CENTER_LINE_2_START_MIN,
        C.CENTER_LINE_2_START_MAX,
    )
    _float_range(
        errors,
        "CENTER_LINE_3_START_RATIO",
        state.CENTER_LINE_3_START_RATIO,
        C.CENTER_LINE_3_START_RATIO_MIN,
        C.CENTER_LINE_3_START_RATIO_MAX,
    )

    # === AUDIO ===
    _float_range(errors, "BASE_PLAYBACK_SPEED", state.BASE_PLAYBACK_SPEED, C.BASE_PLAYBACK_SPEED_MIN, C.BASE_PLAYBACK_SPEED_MAX)
    _float_range(errors, "OVERLAP_PLAYBACK_SPEED", state.OVERLAP_PLAYBACK_SPEED, C.OVERLAP_PLAYBACK_SPEED_MIN, C.OVERLAP_PLAYBACK_SPEED_MAX)
    _float_range(
        errors,
        "VOLUME_REDUCTION_LEVEL",
        state.VOLUME_REDUCTION_LEVEL,
        C.VOLUME_REDUCTION_LEVEL_MIN,
        C.VOLUME_REDUCTION_LEVEL_MAX,
    )

    if state.AUDIO_QUEUE_SIZE not in C.AUDIO_QUEUE_SIZE_ALLOWED:
        errors.append(
            ValidationIssue(
                "AUDIO_QUEUE_SIZE",
                "Dozwolone wartości: 1, 2 lub 3",
                "error",
            )
        )

    # === ŚCIEŻKI ===
    _validate_dir(errors, warnings, "audio_dir", state.audio_dir)
    _validate_txt(errors, warnings, "text_file_path", state.text_file_path)
    _validate_txt(errors, warnings, "names_file_path", state.names_file_path)
    _validate_dir(errors, warnings, "screenshot_dir", state.screenshot_dir)

    # === KEY BINDINGS ===
    _validate_key_bindings(errors, warnings, state.key_bindings)

    return ValidationResult(errors=errors, warnings=warnings)


# ============================================================
# WALIDATORY POMOCNICZE
# ============================================================
def _int_range(errors, field, value, min_v, max_v):
    if not isinstance(value, int):
        errors.append(ValidationIssue(field, "Wartość musi być liczbą całkowitą", "error"))
        return
    if not (min_v <= value <= max_v):
        errors.append(
            ValidationIssue(
                field,
                f"Wartość musi być w zakresie {min_v}–{max_v}",
                "error",
            )
        )

def _float_range(errors, field, value, min_v, max_v):
    if not isinstance(value, (float, int)):
        errors.append(ValidationIssue(field, "Wartość musi być liczbą", "error"))
        return
    if not (min_v <= float(value) <= max_v):
        errors.append(
            ValidationIssue(
                field,
                f"Wartość musi być w zakresie {min_v}–{max_v}",
                "error",
            )
        )

def _validate_dir(errors, warnings, field, path):
    if not path:
        return

    if not isinstance(path, str):
        errors.append(
            ValidationIssue(
                field,
                f"{_path_label(field)} musi być tekstem",
                "error",
            )
        )
        return

    if os.path.exists(path) and not os.path.isdir(path):
        errors.append(
            ValidationIssue(
                field,
                f"{_path_label(field)} nie jest folderem",
                "error",
            )
        )
        return

    if not os.path.exists(path):
        warnings.append(
            ValidationIssue(
                field,
                f"{_path_label(field)} nie istnieje",
                "warning",
            )
        )

def _validate_txt(errors, warnings, field, path):
    if not path:
        return

    if not isinstance(path, str):
        errors.append(
            ValidationIssue(
                field,
                f"{_path_label(field)} musi być tekstem",
                "error",
            )
        )
        return

    if not path.lower().endswith(".txt"):
        errors.append(
            ValidationIssue(
                field,
                f"{_file_label(field)} musi mieć rozszerzenie .txt",
                "error",
            )
        )
        return

    if not os.path.isfile(path):
        warnings.append(
            ValidationIssue(
                field,
                f"{_file_label(field)} nie istnieje",
                "warning",
            )
        )

def _validate_resolution(errors, field, value):
    if value is None or value == "":
        errors.append(
            ValidationIssue(
                field,
                "Brak ustawionej rozdzielczości gry",
                "error",
            )
        )
        return

    if not isinstance(value, str):
        errors.append(
            ValidationIssue(
                field,
                "Rozdzielczość musi być tekstem",
                "error",
            )
        )
        return

    normalized = value.replace(" ", "").lower()

    if normalized not in SUPPORTED_RESOLUTIONS:
        allowed = ", ".join(SUPPORTED_RESOLUTIONS.keys())
        errors.append(
            ValidationIssue(
                field,
                f"Nieobsługiwana rozdzielczość gry: {value}.\n\nDozwolone:\n{allowed}",
                "error",
            )
        )


# ============================================================
# KEY BINDINGS
# ============================================================
_BLOCKED_SHORTCUTS = {
    "alt+tab",
    "alt+f4",
    "ctrl+alt+del",
    "ctrl+shift+esc",
}
_ALLOWED_KEYS = re.compile(
    r"^(?:[a-z0-9_`]+|f[1-9]|f1[0-2]|home|end|insert|delete|page_up|page_down)$"
)
_ALLOWED_MODS = {"ctrl", "alt", "shift"}

def _validate_key_bindings(errors, warnings, bindings):
    if not isinstance(bindings, dict):
        errors.append(
            ValidationIssue("key_bindings", "Nieprawidłowy format", "error")
        )
        return

    used = {}

    for action, value in bindings.items():
        if value is None:
            continue

        if not isinstance(value, str) or not value:
            errors.append(
                ValidationIssue(
                    f"key_bindings.{action}",
                    "Skrót musi być stringiem lub null",
                    "error",
                )
            )
            continue

        normalized = normalize_qt_sequence(value)
        if not normalized:
            errors.append(
                ValidationIssue(
                    f"key_bindings.{action}",
                    "Nieprawidłowy skrót klawiszowy",
                    "error",
                )
            )
            continue

        if normalized in _BLOCKED_SHORTCUTS:
            errors.append(
                ValidationIssue(
                    f"key_bindings.{action}",
                    f"Skrót {normalized} jest zablokowany przez system",
                    "error",
                )
            )
            continue

        parts = normalized.split("+")
        key = parts[-1]
        mods = parts[:-1]

        if not _ALLOWED_KEYS.fullmatch(key):
            errors.append(
                ValidationIssue(
                    f"key_bindings.{action}",
                    f"Nieznany klawisz: {key}",
                    "error",
                )
            )
            continue

        for m in mods:
            if m not in _ALLOWED_MODS:
                errors.append(
                    ValidationIssue(
                        f"key_bindings.{action}",
                        f"Nieznany modyfikator: {m}",
                        "error",
                    )
                )

        if normalized in used:
            warnings.append(
                ValidationIssue(
                    f"key_bindings.{action}",
                    f"Zdublowane przypisanie skrótu klawiszowego {key.upper()}",
                    "warning",
                )
            )
        else:
            used[normalized] = action

def _validate_key_bindings_strict(errors, bindings):
    used = {}

    for action, value in bindings.items():
        if not value:
            continue

        normalized = normalize_qt_sequence(value)
        if not normalized:
            continue

        if normalized in used:
            key = normalized.split("+")[-1].upper()
            errors.append(
                ValidationIssue(
                    f"key_bindings.{action}",
                    "Wykryto zdublowane skróty klawiszowe.\n\n"
                    "Ten sam skrót został przypisany do więcej niż jednej akcji.\n"
                    "Przejdź do zakładki „Skróty klawiszowe” i popraw konflikty.",
                    "error",
                )
            )
            return

        used[normalized] = action


# ============================================================
# WALIDACJA STARTU LEKTORA (RUNTIME)
# ============================================================
def validate_before_reader_start(state):
    errors = []
    warnings = []

    # === ZDUBLOWANE SKRÓTY KLAWISZOWE (ERROR) ===
    _validate_key_bindings_strict(errors, state.key_bindings)
    if errors:
        return ValidationResult(errors, warnings)

    # ===AUDIO_DIR – MUSI BYĆ PODANE ===
    audio_dir = state.audio_dir
    if not audio_dir:
        errors.append(
            ValidationIssue(
                "audio_dir",
                "Nie wskazano folderu z plikami audio.\n\n"
                "Przejdź do zakładki „Foldery i pliki” i wybierz folder z nagraniami lektora.",
                "error",
            )
        )
        return ValidationResult(errors, warnings)

    if not os.path.exists(audio_dir):
        errors.append(
            ValidationIssue(
                "audio_dir",
                "Wskazany folder z plikami audio nie istnieje.\n\n"
                "Sprawdź, czy folder nie został usunięty lub przeniesiony,\n"
                "a następnie wybierz poprawną lokalizację.",
                "error",
            )
        )
        return ValidationResult(errors, warnings)

    if not os.path.isdir(audio_dir):
        errors.append(
            ValidationIssue(
                "audio_dir",
                "Wskazana ścieżka audio nie jest folderem.\n\n"
                "Wybierz folder, który zawiera pliki audio (output1 / output2).",
                "error",
            )
        )
        return ValidationResult(errors, warnings)

    # === OUTPUT1 – MUSI ISTNIEĆ ===
    output1_files = [
        f for f in os.listdir(audio_dir)
        if "output1" in f.lower()
    ]

    if not output1_files:
        errors.append(
            ValidationIssue(
                "audio_dir",
                "Nie znaleziono plików „output1” w folderze audio.\n\n"
                "Folder musi zawierać pliki audio o nazwach:\n"
                "output1 (1), output1 (2), output1 (3), itd.",
                "error",
            )
        )
        return ValidationResult(errors, warnings)

    # === FORMATY OUTPUT1 ===
    for f in output1_files:
        ext = os.path.splitext(f)[1].lower()
        if ext not in state.SUPPORTED_AUDIO_FORMATS:
            errors.append(
                ValidationIssue(
                    "audio_dir",
                    "Pliki „output1” mają nieobsługiwany format.\n\n"
                    "Obsługiwane formaty:\n"
                    + ", ".join(state.SUPPORTED_AUDIO_FORMATS),
                    "error",
                )
            )
            return ValidationResult(errors, warnings)

    # === OUTPUT2 – WARUNKOWO ===
    if state.ENABLE_OUTPUT2_SYSTEM and not state.ENABLE_DYNAMIC_SPEED:
        output2_files = [
            f for f in os.listdir(audio_dir)
            if "output2" in f.lower()
        ]

        if not output2_files:
            errors.append(
                ValidationIssue(
                    "audio_dir",
                    "Brakuje plików „output2”.\n\n"
                    "Masz włączony system stałych prędkości audio, ale w folderze nie ma plików output2.\n\n"
                    "Zmień system prędkości na dynamiczny lub dodaj pliki output2 do folderu audio.",
                    "error",
                )
            )
            return ValidationResult(errors, warnings)

        for f in output2_files:
            ext = os.path.splitext(f)[1].lower()
            if ext not in state.SUPPORTED_AUDIO_FORMATS:
                errors.append(
                    ValidationIssue(
                        "audio_dir",
                        "Pliki „output2” mają nieobsługiwany format audio.\n\n"
                        "Obsługiwane formaty:\n"
                        + ", ".join(state.SUPPORTED_AUDIO_FORMATS),
                        "error",
                    )
                )
                return ValidationResult(errors, warnings)

    # === PLIK DIALOGÓW TXT ===
    if not state.text_file_path or not os.path.isfile(state.text_file_path):
        errors.append(
            ValidationIssue(
                "text_file_path",
                "Nie wybrano pliku z dialogami gry.\n\n"
                "Wybierz plik .txt zawierający dialogi w zakładce „Foldery i pliki”.",
                "error",
            )
        )
        return ValidationResult(errors, warnings)

    # === PLIK NAZW POSTACI – WARUNKOWO ===
    if state.ENABLE_REMOVE_CHARACTER_NAME:
        if not state.names_file_path:
            errors.append(
                ValidationIssue(
                    "names_file_path",
                    "Usuwanie nazw postaci jest włączone, ale nie wybrano pliku z nazwami.\n\n"
                    "Wybierz plik .txt z listą postaci lub wyłącz tę opcję w ustawieniach.",
                    "error",
                )
            )
            return ValidationResult(errors, warnings)

        if not os.path.isfile(state.names_file_path):
            errors.append(
                ValidationIssue(
                    "names_file_path",
                    "Wskazany plik z nazwami postaci nie istnieje.\n\n"
                    "Sprawdź, czy plik nie został usunięty lub wybierz poprawny plik .txt.",
                    "error",
                )
            )
            return ValidationResult(errors, warnings)
        
    # === SCREENSHOTS – WALIDACJA ŚCIEŻKI ZAPISU ===
    if state.ENABLE_SCREENSHOTS:
        # ❌ brak ścieżki
        if not state.screenshot_dir or not state.screenshot_dir.strip():
            errors.append(
                ValidationIssue(
                    "screenshot_dir",
                    "Włączono zapisywanie zrzutów ekranu, ale nie ustawiono folderu zapisu.\n\n"
                    "Przejdź do zakładki „Foldery i pliki” i wybierz folder zapisu zrzutów ekranu.",
                    "error",
                )
            )
            return ValidationResult(errors, warnings)

        if not os.path.isdir(state.screenshot_dir):
            errors.append(
                ValidationIssue(
                    "screenshot_dir",
                    "Wskazany folder zapisu zrzutów ekranu nie istnieje.\n\n"
                    "Sprawdź, czy folder nie został usunięty lub wybierz poprawną lokalizację.",
                    "error",
                )
            )
            return ValidationResult(errors, warnings)

    return ValidationResult(errors=[], warnings=[])