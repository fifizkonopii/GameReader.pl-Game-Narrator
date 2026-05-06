
# ============================================================
# IMPORTY
# ============================================================
import os
import sys
import time
import threading
import pygame
from pycaw.pycaw import AudioUtilities, ISimpleAudioVolume

from core import state as config
from core import constants as C
from core import debug

# ============================================================
# STAN WEWNĘTRZNY
# ============================================================
_current_volume_percent: int | None = None

def _ensure_volume_initialized():
    global _current_volume_percent

    if _current_volume_percent is None:
        try:
            vol = pygame.mixer.music.get_volume()
            _current_volume_percent = int(round(vol * 100))
        except Exception:
            _current_volume_percent = 80

# ============================================================
# GŁOŚNOŚĆ LEKTORA (UI / HOTKEYS)
# ============================================================
def increase_volume():
    global _current_volume_percent
    _ensure_volume_initialized()

    volume_steps = list(range(0, 101, 5))

    try:
        idx = volume_steps.index(_current_volume_percent)
        new_percent = volume_steps[min(idx + 1, len(volume_steps) - 1)]
    except ValueError:
        new_percent = 5

    _current_volume_percent = new_percent
    pygame.mixer.music.set_volume(new_percent / 100.0)

    debug.log(debug.INFO, "Audio", f"Głośność zwiększona ({_current_volume_percent}%)")

def decrease_volume():
    global _current_volume_percent
    _ensure_volume_initialized()

    volume_steps = list(range(0, 101, 5))

    try:
        idx = volume_steps.index(_current_volume_percent)
        new_percent = volume_steps[max(idx - 1, 0)]
    except ValueError:
        new_percent = 0

    _current_volume_percent = new_percent
    pygame.mixer.music.set_volume(new_percent / 100.0)

    debug.log(debug.INFO, "Audio", f"Głośność zmniejszona ({_current_volume_percent}%)")

def get_volume_percent() -> int:
    _ensure_volume_initialized()
    return _current_volume_percent

# ============================================================
# PRĘDKOŚCI ODTWARZANIA (BASE / OVERLAP)
# ============================================================
def _clamp_round(value: float, min_v: float, max_v: float) -> float:
    v = round(float(value), C.SPEED_DECIMALS)
    v = max(min_v, min(v, max_v))
    return round(v, C.SPEED_DECIMALS)

def increase_base_speed():
    config.BASE_PLAYBACK_SPEED = _clamp_round(
        config.BASE_PLAYBACK_SPEED + C.SPEED_STEP,
        C.BASE_PLAYBACK_SPEED_MIN,
        C.BASE_PLAYBACK_SPEED_MAX
    )
    debug.log(debug.INFO, "Audio", f"Prędkość lektora zwiększona do ({config.BASE_PLAYBACK_SPEED}x)")


def decrease_base_speed():
    config.BASE_PLAYBACK_SPEED = _clamp_round(
        config.BASE_PLAYBACK_SPEED - C.SPEED_STEP,
        C.BASE_PLAYBACK_SPEED_MIN,
        C.BASE_PLAYBACK_SPEED_MAX
    )
    debug.log(debug.INFO, "Audio", f"Prędkość lektora zmniejszona ({config.BASE_PLAYBACK_SPEED}x)")


def increase_overlap_speed():
    config.OVERLAP_PLAYBACK_SPEED = _clamp_round(
        config.OVERLAP_PLAYBACK_SPEED + C.SPEED_STEP,
        C.OVERLAP_PLAYBACK_SPEED_MIN,
        C.OVERLAP_PLAYBACK_SPEED_MAX
    )
    debug.log(debug.INFO, "Audio", f"Prędkość przyspieszona lekotra zwiększona ({config.OVERLAP_PLAYBACK_SPEED}x)")


def decrease_overlap_speed():
    config.OVERLAP_PLAYBACK_SPEED = _clamp_round(
        config.OVERLAP_PLAYBACK_SPEED - C.SPEED_STEP,
        C.OVERLAP_PLAYBACK_SPEED_MIN,
        C.OVERLAP_PLAYBACK_SPEED_MAX
    )
    debug.log(debug.INFO, "Audio", f"Prędkość przyspieszona lektora zmniejszona ({config.OVERLAP_PLAYBACK_SPEED}x)")

# ============================================================
# SYSTEMOWA GŁOŚNOŚĆ GRY (DUCKING)
# ============================================================
def adjust_system_volume(reduction=None, steps=10, duration=None):
    if reduction is None:
        reduction = config.VOLUME_REDUCTION_LEVEL
    if duration is None:
        duration = config.VOLUME_FADE_DURATION
    target_volume = max(0.0, 1.0 - reduction)
    sessions = AudioUtilities.GetAllSessions()
    delay = duration / steps
    current_process_name = os.path.basename(sys.executable).lower()
    for i in range(1, steps + 1):
        for session in sessions:
            if session.Process and session.Process.name().lower() != current_process_name:
                volume = session._ctl.QueryInterface(ISimpleAudioVolume)
                current_vol = volume.GetMasterVolume()
                intermediate = current_vol + (target_volume - current_vol) * (i / steps)
                volume.SetMasterVolume(intermediate, None)
        time.sleep(delay)

def restore_system_volume(steps=10, duration=None):
    if duration is None:
        duration = config.VOLUME_FADE_DURATION
    sessions = AudioUtilities.GetAllSessions()
    delay = duration / steps
    current_process_name = os.path.basename(sys.executable).lower()
    for i in range(1, steps + 1):
        for session in sessions:
            if session.Process and session.Process.name().lower() != current_process_name:
                volume = session._ctl.QueryInterface(ISimpleAudioVolume)
                current_vol = volume.GetMasterVolume()
                intermediate = current_vol + (1.0 - current_vol) * (i / steps)
                volume.SetMasterVolume(intermediate, None)
        time.sleep(delay)

def fade_game_volume_if_needed(audio_file: str) -> bool:
    if "output1" in audio_file.lower() or "output2" in audio_file.lower():
        threading.Thread(target=adjust_system_volume, args=(config.VOLUME_REDUCTION_LEVEL,), daemon=True).start()
        return True
    return False

def restore_game_volume_async():
    threading.Thread(target=restore_system_volume, daemon=True).start()
