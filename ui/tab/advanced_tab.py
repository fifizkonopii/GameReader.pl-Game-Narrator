
# ============================================================
# IMPORTY
# ============================================================
import re

from PySide6.QtCore import Qt, QPropertyAnimation, QVariantAnimation, QEasingCurve, QRegularExpression
from PySide6.QtGui import QRegularExpressionValidator, QColor
from PySide6.QtWidgets import (
    QWidget, QLabel, QLineEdit,
    QVBoxLayout, QHBoxLayout, QGroupBox,
)

from ui.theme.theme import (
    TAB_MARGIN_H,
    TAB_MARGIN_V,
    TAB_SPACING,
    OPTION_LABEL_ICON_SPACING,
    LABEL_FIXED_WIDTH_MD,
    TEXT_PRIMARY, TEXT_MUTED,
)

from ui.widgets import (
    ToggleSwitch,
    IconComboBox,
    HUDToggle,
    HelpIcon,
    ErrorTooltipLineEdit,
    FocusClearingTab,
)

from ui.tooltips import TOOLTIPS
from core import state
from core import constants as C


# ============================================================
# ZAKŁADKA: ZAAWANSOWANE
# ============================================================
class AdvancedTab(FocusClearingTab):
    def __init__(self):
        super().__init__()

        # =====================================================
        # GŁÓWNY LAYOUT (2 KOLUMNY)
        # =====================================================
        main_layout = QHBoxLayout(self)
        main_layout.setContentsMargins(
            TAB_MARGIN_H,
            TAB_MARGIN_V,
            TAB_MARGIN_H,
            TAB_MARGIN_V
        )
        main_layout.setSpacing(TAB_SPACING)

        left_layout = QVBoxLayout()
        left_layout.setSpacing(20)

        right_layout = QVBoxLayout()
        right_layout.setSpacing(20)

        main_layout.addLayout(left_layout, 2)
        main_layout.addLayout(right_layout, 1)

        # =====================================================
        # LEWA STRONA
        # =====================================================

        # === OCR I WYDAJNOŚĆ ===
        ocr_group = QGroupBox("Ustawienia OCR i wydajności")
        ocr_layout = QVBoxLayout(ocr_group)
        ocr_layout.setSpacing(10)

        self.ocr_quality = self._text_input("advanced_ocr_quality")
        self.capture_interval = self._text_input("advanced_capture_interval")
        self.min_height = self._text_input("advanced_min_height")
        self.max_height = self._text_input("advanced_max_height")

        ocr_layout.addLayout(self._row("Jakość obrazu OCR", self.ocr_quality, "advanced_ocr_quality"))
        ocr_layout.addLayout(self._row("Interwał przechwytywania (ms)", self.capture_interval, "advanced_capture_interval"))
        ocr_layout.addLayout(self._row("Wysokość minimalna (px)", self.min_height, "advanced_min_height"))
        ocr_layout.addLayout(self._row("Wysokość maksymalna (px)", self.max_height, "advanced_max_height"))

        # === LINIE POMOCNICZE ===
        lines_group = QGroupBox("Ustawienia linii pomocniczych")
        lines_layout = QVBoxLayout(lines_group)
        lines_layout.setSpacing(10)

        self.line_width = self._text_input("advanced_line_width")
        self.line_left_2 = self._text_input("advanced_line_left_2")
        self.line_left_3 = self._text_input("advanced_line_left_3")

        lines_layout.addLayout(self._row("Szerokość linii (px)", self.line_width, "advanced_line_width"))
        lines_layout.addLayout(self._row("Lewa krawędź linii 2 (px)", self.line_left_2, "advanced_line_left_2"))
        lines_layout.addLayout(self._row("Lewa krawędź linii 3 (%)", self.line_left_3, "advanced_line_left_3"))

        # === LEKTOR ===
        tts_group = QGroupBox("Ustawienia lektora")
        tts_layout = QVBoxLayout(tts_group)
        tts_layout.setSpacing(10)

        self.tts_speed = self._text_input("advanced_tts_speed")
        self.tts_boost_speed = self._text_input("advanced_tts_boost_speed")
        self.game_ducking = self._text_input("advanced_game_ducking")
        self.memory_lines = IconComboBox()
        self.memory_lines.addItems(["1", "2", "3"])
        self.memory_lines.setFixedWidth(100)

        self.tts_speed_label, row = self._row("Prędkość lektora", self.tts_speed, "advanced_tts_speed", return_label=True)
        tts_layout.addLayout(row)
        self.tts_boost_label, row = self._row("Prędkość lektora (przyśpieszenie)", self.tts_boost_speed, "advanced_tts_boost_speed", return_label=True)
        tts_layout.addLayout(row)
        tts_layout.addLayout(self._row("Poziom wyciszania gry", self.game_ducking, "advanced_game_ducking"))
        tts_layout.addLayout(self._row("Linie dialogów w pamięci", self.memory_lines, "advanced_memory_lines"))

        left_layout.addWidget(ocr_group)
        left_layout.addWidget(lines_group)
        left_layout.addWidget(tts_group)
        left_layout.addStretch()

        # =====================================================
        # PRAWA STRONA
        # =====================================================

        # === AKTYWACJA FUNKCJI ===
        features_group = QGroupBox("Aktywacja funkcji zaawansowanych")
        features_layout = QVBoxLayout(features_group)
        features_layout.setSpacing(12)

        self.typewriter_toggle = ToggleSwitch()
        self.paragraph_ocr_toggle = ToggleSwitch()
        self.line1_toggle = ToggleSwitch()
        self.line2_toggle = ToggleSwitch()
        self.line3_toggle = ToggleSwitch()

        features_layout.addLayout(
            self._row("Linia pomocnicza 1", self.line1_toggle, "advanced_helper_line_1")
        )
        features_layout.addLayout(
            self._row("Linia pomocnicza 2", self.line2_toggle, "advanced_helper_line_2")
        )
        features_layout.addLayout(
            self._row("Linia pomocnicza 3", self.line3_toggle, "advanced_helper_line_3")
        )
        features_layout.addLayout(
            self._row("Tryb wielu dialogów", self.paragraph_ocr_toggle, "advanced_paragraph_ocr")
        )
        features_layout.addLayout(
            self._row("Tryb animowania napisów", self.typewriter_toggle, "advanced_typewriter_wait")
        )

        # === AUDIO ===
        audio_group = QGroupBox("System odtwarzania audio")
        audio_layout = QVBoxLayout(audio_group)

        audio_mode_layout = QHBoxLayout()
        audio_mode_layout.setSpacing(10)

        self.audio_dynamic_label = QLabel("System dynamicznej prędkości")
        self.audio_static_label = QLabel("System prędkości stałych")

        # === AUDIO LABEL COLOR ANIMATION SETUP ===
        self._color_animations = {}
        _active   = QColor(TEXT_PRIMARY)
        _inactive = QColor(TEXT_MUTED)

        for label in (self.audio_dynamic_label, self.audio_static_label):
            anim = QVariantAnimation(self)
            anim.setDuration(180)
            anim.setEasingCurve(QEasingCurve.OutCubic)
            captured = label
            anim.valueChanged.connect(
                lambda c, lbl=captured: lbl.setStyleSheet(f"color: {c.name()};")
            )
            self._color_animations[label] = anim

        self.audio_dynamic_label.setProperty("class", "fixed-md")
        self.audio_static_label.setProperty("class", "fixed-md")
        self.audio_dynamic_label.setFixedWidth(LABEL_FIXED_WIDTH_MD)
        self.audio_static_label.setFixedWidth(LABEL_FIXED_WIDTH_MD)
        self.audio_dynamic_label.setAlignment(Qt.AlignRight | Qt.AlignVCenter)
        self.audio_static_label.setAlignment(Qt.AlignLeft | Qt.AlignVCenter)

        self.audio_mode_toggle = HUDToggle()

        self.audio_mode_toggle.toggled.connect(self._update_audio_mode_labels)
        self.audio_mode_toggle.toggled.connect(self._audio_mode_changed)

        # === DYNAMICZNE LABEL + HELP ===
        dynamic_container = QWidget()
        dynamic_layout = QHBoxLayout(dynamic_container)
        dynamic_layout.setContentsMargins(0, 0, 0, 0)
        dynamic_layout.setSpacing(OPTION_LABEL_ICON_SPACING)

        dynamic_layout.addWidget(
            HelpIcon(TOOLTIPS["advanced_audio_dynamic"])
        )
        dynamic_layout.addWidget(self.audio_dynamic_label)

        # === STATYCZNE LABEL + HELP ===
        static_container = QWidget()
        static_layout = QHBoxLayout(static_container)
        static_layout.setContentsMargins(0, 0, 0, 0)
        static_layout.setSpacing(OPTION_LABEL_ICON_SPACING)

        static_layout.addWidget(self.audio_static_label)
        static_layout.addWidget(
            HelpIcon(TOOLTIPS["advanced_audio_static"])
        )

        audio_mode_layout.addWidget(dynamic_container)
        audio_mode_layout.addWidget(self.audio_mode_toggle)
        audio_mode_layout.addWidget(static_container)

        audio_layout.addLayout(audio_mode_layout)

        right_layout.addWidget(features_group)
        right_layout.addWidget(audio_group)
        right_layout.addStretch()

        self._load_state_to_ui()

        # === OCR / WYDAJNOŚĆ (WALIDACJA) ===
        self.ocr_quality.editingFinished.connect(self._on_ocr_quality_changed)

        # KONWERSJA MSEK NA SEK
        self.capture_interval.editingFinished.connect(
            lambda: self._commit_int(
                self.capture_interval,
                "CAPTURE_INTERVAL",
                int(C.CAPTURE_INTERVAL_MIN * 1000),
                int(C.CAPTURE_INTERVAL_MAX * 1000),
                transform=lambda ms: ms / 1000,
            )
        )

        self.min_height.editingFinished.connect(
            lambda: self._on_min_max_height_changed()
        )

        self.max_height.editingFinished.connect(
            lambda: self._on_min_max_height_changed()
        )

        # === LINIE POMOCNICZE (WALIDACJA) ===
        self.line_width.editingFinished.connect(
            lambda: self._commit_int(
                self.line_width,
                "CENTER_LINE_MARGIN",
                C.CENTER_LINE_MARGIN_MIN,
                C.CENTER_LINE_MARGIN_MAX,
            )
        )
        self.line_left_2.editingFinished.connect(
            lambda: self._commit_int(
                self.line_left_2,
                "CENTER_LINE_2_START",
                C.CENTER_LINE_2_START_MIN,
                C.CENTER_LINE_2_START_MAX,
            )
        )
        self.line_left_3.editingFinished.connect(
            lambda: self._commit_float(
                self.line_left_3,
                "CENTER_LINE_3_START_RATIO",
                C.CENTER_LINE_3_START_RATIO_MIN,
                C.CENTER_LINE_3_START_RATIO_MAX,
            )
        )

        # === LEKTOR / AUDIO (WALIDACJA) ===
        self.tts_speed.editingFinished.connect(
            lambda: self._commit_float(self.tts_speed, "BASE_PLAYBACK_SPEED", C.BASE_PLAYBACK_SPEED_MIN, C.BASE_PLAYBACK_SPEED_MAX)
        )

        self.tts_boost_speed.editingFinished.connect(
            lambda: self._commit_float(self.tts_boost_speed, "OVERLAP_PLAYBACK_SPEED", C.OVERLAP_PLAYBACK_SPEED_MIN, C.OVERLAP_PLAYBACK_SPEED_MAX)
        )

        self.game_ducking.editingFinished.connect(
            lambda: self._commit_float(
                self.game_ducking,
                "VOLUME_REDUCTION_LEVEL",
                C.VOLUME_REDUCTION_LEVEL_MIN,
                C.VOLUME_REDUCTION_LEVEL_MAX,
            )
        )

        self.memory_lines.currentTextChanged.connect(
            self._memory_lines_changed
        )

        # === PRZEŁĄCZNIKI ===
        self.typewriter_toggle.stateChanged.connect(
            lambda v: setattr(state, "ENABLE_TYPEWRITER_WAIT", bool(v))
        )

        self.paragraph_ocr_toggle.stateChanged.connect(
            lambda v: setattr(state, "ENABLE_PARAGRAPH_OCR", bool(v))
        )

        self.line1_toggle.stateChanged.connect(
            lambda v: setattr(state, "USE_CENTER_LINE_1", bool(v))
        )

        self.line2_toggle.stateChanged.connect(
            lambda v: setattr(state, "USE_CENTER_LINE_2", bool(v))
        )

        self.line3_toggle.stateChanged.connect(
            lambda v: setattr(state, "USE_CENTER_LINE_3", bool(v))
        )



    # =====================================================
    # HELPERS
    # =====================================================

    # === WIERSZ: LABEL + WARTOŚĆ (OPCJONALNY TOOLTIP / LABEL RETURN) ===
    def _row(self, label_text, widget, tooltip_key=None, return_label=False):
        layout = QHBoxLayout()

        label = QLabel(label_text)

        if tooltip_key:
            help_icon = HelpIcon(TOOLTIPS[tooltip_key])

            label_container = QWidget()
            label_layout = QHBoxLayout(label_container)
            label_layout.setContentsMargins(0, 0, 0, 0)
            label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
            label_layout.addWidget(label)
            label_layout.addWidget(help_icon)

            layout.addWidget(label_container)
        else:
            layout.addWidget(label)

        layout.addStretch()
        layout.addWidget(widget)

        if return_label:
            return label, layout
        return layout

    # === WIERSZ: TOGGLE + OPIS ===
    def _toggle_row(self, label_text, toggle):
        layout = QHBoxLayout()
        label = QLabel(label_text)
        layout.addWidget(toggle)
        layout.addWidget(label)
        layout.addStretch()
        return layout
    
    # === FABRYKA POLA TEKSTOWEGO (WSPÓLNY WYGLĄD) ===
    def _text_input(self, tooltip_key: str):
        edit = ErrorTooltipLineEdit(tooltip_key)
        edit.setFixedWidth(100)
        edit.setProperty("class", "path-field")

        # === PREWALIDACJA: TYLKO CYFRY I KROPKI ===
        regex = QRegularExpression(r"[0-9.]*")
        validator = QRegularExpressionValidator(regex, edit)
        edit.setValidator(validator)

        return edit

    
    def _load_state_to_ui(self):
        self._clear_all_field_errors()
        
        # === OCR / WYDAJNOŚĆ ===
        self.ocr_quality.setText(str(state.RESOLUTION_DOWNSCALE))
        self.capture_interval.setText(str(int(state.CAPTURE_INTERVAL * 1000)))
        self.min_height.setText(str(state.MIN_HEIGHT))
        self.max_height.setText(str(state.MAX_HEIGHT))

        # === LINIE POMOCNICZE ===
        self.line_width.setText(str(state.CENTER_LINE_MARGIN))
        self.line_left_2.setText(str(state.CENTER_LINE_2_START))
        self.line_left_3.setText(str(state.CENTER_LINE_3_START_RATIO))

        # === LEKTOR / AUDIO ===
        self.tts_speed.setText(str(state.BASE_PLAYBACK_SPEED))
        self.tts_boost_speed.setText(str(state.OVERLAP_PLAYBACK_SPEED))
        self.game_ducking.setText(str(state.VOLUME_REDUCTION_LEVEL))
        self.memory_lines.setCurrentText(str(state.AUDIO_QUEUE_SIZE))

        # === PRZEŁĄCZNIKI ===
        self.typewriter_toggle.setChecked(state.ENABLE_TYPEWRITER_WAIT)
        self.paragraph_ocr_toggle.setChecked(state.ENABLE_PARAGRAPH_OCR)
        self.line1_toggle.setChecked(state.USE_CENTER_LINE_1)
        self.line2_toggle.setChecked(state.USE_CENTER_LINE_2)
        self.line3_toggle.setChecked(state.USE_CENTER_LINE_3)

        # SYNCHRO TRYBU AUDIO
        mode = 0 if state.ENABLE_DYNAMIC_SPEED else 1
        self.audio_mode_toggle.setValue(mode)

        self._audio_mode_changed(mode)
        self._update_audio_mode_labels(mode)
    
    # =====================================================
    # AUDIO: AKTUALIZACJA STANU UI (LABEL + FADE KOLORU)
    # =====================================================
    def _update_audio_mode_labels(self, value: int):
        active_label   = self.audio_dynamic_label if value == 0 else self.audio_static_label
        inactive_label = self.audio_static_label  if value == 0 else self.audio_dynamic_label

        active_label.setProperty("active", True)
        inactive_label.setProperty("active", False)
        for label in (active_label, inactive_label):
            label.style().unpolish(label)
            label.style().polish(label)

        self._animate_color(active_label,   QColor(TEXT_PRIMARY))
        self._animate_color(inactive_label, QColor(TEXT_MUTED))

    # === POMOCNICZA ANIMACJA KOLORU LABELA ===
    def _animate_color(self, label: QLabel, target: QColor):
        anim = self._color_animations[label]
        anim.stop()
        # Aktualny kolor z obecnego stylu
        current_hex = label.styleSheet().replace("color: ", "").replace(";", "").strip()
        current = QColor(current_hex) if current_hex and QColor(current_hex).isValid() else target
        if not self.isVisible():
            label.setStyleSheet(f"color: {target.name()};")
            return
        anim.setStartValue(current)
        anim.setEndValue(target)
        anim.start()

    # =====================================================
    # WALIDACJA POL
    # =====================================================
    def _set_field_error(self, field: QLineEdit, is_error: bool):
        field.setProperty("state", "error" if is_error else "")
        field.style().unpolish(field)
        field.style().polish(field)
        field.update()

    def _commit_int(self, field: QLineEdit, attr: str, min_v: int, max_v: int, *, transform=None) -> bool:
        raw = (field.text() or "").strip()

        if raw == "":
            self._set_field_error(field, True)
            return False

        if not raw.isdigit():
            self._set_field_error(field, True)
            return False

        val = int(raw)

        if not (min_v <= val <= max_v):
            self._set_field_error(field, True)
            return False

        self._set_field_error(field, False)
        out = transform(val) if transform else val
        setattr(state, attr, out)
        return True

    def _commit_float(self, field: QLineEdit, attr: str, min_v: float, max_v: float) -> bool:
        raw = (field.text() or "").strip()

        if raw == "":
            self._set_field_error(field, True)
            return False

        if not re.fullmatch(r"\d+(\.\d+)?", raw):
            self._set_field_error(field, True)
            return False

        if "." in raw:
            decimals = raw.split(".", 1)[1]
            if len(decimals) > 2:
                self._set_field_error(field, True)
                return False

        val = float(raw)

        if not (min_v <= val <= max_v):
            self._set_field_error(field, True)
            return False

        self._set_field_error(field, False)
        setattr(state, attr, val)
        return True

    def _on_min_max_height_changed(self):

        ok_min = self._commit_int(
            self.min_height,
            "MIN_HEIGHT",
            C.MIN_HEIGHT_MIN,
            C.MIN_HEIGHT_MAX,
        )
        ok_max = self._commit_int(
            self.max_height,
            "MAX_HEIGHT",
            C.MAX_HEIGHT_MIN,
            C.MAX_HEIGHT_MAX,
        )

        if not (ok_min and ok_max):
            return

        min_v = int(self.min_height.text().strip())
        max_v = int(self.max_height.text().strip())

        if min_v > max_v:
            self._set_field_error(self.min_height, True)
            self._set_field_error(self.max_height, True)

            self.min_height.setText(str(state.MIN_HEIGHT))
            self.max_height.setText(str(state.MAX_HEIGHT))
            return

        self._set_field_error(self.min_height, False)
        self._set_field_error(self.max_height, False)

        if not state.preset_path:
            state.runtime_base_resolution = state.resolution
            state.runtime_base_min_height = state.MIN_HEIGHT
            state.runtime_base_max_height = state.MAX_HEIGHT

    def _on_ocr_quality_changed(self):
        ok = self._commit_float(
            self.ocr_quality,
            "RESOLUTION_DOWNSCALE",
            C.RESOLUTION_DOWNSCALE_MIN,
            C.RESOLUTION_DOWNSCALE_MAX,
        )
        if not ok:
            return

        # Bez presetu: zmiana usera nadpisuje bazę runtime
        if not state.preset_path:
            state.runtime_base_resolution = state.resolution
            state.runtime_base_downscale = state.RESOLUTION_DOWNSCALE

    # =====================================================
    # BACKEND HOOKS
    # =====================================================
    # === ZMIANA TRYBU ODTWARZANIA AUDIO (DYNAMICZNY / STAŁY) ===
    def _audio_mode_changed(self, value: int):
        dynamic_enabled = (value == 0)
        state.ENABLE_DYNAMIC_SPEED = dynamic_enabled
        state.ENABLE_OUTPUT2_SYSTEM = not dynamic_enabled

        self.tts_speed.setEnabled(dynamic_enabled)
        self.tts_boost_speed.setEnabled(dynamic_enabled)

        # === STAN LABELI (QSS) ===
        for label in (self.tts_speed_label, self.tts_boost_label):
            label.setProperty(
                "state", "" if dynamic_enabled else "disabled"
            )
            label.style().unpolish(label)
            label.style().polish(label)

    # === ZMIANA LICZBY LINII DIALOGÓW PRZECHOWYWANYCH W PAMIĘCI ===
    def _memory_lines_changed(self, value: str):
        state.AUDIO_QUEUE_SIZE = int(value)
        self.memory_lines.clearFocus()

    # === WYCZYSZCZENIE STANU WALIDACJI DLA WSZYSTKICH PÓL ===
    def _clear_all_field_errors(self):
        fields = [
            self.ocr_quality,
            self.capture_interval,
            self.min_height,
            self.max_height,
            self.line_width,
            self.line_left_2,
            self.line_left_3,
            self.tts_speed,
            self.tts_boost_speed,
            self.game_ducking,
        ]

        for field in fields:
            field.setProperty("state", "")
            field.style().unpolish(field)
            field.style().polish(field)
            field.update()

    # === PUBLICZNE API: ODŚWIEŻENIE UI NA PODSTAWIE STATE ===
    def reload_ui_from_state(self):
        self._load_state_to_ui()

