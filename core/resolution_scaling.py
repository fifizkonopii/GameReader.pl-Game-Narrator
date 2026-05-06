
# ============================================================
# IMPORTY
# ============================================================
from core import state
from core import constants as C
from core.constants import SUPPORTED_RESOLUTIONS
from core import debug

# ============================================================
# POMOCNICZE: WYZNACZENIE OBSZARU 16:9
# ============================================================
def _get_16_9_core(width: int, height: int):
    target_ratio = 16 / 9
    ratio = width / height

    if abs(ratio - target_ratio) < 0.01:
        return width, height, 0

    core_width = int(height * target_ratio)
    offset_x = (width - core_width) // 2

    return core_width, height, offset_x

# ============================================================
# GŁÓWNE PRZELICZENIE ROZDZIELCZOŚCI
# ============================================================
def recalculate_for_resolution(new_resolution: str):
    # === BLOKADA SKALOWANIA (GLOBALNA) ===
    if getattr(state, "lock_scaling", False):
        debug.log(debug.DEBUG, "RESCALING", "Skalowanie zablokowane – pomijam przeliczenie")
        return

    # ==================================================
    # WYBÓR ŹRÓDŁA BAZY
    # ==================================================
    has_preset = bool(state.preset_path)

    if has_preset:
        base_monitor = state.base_monitor_from_preset
        base_monitor2 = state.base_monitor2_from_preset
        base_res = state.base_resolution_from_preset or "1920x1080"
    else:
        if state.runtime_base_monitor and state.runtime_base_resolution:
            base_monitor = state.runtime_base_monitor
            base_monitor2 = state.runtime_base_monitor2
            base_res = state.runtime_base_resolution
        else:
            base_monitor = state.monitor
            base_res = state.resolution

            if state.monitor2_enabled:
                base_monitor2 = {
                    "top": state.monitor2_top,
                    "left": state.monitor2_left,
                    "width": state.monitor2_width,
                    "height": state.monitor2_height,
                }
            else:
                base_monitor2 = None

    # === NIEZNANA ROZDZIELCZOŚĆ ===
    if new_resolution not in SUPPORTED_RESOLUTIONS:
        debug.log(debug.WARNING, "RESCALING", f"Nieznana rozdzielczość: {new_resolution} – pomijam przeliczenie")
        return

    # === POWRÓT DO ROZDZIELCZOŚCI BAZOWEJ (PRESET LUB RUNTIME) ===
    if new_resolution == base_res:

        if has_preset:
            msg = "ustawiono wartości bazowe presetu"

        elif state.runtime_base_monitor and state.runtime_base_resolution:
            msg = "ustawiono zdefiniowane wartości"

        else:
            msg = "ustawiono wartości domyślne"

        debug.log(debug.INFO, "RESCALING", f"Ustawiona rozdzielczość bazowa: ({base_res}) – {msg}")

        # === OBSZAR 1 ===
        state.monitor = base_monitor.copy()

        # === OBSZAR 2 (JEŚLI ISTNIEJE) ===
        if base_monitor2:
            b2 = base_monitor2
            state.monitor2_top = b2["top"]
            state.monitor2_left = b2["left"]
            state.monitor2_width = b2["width"]
            state.monitor2_height = b2["height"]

        # === OCR (POWRÓT DO BAZY) ===
        if has_preset:
            state.RESOLUTION_DOWNSCALE = state.base_downscale_from_preset
            state.MIN_HEIGHT = state.base_min_height_from_preset
            state.MAX_HEIGHT = state.base_max_height_from_preset
        else:
            state.RESOLUTION_DOWNSCALE = state.runtime_base_downscale
            state.MIN_HEIGHT = state.runtime_base_min_height
            state.MAX_HEIGHT = state.runtime_base_max_height

        return

    # === LOG STARTU PRZELICZANIA ===
    debug.log(debug.INFO, "RESCALING", f"Przeliczanie rozdzielczości: {base_res} -> {new_resolution}")

    # === ROZDZIELCZOŚCI BAZOWA I DOCELOWA ===
    base_w, base_h = SUPPORTED_RESOLUTIONS[base_res]
    target_w, target_h = SUPPORTED_RESOLUTIONS[new_resolution]

    # === OBSZAR 16:9 (BAZA / TARGET) ===
    base_core_w, base_core_h, base_offset = _get_16_9_core(base_w, base_h)
    target_core_w, target_core_h, target_offset = _get_16_9_core(target_w, target_h)

    # === SKALE ===
    scale_w = target_core_w / base_core_w
    scale_h = target_core_h / base_core_h

    # ==================================================
    # OBSZAR 1
    # ==================================================
    b = base_monitor

    state.monitor = {
        "top": int(b["top"] * scale_h),
        "left": int((b["left"] - base_offset) * scale_w + target_offset),
        "width": int(b["width"] * scale_w),
        "height": int(b["height"] * scale_h),
    }

    # ==================================================
    # OBSZAR 2 (JEŚLI ISTNIEJE)
    # ==================================================
    if base_monitor2:
        b2 = base_monitor2

        state.monitor2_top = int(b2["top"] * scale_h)
        state.monitor2_left = int((b2["left"] - base_offset) * scale_w + target_offset)
        state.monitor2_width = int(b2["width"] * scale_w)
        state.monitor2_height = int(b2["height"] * scale_h)

    # ==================================================
    # OCR - SKALOWANIE
    # ==================================================

    if has_preset:
        base_downscale = state.base_downscale_from_preset
        base_min_h = state.base_min_height_from_preset
        base_max_h = state.base_max_height_from_preset
        base_h_for_ocr = base_h
    else:
        base_downscale = state.runtime_base_downscale
        base_min_h = state.runtime_base_min_height
        base_max_h = state.runtime_base_max_height

        # == OBSZAR DO SKALOWANIA (PEŁNY / 16:9) ===
        bw, bh = SUPPORTED_RESOLUTIONS[state.runtime_base_resolution]
        base_h_for_ocr = bh

    # === SUROWE PRZELICZENIE DOWNSCALE'U ===
    raw_downscale = base_downscale * (base_h_for_ocr / target_h)

    state.RESOLUTION_DOWNSCALE = round(
        max(C.RESOLUTION_DOWNSCALE_MIN, min(raw_downscale, C.RESOLUTION_DOWNSCALE_MAX)),
        C.RESOLUTION_DOWNSCALE_DECIMALS,
    )

    # === SUROWE PRZELICZENIE MIN/MAX HEIGHT ===
    scale_h_ocr = target_h / base_h_for_ocr

    state.MIN_HEIGHT = max(
        C.MIN_HEIGHT_MIN,
        min(int(round(base_min_h * scale_h_ocr)), C.MIN_HEIGHT_MAX)
    )

    state.MAX_HEIGHT = max(
        C.MAX_HEIGHT_MIN,
        min(int(round(base_max_h * scale_h_ocr)), C.MAX_HEIGHT_MAX)
    )