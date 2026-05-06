
# ============================================================
# IMPORTY
# ============================================================
import ctypes
import threading
import time

from audio import player as audio_player
from audio import volume as audio
from core import debug
from core import state as config
from core.constants import ALLOWED_HOTKEYS_WHEN_READER_OFF
from ui.keymap import pretty_shortcut
from ui.overlay.hotkeys_overlay import HotkeysOverlay

# ============================================================
# GLOBALNY STAN HOTKEYÓW
# ============================================================
hotkeys_suspended = False
wait_for_key_release = False
_last_action_time = {}
ACTION_DEBOUNCE = 0.25
keyboard_polling_active = False
keyboard_polling_thread = None

# ============================================================
# PENDING OVERLAY (START / STOP LEKTORA)
# ============================================================
_pending_reader_action = None
_pending_reader_key = None
_pending_reader_ts = 0.0
PENDING_READER_TTL = 1.5

# ============================================================
# POMOCNICZE
# ============================================================
def switch_monitor(monitor_number):
    config.active_monitor = monitor_number

    sound_area1_path = audio_player.find_system_sound("area1")
    sound_area2_path = audio_player.find_system_sound("area2")

    if monitor_number == 1:
        audio_player.play_system_sound(sound_area1_path)
    elif monitor_number == 2:
        audio_player.play_system_sound(sound_area2_path)

def get_key_state(vk_code):
    return ctypes.windll.user32.GetAsyncKeyState(vk_code) & 0x8000 != 0

def get_active_window_title():
    hwnd = ctypes.windll.user32.GetForegroundWindow()
    length = ctypes.windll.user32.GetWindowTextLengthW(hwnd)
    if length == 0:
        return ""
    buff = ctypes.create_unicode_buffer(length + 1)
    ctypes.windll.user32.GetWindowTextW(hwnd, buff, length + 1)
    return buff.value

# ============================================================
# WĄTEK NASŁUCHU KLAWIATURY
# ============================================================
def keyboard_polling_worker():
    global keyboard_polling_active, wait_for_key_release

    # --- VK kody ---
    VK = {
        "CTRL": 0x11,
        "SHIFT": 0x10,
        "ALT": 0x12,

        "HOME": 0x24,
        "END": 0x23,
        "INSERT": 0x2D,
        "DELETE": 0x2E,
        "PAGE_UP": 0x21,
        "PAGE_DOWN": 0x22,
        "SPACE": 0x20,
        "ENTER": 0x0D,
        "BACKSPACE": 0x08,
        "TAB": 0x09,
        "ESC": 0x1B,

        "OEM_3": 0xC0,
        "OEM_4": 0xDB,
        "OEM_6": 0xDD,
        "OEM_1": 0xBA,
        "OEM_7": 0xDE,
        "OEM_COMMA": 0xBC,
        "OEM_PERIOD": 0xBE,
        "OEM_2": 0xBF,
        "OEM_5": 0xDC,
        "OEM_MINUS": 0xBD,
        "OEM_PLUS": 0xBB,
    }

    # === A-Z ===
    for i in range(26):
        VK[chr(ord("A") + i)] = 0x41 + i
    # === 0-9 ===
    for i in range(10):
        VK[str(i)] = 0x30 + i
    # === F1-F12 ===
    for i in range(1, 13):
        VK[f"F{i}"] = 0x70 + (i - 1)

    def normalize_key_name(name: str) -> str:
        n = (name or "").strip().lower()

        alias = {
            "control": "ctrl",
            "ctl": "ctrl",

            "pgup": "page_up",
            "pageup": "page_up",
            "pgdn": "page_down",
            "pagedown": "page_down",
            "del": "delete",
            "ins": "insert",
            "escape": "esc",

            "`": "oem_3",
            "grave": "oem_3",
            "quoteleft": "oem_3",

            "[": "oem_4",
            "]": "oem_6",
            ";": "oem_1",
            "'": "oem_7",
            ",": "oem_comma",
            ".": "oem_period",
            "/": "oem_2",
            "\\": "oem_5",
            "-": "oem_minus",
            "=": "oem_plus",
        }
        n = alias.get(n, n)

        if n in ("ctrl", "shift", "alt"):
            return n.upper()
        if n.startswith("f") and n[1:].isdigit():
            return n.upper()
        if len(n) == 1 and n.isalpha():
            return n.upper()
        if len(n) == 1 and n.isdigit():
            return n
        return n.upper()

    def parse_binding(binding: str):
        parts = [p.strip() for p in (binding or "").split("+") if p.strip()]
        if not parts:
            return None

        mods = set()
        key = None

        for p in parts:
            k = normalize_key_name(p)
            if k in ("CTRL", "SHIFT", "ALT"):
                mods.add(k)
            else:
                key = k

        if not key:
            return None

        vk = VK.get(key, 0)
        if vk == 0:
            return None

        return {"mods": mods, "key": key, "vk": vk}

    def is_down(vk_code: int) -> bool:
        return ctypes.windll.user32.GetAsyncKeyState(vk_code) & 0x8000 != 0

    bindings = []
    for action, binding in config.key_bindings.items():
        if not binding:
            continue
        parsed = parse_binding(binding)
        if not parsed:
            debug.log(debug.WARNING, "Hotkeys", f"Nieobsługiwany skrót: {action} = {binding}")
            continue
        bindings.append((action, parsed))

    last_key_state = {}
    for _, b in bindings:
        last_key_state[b["vk"]] = False

    try:
        while keyboard_polling_active:

            if hotkeys_suspended:
                for vk in last_key_state:
                    last_key_state[vk] = False
                time.sleep(0.05)
                continue

            if wait_for_key_release:
                if any(is_down(vk) for vk in last_key_state):
                    time.sleep(0.05)
                    continue
                wait_for_key_release = False

            current_mods = set()
            if is_down(VK["CTRL"]):
                current_mods.add("CTRL")
            if is_down(VK["SHIFT"]):
                current_mods.add("SHIFT")
            if is_down(VK["ALT"]):
                current_mods.add("ALT")

            for action, b in bindings:
                vk = b["vk"]
                down = is_down(vk)
                prev = last_key_state.get(vk, False)

                if down and not prev:
                    if current_mods == b["mods"]:
                        active_title = get_active_window_title()

                        # === BLOKADA SKRÓTÓW W OKNIE APLIKACJI ===
                        if not config.capture_enabled:
                            if action in ALLOWED_HOTKEYS_WHEN_READER_OFF:
                                execute_action(action)
                        else:
                            execute_action(action)

                        break

                last_key_state[vk] = down

            time.sleep(0.05)

    except Exception as e:
        debug.log(debug.ERROR, "Hotkeys", f"Błąd w wątku klawiatury: {e}")

# ============================================================
# OVERLAY – POMOCNICZE
# ============================================================
def _show_hotkey_overlay(action: str, text: str):
    seq = config.key_bindings.get(action)
    if not seq:
        return

    HotkeysOverlay.show_overlay(
        key=pretty_shortcut(seq),
        text=text
    )

def _arm_reader_overlay(action: str):
    global _pending_reader_action, _pending_reader_key, _pending_reader_ts

    seq = config.key_bindings.get(action)
    if not seq:
        return

    _pending_reader_action = action
    _pending_reader_key = pretty_shortcut(seq)
    _pending_reader_ts = time.time()

def on_reader_state_changed(enabled: bool):
    global _pending_reader_action, _pending_reader_key, _pending_reader_ts

    if not _pending_reader_action or not _pending_reader_key:
        return

    if time.time() - _pending_reader_ts > PENDING_READER_TTL:
        _pending_reader_action = None
        _pending_reader_key = None
        _pending_reader_ts = 0.0
        return

    if _pending_reader_action == "toggle_reader":
        if enabled:
            HotkeysOverlay.show_overlay(
                key=_pending_reader_key,
                text="Uruchomiono lektora"
            )
        else:
            HotkeysOverlay.show_overlay(
                key=_pending_reader_key,
                text="Zatrzymano lektora"
            )

    _pending_reader_action = None
    _pending_reader_key = None
    _pending_reader_ts = 0.0

# ============================================================
# WYKONANIE AKCJI
# ============================================================
def execute_action(action):
    if hotkeys_suspended:
        return
    
    global _last_action_time

    # === DEBOUNCE – ochrona przed spamem klawisza ===
    now = time.time()
    last = _last_action_time.get(action, 0)

    if now - last < ACTION_DEBOUNCE:
        return

    _last_action_time[action] = now

    # =====================================================
    # AKCJE
    # =====================================================

    # === LOKALNY IMPORT MUSI TU ZOSTAĆ ABY ZABEZPIECZYĆ CYKLE ===
    from core import app as core_app

    try:
        if action == 'toggle_reader':
            _arm_reader_overlay(action)
            if config.capture_enabled:
                core_app.ui_bridge.disable_reader.emit()
            else:
                core_app.ui_bridge.enable_reader.emit()

        elif action == 'volume_up':
            audio.increase_volume()
            value = audio.get_volume_percent()
            _show_hotkey_overlay(
                action,
                f"Zwiększono głośność lektora ({value}%)"
            )

        elif action == 'volume_down':
            audio.decrease_volume()
            value = audio.get_volume_percent()
            _show_hotkey_overlay(
                action,
                f"Zmniejszono głośność lektora ({value}%)"
            )

        elif action == 'switch_monitor_toggle':
            if not config.monitor2_enabled:
                _show_hotkey_overlay(action, "Obszar 2 nieaktywny")
                return

            new_area = 2 if config.active_monitor == 1 else 1
            switch_monitor(new_area)

            _show_hotkey_overlay(
                action,
                f"Przełączono obszary wykrywania – Aktywny obszar {new_area}"
            )

        elif action == 'test_sound':
            audio_player.play_test_sound()

        elif action == 'open_settings':
            core_app.ui_bridge.open_settings.emit()

        elif action == 'interrupt_audio':
            was_interrupted = audio_player.interrupt_audio_playback()

            if was_interrupted:
                seq = config.key_bindings.get(action)
                if seq:
                    HotkeysOverlay.show_overlay(
                        key=pretty_shortcut(seq),
                        text="Przerwano kwestię lektora"
                    )

        elif action == 'base_speed_up':
            audio.increase_base_speed()
            core_app.ui_bridge.reload_ui.emit()
            value = round(float(config.BASE_PLAYBACK_SPEED), 2)
            _show_hotkey_overlay(action, f"Prędkość lektora zwiększona ({value}x)")

        elif action == 'base_speed_down':
            audio.decrease_base_speed()
            core_app.ui_bridge.reload_ui.emit()
            value = round(float(config.BASE_PLAYBACK_SPEED), 2)
            _show_hotkey_overlay(action, f"Prędkość lektora zmniejszona ({value}x)")

        elif action == 'overlap_speed_up':
            audio.increase_overlap_speed()
            core_app.ui_bridge.reload_ui.emit()
            value = round(float(config.OVERLAP_PLAYBACK_SPEED), 2)
            _show_hotkey_overlay(action, f"Prędkość przyśpieszona zwiększona ({value}x)")

        elif action == 'overlap_speed_down':
            audio.decrease_overlap_speed()
            core_app.ui_bridge.reload_ui.emit()
            value = round(float(config.OVERLAP_PLAYBACK_SPEED), 2)
            _show_hotkey_overlay(action, f"Prędkość przyśpieszona zmniejszona ({value}x)")

        elif action == 'toggle_areas':
            was_enabled = bool(config.debug_enabled)

            core_app.ui_bridge.toggle_debug_overlay.emit()

            if was_enabled:
                _show_hotkey_overlay(action, "Ukryto obszary wykrywania")
            else:
                _show_hotkey_overlay(action, "Pokazano obszary wykrywania")

        elif action == 'debug_console':
            from core import app as core_app
            core_app.ui_bridge.toggle_debug_console.emit()

        else:
            debug.log(debug.WARNING, "Hotkeys", f"Nieznana akcja hotkey: {action}")

    except Exception as e:
        debug.log(debug.ERROR, "Hotkeys", f"Błąd wykonania akcji '{action}': {e}")

# ============================================================
# API PUBLICZNE
# ============================================================
def suspend():
    global hotkeys_suspended
    hotkeys_suspended = True

def resume():
    global hotkeys_suspended, wait_for_key_release
    hotkeys_suspended = False
    wait_for_key_release = True

def start_listener():
    global keyboard_polling_active, keyboard_polling_thread
    if keyboard_polling_active:
        return
    try:
        keyboard_polling_active = True
        keyboard_polling_thread = threading.Thread(target=keyboard_polling_worker, daemon=True)
        keyboard_polling_thread.start()
    except Exception as e:
        debug.log(debug.ERROR, "Hotkeys", f"Błąd start_listener: {e}")

def stop_listener():
    global keyboard_polling_active, keyboard_polling_thread
    try:
        keyboard_polling_active = False
        if keyboard_polling_thread and keyboard_polling_thread.is_alive():
            keyboard_polling_thread.join(timeout=1)
        keyboard_polling_thread = None
    except Exception as e:
        debug.log(debug.ERROR, "Hotkeys", f"Błąd stop_listener: {e}")
