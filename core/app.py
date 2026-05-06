
# ============================================================
# IMPORTY
# ============================================================
import os
import threading
import time
import queue
from collections import deque
from rapidfuzz import fuzz

from PySide6.QtCore import QObject, Signal, Qt, QMetaObject
from PySide6.QtWidgets import QSystemTrayIcon, QMenu, QApplication
from PySide6.QtGui import QIcon, QAction, QCursor

from core import state
from core.validation import validate_before_reader_start
from core import debug

from ui.widgets import show_validation_result
from ui.theme.global_qss import build_global_qss

from ocr import capture as ocr
from ocr.filters import remove_character_name
from audio import player as audio
from input import hotkeys

# ============================================================
# STAŁE MATCHOWANIA
# ============================================================
# Kandydaci w promieniu tylu punktów od najlepszego score'a
_SCORE_MARGIN       = 5
# Ile linii do przodu od ostatniego dopasowania przeszukiwać priorytetowo
_FORWARD_WINDOW     = 400
# Ile linii do tyłu dopuszczamy (powtórzenia, echo OCR)
_BACK_WINDOW        = 30
# O ile punktów globalny najlepszy musi bić najlepszego w oknie,
# żeby przejąć priorytet (zapobiega skokom do tyłu o setki linii)
_GLOBAL_OVERRIDE_MARGIN = 12

# Typewriter: fragment OCR musi pokrywać co najmniej tę część długości linii docelowej
# Wyższy próg = czekamy na więcej tekstu przed dopasowaniem → mniej fałszywych trafień
# np. 0.65 = "Nie musisz" (10) nie trafi w "Nie ma go" (9): 10/9>1 ale fuzz niski
#            "Nie m" (5) nie trafi w "Nie ma go" (9): 5/9=55% < 65% → czeka dalej
_TYPEWRITER_MIN_COVERAGE  = state.TYPEWRITER_MIN_COVERAGE

# Typewriter: jeśli rywal ma score w tym marginesie od zwycięzcy → niejednoznaczne → czekaj
# Np. margin=20: "Nie mu" pasuje do "Nie ma go" (83%) i "Nie musisz" (90%) → 90-83=7 < 20 → czeka
_TYPEWRITER_DISAMBIG_MARGIN = 20



# ============================================================
# GLOBALNE REFERENCJE UI
# ============================================================
debug_overlay = None
main_window_instance = None
tray_icon = None
tray_menu = None

# ============================================================
# UI BRIDGE (SYGNAŁY)
# ============================================================
class _UIBridge(QObject):
    toggle_debug_overlay = Signal()
    open_settings = Signal()
    enable_reader = Signal()
    disable_reader = Signal()
    reload_ui = Signal()
    toggle_debug_console = Signal()

ui_bridge = _UIBridge()

# ============================================================
# UI CALLBACKS (STANU CAPTURE)
# ============================================================
_capture_state_callback = None

def register_capture_state_callback(callback):
    global _capture_state_callback
    _capture_state_callback = callback

def _notify_capture_state(enabled: bool):
    if _capture_state_callback:
        _capture_state_callback(enabled)

def register_main_window(window):
    global main_window_instance
    main_window_instance = window

# ============================================================
# MATCHING (OKIENKOWY, KIERUNKOWY, CASE-INSENSITIVE)
# ============================================================
def _find_best_match(text: str, dialog_lines: list, last_index: int, *, typewriter_mode: bool = False, frame_stable: bool = False) -> tuple[int, float]:
    """
    Zwraca (index_1based, score) najlepiej pasującej linii dialogowej,
    lub (-1, 0.0) jeśli żadna nie przekracza progu.

    Strategia:
      1. Porównanie case-insensitive (różnice OCR vs plik napisów).
      2. Zbieramy kandydatów w promieniu _SCORE_MARGIN punktów od najlepszego.
      3. Jeśli znamy poprzednią pozycję, priorytetowo szukamy w oknie
         [last -_BACK_WINDOW, last +_FORWARD_WINDOW]. Globalny wynik
         przejmuje kontrolę tylko gdy bije okienkowy o _GLOBAL_OVERRIDE_MARGIN.
      4. Wśród finałowych kandydatów: preferujemy kierunek do przodu,
         potem bliskość, potem wyższy score.
    """
    if not dialog_lines:
        return -1, 0.0

    text_norm = text.lower()

    # --- jeden przebieg: score każdej linii ---
    # Typewriter: tekst rośnie od lewej → OCR fragment to zawsze PREFIX linii.
    # Porównujemy OCR do pierwszych len(text) znaków każdej linii dialogowej,
    # a nie partial_ratio (które znajdzie fragment gdziekolwiek, np. na końcu).
    if typewriter_mode:
        n = len(text_norm)
        scores = [fuzz.ratio(text_norm, line.lower()[:n]) for line in dialog_lines]
    else:
        scores = [fuzz.ratio(text_norm, line.lower()) for line in dialog_lines]
    best_score = max(scores)

    # --- próg na podstawie długości linii źródłowej LUB tekstu OCR ---
    def _threshold(line: str) -> float:
        return (
            state.SIMILARITY_THRESHOLD2
            if len(line) < state.SHORT_LINE_MAX_LENGTH or len(text_norm) < state.SHORT_LINE_MAX_LENGTH
            else state.SIMILARITY_THRESHOLD
        )

    if best_score < state.SIMILARITY_THRESHOLD:
        return -1, 0.0

    # --- kandydaci: wynik w marginesie I powyzej swojego progu I proporcja długości ---
    min_viable = best_score - _SCORE_MARGIN
    candidates = []
    for i, line in enumerate(dialog_lines):
        sc = scores[i]
        if sc < min_viable or sc < _threshold(line):
            continue
        # Odrzucamy gdy tekst OCR jest 2x dłuższy niż kandydująca linia
        # i score nie jest bardzo wysoki — zapobiega matchowaniu fragmentu
        # długiego tekstu z krótką linią na początku pliku
        if len(text_norm) > len(line) * 2.0 and sc < 90:
            continue
        # Odrzucamy gdy tekst OCR jest 2x krótszy niż kandydująca linia
        # i score nie jest bardzo wysoki — tylko w trybie normalnym
        # (typewriter celowo matchuje krótki tekst do długiej linii)
        if not typewriter_mode and len(line) > len(text_norm) * 2.0 and sc < 90:
            continue
        # Typewriter: odrzucamy jeśli fragment OCR pokrywa < MIN_COVERAGE linii
        # "Musi" (4) vs "Musisz wygłosić przemówienie" (28): 4/28=14% < 30% → skip
        if typewriter_mode and len(line) > 0 and len(text_norm) / len(line) < _TYPEWRITER_MIN_COVERAGE:
            continue
        # Typewriter: pierwsze znaki OCR muszą pasować do początku linii
        # Zapobiega false-match gdy OCR zgubił prefix — np. "co tam" zamiast "no co tam"
        if typewriter_mode:
            check_len = min(4, len(text_norm))
            if check_len >= 2 and fuzz.ratio(text_norm[:check_len], line.lower()[:check_len]) < 60:
                continue
        candidates.append((i + 1, sc))

    if not candidates:
        return -1, 0.0

    # --- brak poprzedniej pozycji: najlepszy score, tiebreak = najniższy index ---
    if last_index < 0:
        candidates.sort(key=lambda x: (-x[1], x[0]))
        return candidates[0]

    # --- szukaj po całym pliku, tiebreak = najbliższy last_index ---
    candidates.sort(key=lambda x: (abs(x[0] - last_index), -x[1]))
    winner_idx, winner_sc = candidates[0]

    # Typewriter: sprawdzamy czy zwycięzca jest jednoznaczny.
    # Jeśli inna linia w całym pliku ma podobny score dla tego samego prefiksu,
    # fragment jest za krótki żeby rozstrzygnąć — czekamy na więcej tekstu.
    # Wyjątek: frame_stable=True → animacja skończyła się, tekst kompletny → odpалamy.
    if typewriter_mode and not frame_stable and len(candidates) > 0:
        n = len(text_norm)
        all_prefix_scores = [fuzz.ratio(text_norm, line.lower()[:n]) for line in dialog_lines]
        rivals = [
            sc for i, sc in enumerate(all_prefix_scores)
            if (i + 1) != winner_idx and sc >= winner_sc - _TYPEWRITER_DISAMBIG_MARGIN
        ]
        if rivals:
            debug.log(
                debug.DEBUG, "OCR",
                f"Typewriter: niejednoznaczne dopasowanie linii {winner_idx} "
                f"(score={winner_sc:.0f}, rywal={max(rivals):.0f}) — czekam na więcej tekstu"
            )
            return -1, 0.0

    return winner_idx, winner_sc

def main_loop():
    last_text = ""
    consecutive_empty_reads = 0
    last_capture_time = 0.0

    if not hasattr(main_loop, "last_match_index"):
        main_loop.last_match_index = -1

    if not hasattr(main_loop, "last_texts"):
        main_loop.last_texts = deque(maxlen=2)

    if not hasattr(main_loop, "last_matched_text"):
        main_loop.last_matched_text = ""

    if not hasattr(main_loop, "typewriter_matched"):
        main_loop.typewriter_matched = {}

    if not hasattr(main_loop, "stable_text_count"):
        main_loop.stable_text_count = 0

    while True:
        # === LEKTOR WYŁĄCZONY -> ŚPIMY ===
        if not state.capture_enabled:
            time.sleep(0.3)
            continue

        now = time.time()

        # === TWARDY INTERWAŁ ===
        if now - last_capture_time < state.CAPTURE_INTERVAL:
            time.sleep(0.1)
            continue

        last_capture_time = now

        # === OCR (SINGLE lub MULTI-PARAGRAPH) ===
        current_monitor = state.get_active_monitor_rect()
        if state.ENABLE_PARAGRAPH_OCR:
            paragraphs = ocr.capture_and_extract_paragraphs(current_monitor)
        else:
            raw = ocr.capture_and_extract_text(current_monitor)
            paragraphs = [raw.strip()] if raw and raw.strip() else []

        # === BRAK TEKSTU ===
        if not paragraphs:
            consecutive_empty_reads += 1

            # === RESET TYPEWRITER PO PUSTEJ KLATCE ===
            if state.ENABLE_TYPEWRITER_WAIT:
                main_loop.typewriter_matched = {}

            # === LOG BRAK NAPISÓW CO JAKIŚ CZAS ===
            if consecutive_empty_reads % 20 == 0:
                debug.log(debug.DEBUG, "OCR", "Brak napisów (pusta ramka)")

            # === OCR IDLE ===
            if consecutive_empty_reads >= state.EMPTY_READS_TO_IDLE:
                time.sleep(state.IDLE_SLEEP_SECONDS)
                continue

            time.sleep(0.1)
            continue

        consecutive_empty_reads = 0

        # === DUPLIKAT OCR -> OLEWAMY ===
        combined_fingerprint = "\n\n".join(paragraphs)
        if combined_fingerprint == last_text:
            main_loop.stable_text_count += 1
            if not state.ENABLE_TYPEWRITER_WAIT:
                continue
            # Typewriter: ten sam tekst może być stabilnym końcem animacji
            # — nie skipujemy, typewriter_matched zadba o dedup
        else:
            main_loop.stable_text_count = 0
        last_text = combined_fingerprint

        dialog_lines = state.dialog_lines
        if not dialog_lines:
            continue

        # === PRZETWARZANIE KAŻDEGO PARAGRAFU OSOBNO ===
        for para_idx, text in enumerate(paragraphs):
            text = text.strip()
            if not text:
                continue

            # === RAW LOGGING (DEBUG UI) ===
            debug.log(debug.DEBUG, "OCR", f"Rozpoznano tekst: '{text}'")

            # === USUWANIE NAZW POSTACI ===
            if state.ENABLE_REMOVE_CHARACTER_NAME:
                text = remove_character_name(text).strip()
            if not text:
                continue

            # === FILTR ŚMIECIOWEGO OCR ===
            # Odrzucamy tekst krótszy niż 3 znaki
            if len(text) < 3:
                debug.log(debug.DEBUG, "OCR", f"Odrzucono za krótki tekst: '{text}'")
                continue
            # Odrzucamy gdy ponad 30% znaków to nie-litery i nie-spacje
            # (np. "@", "#", symbole z błędnego OCR)
            non_alpha = sum(1 for c in text if not c.isalpha() and c not in " .,!?-–—'\":")
            if non_alpha / max(len(text), 1) > 0.30:
                debug.log(debug.DEBUG, "OCR", f"Odrzucono śmieciowy OCR: '{text}'")
                continue

            # === MATCHING ===
            frame_stable = main_loop.stable_text_count >= state.TYPEWRITER_STABLE_READS
            if state.ENABLE_TYPEWRITER_WAIT:
                best_match_index, match_score = _find_best_match(
                    text, dialog_lines, main_loop.last_match_index,
                    typewriter_mode=True, frame_stable=frame_stable
                )
                # Już dopasowaliśmy ten paragraf w tej sekwencji typewriter — skip
                if best_match_index != -1 and \
                        main_loop.typewriter_matched.get(para_idx) == best_match_index:
                    debug.log(
                        debug.DEBUG, "OCR",
                        f"Typewriter: linia {best_match_index} już odegrana dla para {para_idx}, pomijam"
                    )
                    continue
                if best_match_index != -1:
                    main_loop.typewriter_matched[para_idx] = best_match_index
            else:
                best_match_index, match_score = _find_best_match(
                    text, dialog_lines, main_loop.last_match_index
                )

            if best_match_index == -1:
                continue

            debug.log(
                debug.DEBUG, "OCR",
                f"Dopasowanie: linia {best_match_index} ({match_score:.0f}%): "
                f"'{dialog_lines[best_match_index - 1]}'"
            )

            if best_match_index in main_loop.last_texts:
                continue

            # === DEDUP: ta sama treść linii co ostatnio odczytana → skip ===
            matched_line_text = dialog_lines[best_match_index - 1]
            if main_loop.last_matched_text and fuzz.ratio(
                matched_line_text.lower(), main_loop.last_matched_text.lower()
            ) >= 92:
                debug.log(
                    debug.DEBUG, "OCR",
                    f"Pominięto duplikat treści: '{matched_line_text}'"
                )
                continue

            main_loop.last_match_index = best_match_index
            main_loop.last_texts.append(best_match_index)
            main_loop.last_matched_text = matched_line_text

            # === AUDIO ===
            base_filename = f"output1 ({best_match_index})"
            found_audio_file = None

            for ext in state.SUPPORTED_AUDIO_FORMATS:
                path = os.path.join(state.audio_dir, base_filename + ext)
                if os.path.exists(path):
                    found_audio_file = path
                    break

            if not found_audio_file:
                continue

            with state.audio_playing_lock:
                is_busy = state.is_audio_playing

            queue_size = state.audio_queue.qsize()

            if state.ENABLE_DYNAMIC_SPEED:
                current_speed = (
                    state.OVERLAP_PLAYBACK_SPEED
                    if (is_busy or queue_size > 0)
                    else state.BASE_PLAYBACK_SPEED
                )
            else:
                current_speed = 1.0

            try:
                if state.audio_queue.full():
                    state.audio_queue.get_nowait()
                    state.audio_queue.task_done()

                state.audio_queue.put((found_audio_file, current_speed), block=False)
            except queue.Full:
                pass

# ============================================================
# INICJALIZACJA BACKENDU
# ============================================================
def initialize_backend():
    state.ensure_defaults()

    _connect_ui_signals()

    hotkeys.start_listener()

    debug.log(debug.INFO, "Core", "Backend zainicjalizowany")

    state.refresh_selected_mss_monitor_rect()

    if not hasattr(state, "audio_thread") or state.audio_thread is None or not state.audio_thread.is_alive():
        state.audio_thread = threading.Thread(
            target=audio.audio_player,
            daemon=True
        )
        state.audio_thread.start()
        debug.log(debug.INFO, "Audio", "Wątek audio uruchomiony")

# ============================================================
# STEROWANIE LEKTOREM
# ============================================================
def enable_reader():
    if state.capture_enabled:
        return

    # === WALIDACJA STARTOWA LEKTORA ===
    result = validate_before_reader_start(state)
    parent = main_window_instance
    if not show_validation_result(
        result,
        parent=main_window_instance,
        error_context="Nie można uruchomić lektora."
    ):
        return

    # === START LEKTORA ===
    state.capture_enabled = True
    debug.log(debug.INFO, "Reader", "Lektor uruchomiony")

    state.refresh_selected_mss_monitor_rect()

    if state.main_loop_thread is None or not state.main_loop_thread.is_alive():
        state.main_loop_thread = threading.Thread(
            target=main_loop,
            daemon=True
        )
        state.main_loop_thread.start()

    audio.play_system_sound(
        audio.find_system_sound("on")
    )

    _notify_capture_state(True)

    # === CHOWANIE OKNA DO TRAYA ===
    if main_window_instance:
        main_window_instance.hide()

    if tray_icon:
        tray_icon.setToolTip("GameReader – lektor aktywny")

    update_tray_status()

def disable_reader():
    if not state.capture_enabled:
        return

    state.capture_enabled = False
    debug.log(debug.INFO, "Reader", "Lektor zatrzymany")

    audio.play_system_sound(
        audio.find_system_sound("off")
    )

    _notify_capture_state(False)

    if tray_icon:
        tray_icon.setToolTip("GameReader")

    update_tray_status()

def toggle_reader():
    if state.capture_enabled:
        disable_reader()
    else:
        enable_reader()

def can_enable_reader() -> bool:
    result = validate_before_reader_start(state)
    return show_validation_result(
        result,
        parent=main_window_instance,
        error_context="Nie można uruchomić lektora."
    )

# ============================================================
# DEBUG OVERLAY
# ============================================================
def toggle_debug_overlay():
    global debug_overlay

    if debug_overlay is None:
        # === IMPORT MUSI TU ZOSTAĆ - OCHRONA CYKLI ===
        from ui.overlay.debug_overlay import DebugOverlay
        debug_overlay = DebugOverlay()

    debug_overlay.toggle()

# ============================================================
# UI SYGNAŁY
# ============================================================
def _connect_ui_signals():
    ui_bridge.toggle_debug_overlay.connect(toggle_debug_overlay)
    ui_bridge.open_settings.connect(_show_settings_window)
    ui_bridge.enable_reader.connect(enable_reader)
    ui_bridge.disable_reader.connect(disable_reader)
    ui_bridge.reload_ui.connect(_reload_ui_from_state)

def _reload_ui_from_state():
    win = main_window_instance
    if not win:
        return

    QMetaObject.invokeMethod(
        win,
        "reload_ui_from_state",
        Qt.QueuedConnection
    )

# ============================================================
# OKNO USTAWIEŃ
# ============================================================
def _show_settings_window():
    global main_window_instance

    if state.capture_enabled:
        disable_reader()

    if not main_window_instance:
        return

    win = main_window_instance

    if win.isMinimized():
        win.showNormal()

    win.setWindowFlag(Qt.WindowStaysOnTopHint, True)
    win.show()
    win.raise_()
    win.activateWindow()

    win.setWindowFlag(Qt.WindowStaysOnTopHint, False)
    win.show()

    for i in range(win.sidebar.count()):
        item = win.sidebar.item(i)
        if item and item.text() == "Zaawansowane":
            win.sidebar.setCurrentRow(i)
            break

# ============================================================
# SYSTEM TRAY
# ============================================================
_tray_action_status = None
_tray_action_preset = None

def update_tray_status():
    global _tray_action_status, _tray_action_preset
    if _tray_action_status is None:
        return
    if state.capture_enabled:
        _tray_action_status.setText("●  Lektor aktywny")
    else:
        _tray_action_status.setText("○  Lektor zatrzymany")
    name = state.preset_filename or "—"
    _tray_action_preset.setText(f"Preset: {name}")

def setup_tray():
    global tray_icon, tray_menu, main_window_instance
    global _tray_action_status, _tray_action_preset

    if tray_icon is not None:
        return

    app = QApplication.instance()

    tray_icon = QSystemTrayIcon(app)
    tray_icon.setIcon(QIcon(state.ICON_PATH))
    tray_icon.setToolTip("GameReader")

    tray_menu = QMenu()
    tray_menu.setStyleSheet(build_global_qss())

    # === INFO: STATUS ===
    _tray_action_status = QAction("○  Lektor zatrzymany", tray_menu)
    _tray_action_status.setEnabled(False)
    tray_menu.addAction(_tray_action_status)

    # === INFO: PRESET ===
    _tray_action_preset = QAction("Preset: —", tray_menu)
    _tray_action_preset.setEnabled(False)
    tray_menu.addAction(_tray_action_preset)

    tray_menu.addSeparator()

    action_settings = QAction("Okno ustawień", tray_menu)
    action_console  = QAction("Konsola debug", tray_menu)
    action_exit = QAction("Zamknij program", tray_menu)

    action_settings.triggered.connect(lambda: ui_bridge.open_settings.emit())
    action_console.triggered.connect(lambda: ui_bridge.toggle_debug_console.emit())
    action_exit.triggered.connect(_quit_app)

    tray_menu.addAction(action_settings)
    tray_menu.addAction(action_console)
    tray_menu.addSeparator()
    tray_menu.addAction(action_exit)

    tray_icon.setContextMenu(tray_menu)
    tray_icon.activated.connect(_tray_activated)

    tray_icon.show()

def _tray_activated(reason):
    # === PPM ===
    if reason in (QSystemTrayIcon.Context, QSystemTrayIcon.Trigger):
        if tray_menu:
            tray_menu.popup(QCursor.pos())
            return

    # === 2CLICK ===
    if reason == QSystemTrayIcon.DoubleClick:
        ui_bridge.open_settings.emit()

def _quit_app():
    global tray_icon

    try:
        disable_reader()
    except Exception:
        pass

    if tray_icon:
        tray_icon.hide()

    QApplication.quit()
