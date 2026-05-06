
# ============================================================
# IMPORTY
# ============================================================
import os
import sys
import time

import cv2
import easyocr
import torch
import mss
import numpy as np
from PIL import Image, ImageDraw

from core import debug
from core import state as config
from .filters import filter_centered_text

_ocr_rect_logged_once = False

# ============================================================
# INICJALIZACJA OCR (EasyOCR)
# ===========================================================
_old_stdout = sys.stdout
sys.stdout = open(os.devnull, 'w')
try:
    try:
        torch.set_num_threads(2)
        torch.set_num_interop_threads(2)
        torch.set_grad_enabled(False)  # wyłącz gradienty — model tylko czyta, nie trenuje
    except Exception:
        pass
    reader = easyocr.Reader(
        ['pl'],
        gpu=False,
        quantize=True,
        model_storage_directory=config.easyocr_dir,
        user_network_directory=config.easyocr_dir,
        download_enabled=True,
        verbose=False
    )
finally:
    sys.stdout.close()
    sys.stdout = _old_stdout

# Zwolnij pamięć po inicjalizacji modelu (PyTorch zostawia sporo garbage'u)
import gc as _gc
_gc.collect()
try:
    torch.cuda.empty_cache()
except Exception:
    pass

# ============================================================
# OPTIMIZATION: GLOBALS
# ============================================================
_sct = None
_last_frame = None
_last_text = ""
_last_signature = None

# ============================================================
# PREPROCESSING OCR
# ============================================================
_clahe = cv2.createCLAHE(clipLimit=2.0, tileGridSize=(8, 8))
_sharpen_kernel = np.array([[0, -1, 0], [-1, 5, -1], [0, -1, 0]])
# Kernel erozji: usuwa glow/bloom wokół liter (3x3 = ostrożny, nie niszczy cienkich liter)
_erode_kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3))

def _preprocess_for_ocr(img_bgr: np.ndarray, gray_img: np.ndarray, scale: float = 1.0) -> np.ndarray:
    """Poprawia czytelność napisów przed OCR.
    Przy scale < 0.7: zwraca standardowy gray_img (jak oryginalnie — EasyOCR radzi sobie sam).
    Przy scale >= 0.7: max-channel + erozja glow + CLAHE + wyostrzenie."""
    if scale < 0.7:
        return gray_img
    # Max z kanałów B, G, R: cyan/biały/żółty tekst → 255, ciemne tło → 0
    gray = np.max(img_bgr[:, :, :3], axis=2).astype(np.uint8)
    # Erozja: usuwa glow/bloom wokół liter
    gray = cv2.erode(gray, _erode_kernel, iterations=1)
    gray = _clahe.apply(gray)
    gray = cv2.filter2D(gray, -1, _sharpen_kernel)
    return gray

# ============================================================
# CAPTURE + OCR
# ============================================================
def capture_and_extract_text(monitor):
    global _last_frame, _last_text, _sct, _last_signature

    try:
        if _sct is None:
            _sct = mss.mss()

        # === DOBIERAMY MONITOR MSS WG WYBORU W GUI (Qt->MSS mapping) ===
        mss_monitor = config.selected_mss_monitor_rect
        if not mss_monitor:
            mss_monitor = _sct.monitors[1]

        adjusted_monitor = {
            "left": int(mss_monitor["left"] + monitor["left"]),
            "top": int(mss_monitor["top"] + monitor["top"]),
            "width": int(monitor["width"]),
            "height": int(monitor["height"]),
        }

        # === RESET CACHE PO ZMIANIE OBSZARU / MONITORA ===
        sig = (
            adjusted_monitor["left"],
            adjusted_monitor["top"],
            adjusted_monitor["width"],
            adjusted_monitor["height"],
        )

        if _last_signature != sig:
            _last_signature = sig
            _last_frame = None
            _last_text = ""

        screenshot = _sct.grab(adjusted_monitor)

        img = np.array(screenshot)
        gray_img = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)

        if config.ENABLE_SCREENSHOTS and config.screenshot_dir:
            timestamp = time.strftime("%Y%m%d_%H%M%S")
            screenshot_path = os.path.join(config.screenshot_dir, f"screenshot_{timestamp}.png")

            rgb_img = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
            pil_img = Image.fromarray(rgb_img).convert("RGBA")

            overlay_img = Image.new("RGBA", pil_img.size, (0, 0, 0, 0))
            draw = ImageDraw.Draw(overlay_img)
            width, height = pil_img.size
            margin = int(config.CENTER_LINE_MARGIN)

            if config.USE_CENTER_LINE_1:
                center_x = width // 2
                left = center_x - margin // 2
                right = center_x + margin // 2
                draw.rectangle([left, 0, right, height], fill=(0, 0, 255, 80))

            if config.USE_CENTER_LINE_2:
                left = int(config.CENTER_LINE_2_START)
                right = left + margin
                draw.rectangle([left, 0, right, height], fill=(0, 0, 255, 80))

            if config.USE_CENTER_LINE_3:
                left = int(width * config.CENTER_LINE_3_START_RATIO)
                right = left + margin
                draw.rectangle([left, 0, right, height], fill=(0, 0, 255, 80))

            combined = Image.alpha_composite(pil_img, overlay_img)
            combined.save(screenshot_path)
            debug.log(debug.INFO, "OCR", f"Zapisano zrzut ekranu: {screenshot_path}")

        scale = config.RESOLUTION_DOWNSCALE
        new_w = int(gray_img.shape[1] * scale)
        new_h = int(gray_img.shape[0] * scale)
        gray_img = cv2.resize(gray_img, (new_w, new_h), interpolation=cv2.INTER_AREA)
        color_img = cv2.resize(img, (new_w, new_h), interpolation=cv2.INTER_AREA)

        # === OPTIMIZATION: FRAME COMPARISON ===
        if _last_frame is not None and _last_frame.shape == gray_img.shape:
            score = np.sum(cv2.absdiff(gray_img, _last_frame)) / gray_img.size
            if score < config.FRAME_DIFFERENCE_THRESHOLD:
                return _last_text

        _last_frame = gray_img

        ocr_input = _preprocess_for_ocr(color_img, gray_img, scale)
        del color_img, img  # zwolnij duże bufory przed OCR
        result = reader.readtext(ocr_input, low_text=0.4)
        del ocr_input
        result = sorted(result, key=lambda x: (x[0][0][1], x[0][0][0]))

        grouped_lines = []
        line_threshold = int(config.LINE_THRESHOLD * scale)

        for bbox, text, prob in result:
            # === FILTR PEWNOŚCI OCR (tylko tryb maszyny do pisania) ===
            if config.ENABLE_TYPEWRITER_WAIT and prob < config.OCR_MIN_CONFIDENCE:
                debug.log(debug.DEBUG, "OCR", f"Odrzucono niepewny tekst '{text}' (prob={prob:.2f})")
                continue

            top_left = tuple(bbox[0])
            bottom_right = tuple(bbox[2])

            height_scaled = bottom_right[1] - top_left[1]
            height_original = height_scaled / scale
            if height_original > config.MAX_HEIGHT or height_original < config.MIN_HEIGHT:
                debug.log(debug.DEBUG, "OCR", f"Ignorowany tekst '{text}' (wysokość={height_original:.2f}px)")
                continue

            mid_y = (top_left[1] + bottom_right[1]) / 2

            added_to_line = False
            for line in grouped_lines:
                if abs(mid_y - line['y_mean']) < line_threshold:
                    line['elements'].append((bbox, text))
                    line['y_mean'] = np.mean([line['y_mean'], mid_y])
                    added_to_line = True
                    break
            if not added_to_line:
                grouped_lines.append({'y_mean': mid_y, 'elements': [(bbox, text)]})

        for line in grouped_lines:
            line['elements'] = sorted(line['elements'], key=lambda x: x[0][0][0])

        screen_width = gray_img.shape[1]
        screen_height = gray_img.shape[0]
        grouped_lines = filter_centered_text(grouped_lines, screen_width, screen_height)

        extracted_text = "\n".join(
            " ".join(text for _, text in line['elements']) for line in sorted(grouped_lines, key=lambda x: x['y_mean'])
        )

        extracted_text = extracted_text.strip()
        _last_text = extracted_text
        return extracted_text

    except Exception as e:
        debug.log(debug.ERROR, "OCR", f"Błąd w capture_and_extract_text: {e}")
        return ""


# ============================================================
# CAPTURE + OCR: PARAGRAPHS (wykrywanie wielu dialogów naraz)
# ============================================================
_last_paragraphs: list = []

def capture_and_extract_paragraphs(monitor) -> list:
    """Jak capture_and_extract_text, ale zwraca listę stringów –
    każdy string to osobna wizualna grupa tekstu (osobny dialog).
    Grupy wyznaczane są na podstawie dużej przerwy pionowej między
    wierszami (paragraf = klaster linii bliskich siebie w osi Y)."""
    global _last_frame, _last_text, _last_paragraphs, _sct, _last_signature

    try:
        if _sct is None:
            _sct = mss.mss()

        mss_monitor = config.selected_mss_monitor_rect
        if not mss_monitor:
            mss_monitor = _sct.monitors[1]

        adjusted_monitor = {
            "left": int(mss_monitor["left"] + monitor["left"]),
            "top":  int(mss_monitor["top"]  + monitor["top"]),
            "width":  int(monitor["width"]),
            "height": int(monitor["height"]),
        }

        sig = (
            adjusted_monitor["left"],
            adjusted_monitor["top"],
            adjusted_monitor["width"],
            adjusted_monitor["height"],
        )

        if _last_signature != sig:
            _last_signature = sig
            _last_frame = None
            _last_text = ""
            _last_paragraphs = []

        screenshot = _sct.grab(adjusted_monitor)
        img = np.array(screenshot)
        gray_img = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)

        scale = config.RESOLUTION_DOWNSCALE
        new_w = int(gray_img.shape[1] * scale)
        new_h = int(gray_img.shape[0] * scale)
        gray_img = cv2.resize(gray_img, (new_w, new_h), interpolation=cv2.INTER_AREA)
        color_img = cv2.resize(img, (new_w, new_h), interpolation=cv2.INTER_AREA)

        # === FRAME DIFF: jeśli obraz nie zmienił się -> zwróć cache ===
        if _last_frame is not None and _last_frame.shape == gray_img.shape:
            score = np.sum(cv2.absdiff(gray_img, _last_frame)) / gray_img.size
            if score < config.FRAME_DIFFERENCE_THRESHOLD:
                return _last_paragraphs

        _last_frame = gray_img

        ocr_input = _preprocess_for_ocr(color_img, gray_img, scale)
        del color_img, img  # zwolnij duże bufory przed OCR
        result = reader.readtext(ocr_input, low_text=0.4)
        del ocr_input
        result = sorted(result, key=lambda x: (x[0][0][1], x[0][0][0]))

        grouped_lines = []
        line_threshold = int(config.LINE_THRESHOLD * scale)

        for bbox, text, prob in result:
            # === FILTR PEWNOŚCI OCR (tylko tryb maszyny do pisania) ===
            if config.ENABLE_TYPEWRITER_WAIT and prob < config.OCR_MIN_CONFIDENCE:
                debug.log(debug.DEBUG, "OCR", f"Odrzucono niepewny tekst '{text}' (prob={prob:.2f})")
                continue

            top_left    = tuple(bbox[0])
            bottom_right = tuple(bbox[2])

            height_scaled   = bottom_right[1] - top_left[1]
            height_original = height_scaled / scale
            if height_original > config.MAX_HEIGHT or height_original < config.MIN_HEIGHT:
                debug.log(debug.DEBUG, "OCR", f"Ignorowany tekst '{text}' (wysokość={height_original:.2f}px)")
                continue

            mid_y = (top_left[1] + bottom_right[1]) / 2

            added_to_line = False
            for line in grouped_lines:
                if abs(mid_y - line['y_mean']) < line_threshold:
                    line['elements'].append((bbox, text))
                    line['y_mean'] = np.mean([line['y_mean'], mid_y])
                    added_to_line = True
                    break
            if not added_to_line:
                grouped_lines.append({'y_mean': mid_y, 'elements': [(bbox, text)]})

        for line in grouped_lines:
            line['elements'] = sorted(line['elements'], key=lambda x: x[0][0][0])

        screen_width  = gray_img.shape[1]
        screen_height = gray_img.shape[0]
        grouped_lines = filter_centered_text(grouped_lines, screen_width, screen_height)

        if not grouped_lines:
            _last_paragraphs = []
            return []

        sorted_lines = sorted(grouped_lines, key=lambda x: x['y_mean'])

        # === KLASTERYZACJA: duża przerwa pionowa = osobny dialog ===
        # Próg: 4× line_threshold (wiersze w tym samym dialogu są bliżej siebie)
        cluster_gap = line_threshold * 4
        clusters = []
        current_cluster = [sorted_lines[0]]

        for i in range(1, len(sorted_lines)):
            gap = sorted_lines[i]['y_mean'] - sorted_lines[i - 1]['y_mean']
            if gap > cluster_gap:
                clusters.append(current_cluster)
                current_cluster = [sorted_lines[i]]
            else:
                current_cluster.append(sorted_lines[i])
        clusters.append(current_cluster)

        paragraphs = []
        for cluster in clusters:
            lines_text = [
                " ".join(text for _, text in line['elements'])
                for line in sorted(cluster, key=lambda x: x['y_mean'])
            ]
            para_text = "\n".join(lines_text).strip()
            if para_text:
                paragraphs.append(para_text)

        if paragraphs:
            debug.log(debug.DEBUG, "OCR", f"Wykryto {len(paragraphs)} paragrafy dialogowe")

        _last_paragraphs = paragraphs
        return paragraphs

    except Exception as e:
        debug.log(debug.ERROR, "OCR", f"Błąd w capture_and_extract_paragraphs: {e}")
        return []