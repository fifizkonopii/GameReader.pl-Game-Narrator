
# ============================================================
# IMPORTY
# ============================================================
import re

from rapidfuzz import fuzz

from core import debug
from core import state as config


# ============================================================
# USUWANIE NAZW POSTACI Z TEKSTU OCR
# ============================================================
def remove_character_name(text: str) -> str:
    if not text:
        return text

    # === Nazwa postaci na początku linii (stary format) ===
    for name in config.character_names:
        if text.startswith(f"{name}:") or text.startswith(f"{name};"):
            debug.log(debug.DEBUG, "OCR", f"Wykryto nazwę postaci: {name}")
            return text[len(name) + 1:].strip()

    # === Nazwa postaci w osobnej linii (nowy format) ===
    lines = text.splitlines()
    if len(lines) < 2:
        return text

    first_line_raw = lines[0].strip()
    if not first_line_raw:
        return text

    # === Bezpieczniki ===
    if len(first_line_raw) > 40:
        return text
    
    if len(first_line_raw.split()) > 4:
        return text

    # === Normalizacja pierwszej linii do porównań ===
    first_line_norm = first_line_raw.upper().rstrip(":;.-— ").strip()

    # === Dokładne dopasowanie (TYLKO dla pierwszej linii) ===
    for name in config.character_names:
        if first_line_norm == name.upper():
            debug.log(debug.DEBUG, "OCR", f"Wykryto nazwę postaci (multi-line): {name}")
            return "\n".join(lines[1:]).strip()

    # === Fuzzy match (TYLKO dla pierwszej linii) ===
    best_name = None
    best_score = 0

    for name in config.character_names:
        score = fuzz.ratio(first_line_norm, name.upper())
        if score > best_score:
            best_score = score
            best_name = name

    if best_name and best_score >= 90:
        debug.log(
            debug.DEBUG,
            "OCR",
            f"Wykryto nazwę postaci (multi-line fuzzy): {best_name} (dopasowanie: {best_score:.0f}%)"
        )
        return "\n".join(lines[1:]).strip()

    return text

# ============================================================
# FILTROWANIE TEKSTU PO LINII ŚRODKOWEJ (L1 / L2 / L3)
# ============================================================
def filter_centered_text(grouped_lines, screen_width, screen_height):
    if not (config.USE_CENTER_LINE_1 or config.USE_CENTER_LINE_2 or config.USE_CENTER_LINE_3):
        return grouped_lines

    filtered_lines = []
    margin = int(config.CENTER_LINE_MARGIN * config.RESOLUTION_DOWNSCALE)

    if config.USE_CENTER_LINE_1:
        center_start_1 = int((screen_width // 2) - (margin // 2))
        center_end_1 = int((screen_width // 2) + (margin // 2))
    if config.USE_CENTER_LINE_2:
        center_start_2 = int(config.CENTER_LINE_2_START * config.RESOLUTION_DOWNSCALE)
        center_end_2 = center_start_2 + margin
    if config.USE_CENTER_LINE_3:
        center_start_3 = int(screen_width * config.CENTER_LINE_3_START_RATIO)
        center_end_3 = center_start_3 + margin

    for line in grouped_lines:
        for bbox, text in line['elements']:
            top_left_x = bbox[0][0]
            bottom_right_x = bbox[2][0]

            match_vertical = (
                (config.USE_CENTER_LINE_1 and top_left_x <= center_end_1 and bottom_right_x >= center_start_1) or
                (config.USE_CENTER_LINE_2 and top_left_x <= center_end_2 and bottom_right_x >= center_start_2) or
                (config.USE_CENTER_LINE_3 and top_left_x <= center_end_3 and bottom_right_x >= center_start_3)
            )

            if match_vertical:
                filtered_lines.append(line)
                break

    return filtered_lines
