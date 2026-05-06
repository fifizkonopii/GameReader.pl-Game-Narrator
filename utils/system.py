
# ============================================================
# IMPORTY
# ============================================================
import os
import sys
import psutil
import tempfile
import atexit

from core import state as config


# ============================================================
# ZARZĄDZANIE PROCESEM
# ============================================================
def check_single_instance() -> bool:
    lock_file = os.path.join(
        tempfile.gettempdir(),
        "gamereader_beta.lock"
    )

    try:
        # === JEŚLI LOCK ISTNIEJE → SPRAWDŹ STARY PID ===
        if os.path.exists(lock_file):
            try:
                with open(lock_file, "r") as f:
                    old_pid = int(f.read().strip())

                try:
                    process = psutil.Process(old_pid)
                    if process.is_running() and "python" in process.name().lower():
                        print("Program GameReader jest już uruchomiony.")
                        print("Zamknij istniejącą instancję przed uruchomieniem nowej.")
                        input("Naciśnij Enter aby zamknąć...")
                        sys.exit(1)

                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    os.unlink(lock_file)

            except (ValueError, OSError):
                try:
                    os.unlink(lock_file)
                except OSError:
                    pass

        # === ZAPIS BIEŻĄCEGO PID ===
        with open(lock_file, "w") as f:
            f.write(str(os.getpid()))

        # === CLEANUP PRZY ZAMYKANIU APLIKACJI ===
        def _cleanup_lock_file():
            try:
                if os.path.exists(lock_file):
                    os.unlink(lock_file)
            except OSError:
                pass

        atexit.register(_cleanup_lock_file)
        return True

    except Exception as e:
        print(f"Błąd sprawdzania instancji: {e}")
        return True


def set_low_priority():
    try:
        process = psutil.Process(os.getpid())

        if hasattr(psutil, "BELOW_NORMAL_PRIORITY_CLASS"):
            process.nice(psutil.BELOW_NORMAL_PRIORITY_CLASS)

    except Exception as e:
        try:
            print(f"Nie udało się ustawić niższego priorytetu: {e}")
        except Exception:
            pass


# ============================================================
# SKRÓTY KLAWISZOWE
# ============================================================
def merge_key_bindings(preset_bindings: dict | None) -> dict:
    bindings = config.DEFAULT_KEY_BINDINGS.copy()

    if preset_bindings:
        pb = preset_bindings.copy()

        # === MIGRACJA: TOGGLE_ON -> TOGGLE_READER ===
        if "toggle_reader" not in pb and "toggle_on" in pb:
            pb["toggle_reader"] = pb["toggle_on"]

        # === USUWANIE LEGACY KLUCZY ===
        pb.pop("toggle_on", None)
        pb.pop("toggle_off", None)

        bindings.update(pb)

    return bindings