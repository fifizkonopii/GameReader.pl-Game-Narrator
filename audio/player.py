
# ============================================================
# IMPORTY
# ============================================================
import os
import time
import gc

import pygame

from core import state as config
from core import debug

from audio import speed
from audio import volume as vol

# ============================================================
# INICJALIZACJA MIXERA
# ============================================================
pygame.mixer.init(frequency=44100, size=-16, channels=2, buffer=512)
pygame.mixer.music.set_volume(0.8)

# === DETEKCJA DOSTĘPNOŚCI DYNAMICZNEJ PRĘDKOŚC ===
speed.detect_dynamic_speed_availability()

# ============================================================
# WYSZUKIWANIE PLIKÓW AUDIO
# ============================================================
def find_audio_file(audio_dir, base_filename):
    if not audio_dir or not os.path.exists(audio_dir):
        return None
    for ext in config.SUPPORTED_AUDIO_FORMATS:
        file_path = os.path.join(audio_dir, f"{base_filename}{ext}")
        if os.path.exists(file_path):
            debug.log(debug.DEBUG, "Audio", f"Znaleziono plik audio: {os.path.basename(file_path)}")
            return file_path
    debug.log(
        debug.WARNING,
        "Audio",
        f"Brak pliku audio dla {base_filename} ({', '.join(config.SUPPORTED_AUDIO_FORMATS)})"
    )
    return None


def find_system_sound(sound_name):
    for ext in config.SUPPORTED_AUDIO_FORMATS:
        sound_path = os.path.join(config.sounds_dir, f"{sound_name}{ext}")
        if os.path.exists(sound_path):
            return sound_path
    return os.path.join(config.sounds_dir, f"{sound_name}.ogg")


def list_available_audio_files(audio_dir):
    if not audio_dir or not os.path.exists(audio_dir):
        debug.log(debug.WARNING, "Audio", "Katalog audio nie istnieje lub nie został ustawiony")
        return
    audio_files = []
    for file in os.listdir(audio_dir):
        if any(file.lower().endswith(ext) for ext in config.SUPPORTED_AUDIO_FORMATS):
            audio_files.append(file)
    if audio_files:
        debug.log(debug.INFO, "Audio", f"Znaleziono {len(audio_files)} plików audio w katalogu")
    else:
        debug.log(debug.WARNING, "Audio", "Nie znaleziono plików audio w obsługiwanych formatach")

# ============================================================
# DŹWIĘKI SYSTEMOWE
# ============================================================
def play_system_sound(sound_path):
    try:
        sound_obj = pygame.mixer.Sound(sound_path)
        current_master_volume = pygame.mixer.music.get_volume()
        sound_obj.set_volume(current_master_volume)
        sound_obj.play()
    except Exception as e:
        debug.log(debug.ERROR, "Audio", f"Błąd odtwarzania dźwięku systemowego:\n{e}")


def play_test_sound():
    try:
        sound_test_path = find_system_sound("test")
        pygame.mixer.music.load(sound_test_path)
        pygame.mixer.music.play()
    except Exception as e:
        debug.log(debug.ERROR, "Audio", f"Błąd odtwarzania pliku testowego:\n{e}")


# ============================================================
# PRZERWANIE ODTWARZANIA
# ============================================================
def interrupt_audio_playback() -> bool:
    interrupted = False

    try:
        if pygame.mixer.music.get_busy():
            pygame.mixer.music.stop()
            debug.log(debug.INFO, "Audio", "Przerwano aktualnie odtwarzany dźwięk")
            interrupted = True

        if not config.audio_queue.empty():
            next_audio = config.audio_queue.get_nowait()
            if next_audio:
                if isinstance(next_audio, tuple):
                    next_file, _speed = next_audio
                else:
                    next_file, _speed = next_audio, 1.0

                pygame.mixer.music.load(next_file)
                pygame.mixer.music.play(fade_ms=50)
                debug.log(debug.INFO, "Audio", f"Odtwarzam następny plik audio: {os.path.basename(next_file)}")
                interrupted = True

            config.audio_queue.task_done()

    except Exception as e:
        debug.log(debug.ERROR, "Audio", f"Błąd podczas przerywania odtwarzania:\n{e}")
    return interrupted

# ============================================================
# WĄTEK ODTWARZANIA AUDIO
# ============================================================
def audio_player():
    current_volume = 0.8
    _audio_gc_counter = 0

    while True:
        audio_data = config.audio_queue.get()
        if audio_data is None:
            break

        if isinstance(audio_data, tuple):
            audio_file, playback_speed = audio_data
        else:
            audio_file = audio_data
            playback_speed = 1.0

        should_restore = False
        sound_object = None

        try:
            with config.audio_playing_lock:
                config.is_audio_playing = True

            if pygame.mixer.music.get_busy():
                pygame.mixer.music.fadeout(50)
                time.sleep(0.05)

            pygame.mixer.stop()
            current_volume = pygame.mixer.music.get_volume()

            debug.log(debug.INFO, "Audio", f"Odtwarzanie pliku audio: {os.path.basename(audio_file)}")

            if config.ENABLE_DYNAMIC_SPEED and config.DYNAMIC_SPEED_AVAILABLE:
                debug.log(
                    debug.DEBUG,
                    "Audio",
                    f"Prędkość odtwarzania: {playback_speed}x ({'pydub' if config.PYDUB_AVAILABLE else 'podstawowy'})"
                )
                sound_object = speed.load_and_change_speed(audio_file, playback_speed)

                if sound_object:
                    sound_object.set_volume(current_volume)
                    should_restore = vol.fade_game_volume_if_needed(audio_file)

                    channel = sound_object.play()
                    if channel:
                        while channel.get_busy():
                            time.sleep(0.1)
                else:
                    debug.log(
                        debug.WARNING,
                        "Audio",
                        "Błąd ładowania dźwięku – fallback na standardowe odtwarzanie"
                    )
                    pygame.mixer.music.load(audio_file)
                    should_restore = vol.fade_game_volume_if_needed(audio_file)
                    pygame.mixer.music.play(fade_ms=50)
                    while pygame.mixer.music.get_busy():
                        time.sleep(0.1)
            else:
                pygame.mixer.music.load(audio_file)
                should_restore = vol.fade_game_volume_if_needed(audio_file)
                pygame.mixer.music.play(fade_ms=50)
                while pygame.mixer.music.get_busy():
                    time.sleep(0.1)

        except pygame.error as e:
            debug.log(debug.ERROR, "Audio", f"Błąd odtwarzania pliku audio:\n{e}")
        except Exception as e:
            debug.log(debug.ERROR, "Audio", f"Nieoczekiwany błąd podczas odtwarzania:\n{e}")
            # === IMPORT MUSI TU ZOSTAĆ - OPTYMALIZACJA ===
            import traceback
            traceback.print_exc()
        finally:
            with config.audio_playing_lock:
                config.is_audio_playing = False

            if should_restore:
                vol.restore_game_volume_async()

            if sound_object:
                del sound_object

            _audio_gc_counter += 1
            if _audio_gc_counter >= 10:
                gc.collect(0)
                _audio_gc_counter = 0

            config.audio_queue.task_done()