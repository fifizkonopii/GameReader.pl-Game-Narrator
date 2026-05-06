
# ============================================================
# IMPORTY
# ============================================================
import os
import io
import sys
import subprocess
from collections import OrderedDict
import numpy as np
import pygame

from core import state as config
from core import paths
from core import debug

# ============================================================
# CACHE PRZETWORZONEGO AUDIO
# ============================================================
# LRU cache: klucz = (ścieżka_pliku, prędkość_zaokrąglona_do_2_miejsc)
# Wartość = bytes WAV gotowy do załadowania przez pygame.mixer.Sound
_speed_cache: OrderedDict = OrderedDict()
_SPEED_CACHE_MAX = 15  # maks. liczba wpisów w cache (WAV może być duży — trzymamy mniej)

def _cache_key(audio_file: str, speed: float) -> tuple:
    return (audio_file, round(speed, 2))

def _cache_get(audio_file: str, speed: float):
    key = _cache_key(audio_file, speed)
    if key in _speed_cache:
        _speed_cache.move_to_end(key)  # LRU: ostatnio użyty na końcu
        return _speed_cache[key]
    return None

def _cache_put(audio_file: str, speed: float, data: bytes):
    key = _cache_key(audio_file, speed)
    _speed_cache[key] = data
    _speed_cache.move_to_end(key)
    if len(_speed_cache) > _SPEED_CACHE_MAX:
        _speed_cache.popitem(last=False)  # usuń najstarszy

# ============================================================
# HELPER: SUPPRESS CONSOLE WINDOW
# ============================================================
class SuppressConsole:
    def __enter__(self):
        if sys.platform != "win32":
            return
        self.original_popen = subprocess.Popen
        
        def patched_popen(*args, **kwargs):
            if "startupinfo" not in kwargs:
                si = subprocess.STARTUPINFO()
                si.dwFlags |= subprocess.STARTF_USESHOWWINDOW
                si.wShowWindow = subprocess.SW_HIDE
                kwargs["startupinfo"] = si
            return self.original_popen(*args, **kwargs)
            
        subprocess.Popen = patched_popen

    def __exit__(self, exc_type, exc_val, exc_tb):
        if sys.platform != "win32":
            return
        subprocess.Popen = self.original_popen

# ============================================================
# FFMPEG / ŚRODOWISKO
# ============================================================
def _configure_ffmpeg():
    ffmpeg_path = paths.FFMPEG_PATH
    ffprobe_path = paths.FFPROBE_PATH

    if os.path.isfile(ffmpeg_path):
        os.environ["FFMPEG_BINARY"] = ffmpeg_path
        bin_dir = os.path.dirname(ffmpeg_path)
        if bin_dir not in os.environ.get("PATH", ""):
            os.environ["PATH"] += os.pathsep + bin_dir

    if os.path.isfile(ffprobe_path):
        os.environ["FFPROBE_BINARY"] = ffprobe_path

# ============================================================
# DETEKCJA DYNAMICZNEJ PRĘDKOŚCI
# ============================================================
def detect_dynamic_speed_availability():
    _configure_ffmpeg()

    try:
        # === IMPORT MUSI TU ZOSTAĆ - TESTOWANIE DOSTĘPNOŚCI ===
        import pygame.sndarray
        with SuppressConsole():
            from pydub import AudioSegment
            from pydub.effects import speedup

        config.DYNAMIC_SPEED_AVAILABLE = True
        config.PYDUB_AVAILABLE = True

    except Exception:
        try:
            # === IMPORT MUSI TU ZOSTAĆ - TESTOWANIE DOSTĘPNOŚCI ===
            import pygame.sndarray
            config.DYNAMIC_SPEED_AVAILABLE = True
            config.PYDUB_AVAILABLE = False
            debug.log(debug.INFO, "Audio", "System dynamicznej prędkości dostępny (tryb podstawowy)")
        except Exception:
            config.DYNAMIC_SPEED_AVAILABLE = False
            config.PYDUB_AVAILABLE = False

def simple_overlap_add_stretch(audio_data, stretch_factor, frame_size=2048, overlap_ratio=0.25):
    if abs(stretch_factor - 1.0) < 0.01:
        return audio_data

    is_stereo = len(audio_data.shape) == 2
    if is_stereo:
        left_processed = simple_overlap_add_stretch(audio_data[:, 0], stretch_factor, frame_size, overlap_ratio)
        right_processed = simple_overlap_add_stretch(audio_data[:, 1], stretch_factor, frame_size, overlap_ratio)
        return np.column_stack((left_processed, right_processed))

    input_length = len(audio_data)
    hop_analysis = int(frame_size * (1 - overlap_ratio))
    hop_synthesis = int(hop_analysis / stretch_factor)

    output_length = int(input_length * stretch_factor) + frame_size
    output = np.zeros(output_length, dtype=np.float32)

    window = np.hanning(frame_size).astype(np.float32)

    input_pos = 0
    output_pos = 0

    while input_pos + frame_size <= input_length:
        frame = audio_data[input_pos:input_pos + frame_size].astype(np.float32)
        windowed_frame = frame * window

        end_pos = min(output_pos + frame_size, len(output))
        actual_frame_size = end_pos - output_pos

        if actual_frame_size > 0:
            output[output_pos:end_pos] += windowed_frame[:actual_frame_size]

        input_pos += hop_analysis
        output_pos += hop_synthesis

    expected_length = int(input_length * stretch_factor)
    if len(output) > expected_length:
        output = output[:expected_length]

    max_val = np.max(np.abs(output))
    if max_val > 0.95:
        output = output * (0.95 / max_val)

    return output

# ============================================================
# ŁADOWANIE AUDIO + ZMIANA PRĘDKOŚCI
# ============================================================
def load_and_change_speed(audio_file, speed=1.0):
    if not config.DYNAMIC_SPEED_AVAILABLE:
        try:
            return pygame.mixer.Sound(audio_file)
        except Exception:
            return None

    try:
        if abs(speed - 1.0) < 0.01:
            return pygame.mixer.Sound(audio_file)

        # === CACHE: jeśli ten plik przy tej prędkości był już przetworzony → instant load ===
        cached = _cache_get(audio_file, speed)
        if cached is not None:
            debug.log(debug.DEBUG, "Audio", f"Cache audio: ({speed}x) {os.path.basename(audio_file)}")
            return pygame.mixer.Sound(io.BytesIO(cached))

        if config.PYDUB_AVAILABLE:
            # === IMPORT MUSI TU ZOSTAĆ - OPTYMALIZACJA ===
            from pydub import AudioSegment
            debug.log(debug.DEBUG, "Audio", f"Zmiana prędkości audio: ({speed}x) (pydub + ffmpeg)")
            audio_segment = None
            buffer = None
            try:
                with SuppressConsole():
                    audio_segment = AudioSegment.from_file(audio_file)
                    buffer = io.BytesIO()

                    atempo_value = speed
                    atempo_filters = []
                    while atempo_value < 0.5:
                        atempo_filters.append("atempo=0.5")
                        atempo_value /= 0.5
                    while atempo_value > 2.0:
                        atempo_filters.append("atempo=2.0")
                        atempo_value /= 2.0
                    if abs(atempo_value - 1.0) > 0.01:
                        atempo_filters.append(f"atempo={atempo_value:.3f}")

                    if atempo_filters:
                        filter_chain = ",".join(atempo_filters)
                        audio_segment.export(buffer, format="wav", parameters=["-af", filter_chain])
                    else:
                        audio_segment.export(buffer, format="wav")

                wav_bytes = buffer.getvalue()
                _cache_put(audio_file, speed, wav_bytes)
                buffer.seek(0)
                new_sound = pygame.mixer.Sound(buffer)
                buffer.close()
                del buffer, audio_segment
                return new_sound
            except Exception as e:
                debug.log(
                    debug.WARNING, "Audio", f"Błąd pydub/ffmpeg – fallback na oryginalny plik\n({e})")
                if buffer:
                    try:
                        buffer.close()
                    except Exception:
                        pass
                if audio_segment:
                    del audio_segment
                try:
                    return pygame.mixer.Sound(audio_file)
                except Exception:
                    return None
        else:
            debug.log(debug.DEBUG, "Audio", f"Zmiana prędkości audio: ({speed}x) (podstawowy - własny algorytm)")
            sound = pygame.mixer.Sound(audio_file)
            sound_array = pygame.sndarray.array(sound)

            if sound_array.dtype == np.int16:
                sound_array_float = sound_array.astype(np.float32) / 32768.0
            else:
                sound_array_float = sound_array.astype(np.float32)

            stretch_factor = 1.0 / speed
            processed_array = simple_overlap_add_stretch(sound_array_float, stretch_factor)

            if np.max(np.abs(processed_array)) > 1.0:
                processed_array = processed_array / np.max(np.abs(processed_array)) * 0.95

            processed_int16 = (processed_array * 32767).astype(np.int16)
            new_sound = pygame.sndarray.make_sound(processed_int16)
            return new_sound

    except ImportError as e:
        debug.log(debug.ERROR, "Audio", f"Brak wymaganej biblioteki audio:\n{e}")
        try:
            return pygame.mixer.Sound(audio_file)
        except Exception:
            return None
    except Exception as e:
        debug.log(debug.ERROR, "Audio", f"Błąd podczas zmiany prędkości audio:\n{e}")
        try:
            return pygame.mixer.Sound(audio_file)
        except Exception:
            return None
