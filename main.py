
# ============================================================
# OPTYMALIZACJA CPU / ENV / RAM
# ============================================================
import os
import gc

gc.set_threshold(700, 10, 10)

os.environ["OMP_NUM_THREADS"] = "3"
os.environ["OPENBLAS_NUM_THREADS"] = "3"
os.environ["MKL_NUM_THREADS"] = "3"
os.environ["VECLIB_MAXIMUM_THREADS"] = "3"
os.environ["NUMEXPR_NUM_THREADS"] = "3"
os.environ["PYGAME_HIDE_SUPPORT_PROMPT"] = "1"

# ============================================================
# CZYSZCZENIE WARNINGÓW
# ============================================================
import warnings
from PySide6.QtCore import qInstallMessageHandler, QtMsgType
warnings.filterwarnings("ignore", message=".*pin_memory.*")
warnings.filterwarnings("ignore", message=".*Setuptools<81*")

# === FUNKCJA DO CZYSZCZENIA SPAMU QT ===
def qt_message_handler(msg_type, context, message):
    if msg_type == QtMsgType.QtWarningMsg:
        if "QFont::setPointSize" in message:
            return
        if "Unable to set geometry" in message:
            return
    print(message)
qInstallMessageHandler(qt_message_handler)

# ============================================================
# IMPORTY
# ============================================================
import sys

import torch
try:
    torch.set_num_threads(3)
    torch.set_num_interop_threads(3)
except Exception:
    pass

from PySide6.QtGui import QFontDatabase, QFont
from PySide6.QtWidgets import QApplication

from core import debug
from utils.system import check_single_instance, set_low_priority
from core.paths import asset_path
from ui.main_window import MainWindow
from ui.theme.theme import FONT_BASE


def main():
    check_single_instance()
    debug.log(debug.INFO, "Core", "Uruchomiono aplikację")

    # === OCR PARALLEL MODE ===
    if "--ocr-parallel" in sys.argv:
        try:
            idx = sys.argv.index("--ocr-parallel")
            paths_arg = sys.argv[idx + 1] if len(sys.argv) > idx + 1 else ""
            paths = [p for p in paths_arg.split(",") if p]
            if not paths:
                debug.log(debug.ERROR, "Core", "No paths provided for --ocr-parallel")
                sys.exit(1)
            debug.log(debug.INFO, "Core", f"Starting parallel OCR on {len(paths)} images")
            from ocr.parallel import process_images
            results = process_images(paths, langs=["en"], gpu=False, num_processes=4, threads_per_process=3)
            for p, res in zip(paths, results):
                debug.log(debug.INFO, "OCR", f"{p}: {res}")
            sys.exit(0)
        except Exception as e:
            debug.log(debug.ERROR, "Core", f"Parallel OCR failed: {e}")
            sys.exit(1)

    app = QApplication(sys.argv)
    app.setQuitOnLastWindowClosed(True)

    # === WCZYTANIE CZCIONKI: NUNITO ===
    font_id = QFontDatabase.addApplicationFont(
        asset_path("fonts", "Nunito-Regular.ttf")
    )

    if font_id != -1:
        family = QFontDatabase.applicationFontFamilies(font_id)[0]
        app.setFont(QFont(family, FONT_BASE))
    else:
        debug.log(debug.ERROR, "Core", "Nie udało się załadować czcionki Nunito")

    window = MainWindow()
    window.show()

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
