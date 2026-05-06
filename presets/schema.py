
# ============================================================
# WYJĄTKI
# ============================================================
class PresetValidationError(Exception):
    pass

# ============================================================
# SCHEMAT PRESETU
# ============================================================
PRESET_SCHEMA = {
    "monitor": {
        "top": int,
        "left": int,
        "width": int,
        "height": int,
    },

    "resolution": str,

    "CENTER_LINE_MARGIN": int,
    "CENTER_LINE_2_START": int,
    "CENTER_LINE_3_START_RATIO": (int, float),

    "RESOLUTION_DOWNSCALE": (int, float),
    "CAPTURE_INTERVAL": (int, float),

    "MIN_HEIGHT": int,
    "MAX_HEIGHT": int,

    "ENABLE_REMOVE_CHARACTER_NAME": bool,
    "ENABLE_SCREENSHOTS": bool,
    "ENABLE_OUTPUT2_SYSTEM": bool,
    "ENABLE_DYNAMIC_SPEED": bool,

    "BASE_PLAYBACK_SPEED": (int, float),
    "OVERLAP_PLAYBACK_SPEED": (int, float),

    "USE_CENTER_LINE_1": bool,
    "USE_CENTER_LINE_2": bool,
    "USE_CENTER_LINE_3": bool,

    "audio_dir": str,
    "text_file_path": str,
    "names_file_path": str,
    "screenshot_dir": str,

    "key_bindings": dict,

    "monitor2_enabled": bool,
    "monitor2_top": int,
    "monitor2_left": int,
    "monitor2_width": int,
    "monitor2_height": int,

    "VOLUME_REDUCTION_LEVEL": (int, float),
    "AUDIO_QUEUE_SIZE": int,
}

# ============================================================
# WALIDACJA PRESETU
# ============================================================
def validate_preset(data: dict):
    if not isinstance(data, dict):
        raise PresetValidationError("Preset nie jest obiektem JSON!")

    errors = []

    for key, expected_type in PRESET_SCHEMA.items():
        if key not in data:
            errors.append(f"Brak klucza: '{key}'")
            continue

        value = data[key]

        if isinstance(expected_type, dict):
            if not isinstance(value, dict):
                errors.append(f"'{key}' musi być obiektem")
                continue

            for subkey, subtype in expected_type.items():
                if subkey not in value:
                    errors.append(f"Brak '{key}.{subkey}'")
                elif not isinstance(value[subkey], subtype):
                    errors.append(
                        f"'{key}.{subkey}' ma zły typ "
                        f"(oczekiwany {subtype.__name__})"
                    )

        else:
            if not isinstance(value, expected_type):
                tname = (
                    ", ".join(t.__name__ for t in expected_type)
                    if isinstance(expected_type, tuple)
                    else expected_type.__name__
                )
                errors.append(
                    f"'{key}' ma zły typ (oczekiwany {tname})"
                )

    if errors:
        msg = "Nieprawidłowa struktura presetu:\n\n• " + "\n• ".join(errors)
        raise PresetValidationError(msg)

    return True
