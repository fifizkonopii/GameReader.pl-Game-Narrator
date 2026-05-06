
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtCore import Qt
from PySide6.QtGui import QIntValidator, QGuiApplication
from PySide6.QtWidgets import (
    QWidget, QLabel, QLineEdit,
    QVBoxLayout, QHBoxLayout,
    QGroupBox, QPushButton,
    QSizePolicy
)

from ui.theme.theme import (
    TAB_MARGIN_H,
    TAB_MARGIN_V,
    TAB_SPACING,
    OPTION_LABEL_ICON_SPACING,
)

from ui.widgets import ToggleSwitch, HelpIcon, FocusClearingTab
from ui.tooltips import TOOLTIPS
from ui.overlay.screen_area_selector import ScreenAreaSelector

from core import state

# ============================================================
# MAPA ZMIENNYCH
# ============================================================
FIELD_NAME_MAP = {
    "górna krawędź": "top",
    "lewa krawędź": "left",
    "wysokość": "height",
    "szerokość": "width",
}

# ============================================================
# ZAKŁADKA: OBSZAR EKRANU
# ============================================================
class ScreenTab(FocusClearingTab):
    def __init__(self):
        super().__init__()

        # =====================================================
        # LAYOUT GŁÓWNY ZAKŁADKI
        # =====================================================
        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(
            TAB_MARGIN_H,
            TAB_MARGIN_V,
            TAB_MARGIN_H,
            TAB_MARGIN_V
        )
        main_layout.setSpacing(TAB_SPACING)

        # =====================================================
        # OBSZAR 1
        # =====================================================
        area1_group = QGroupBox("Ustawienie obszaru 1")
        area1_layout = QVBoxLayout(area1_group)
        area1_layout.setSpacing(12)

        self.area1_layout, self.area1_fields = self._build_area_inputs(1)
        area1_layout.addLayout(self.area1_layout)

        # === PRZYCISK WYBORU OBSZARU 1 ===
        area1_button_layout = QHBoxLayout()

        self.select_area1_btn = QPushButton("Zaznacz obszar 1")
        self.select_area1_btn.setCursor(Qt.PointingHandCursor)
        self.select_area1_btn.setProperty("class", "preset")
        self.select_area1_btn.clicked.connect(self._select_area1_requested)

        area1_button_layout.addWidget(self.select_area1_btn)
        area1_button_layout.addStretch()
        area1_layout.addLayout(area1_button_layout)

        # =====================================================
        # OBSZAR 2 (opcjonalny)
        # =====================================================
        area2_group = QGroupBox("Ustawienie obszaru 2")
        area2_layout = QVBoxLayout(area2_group)
        area2_layout.setSpacing(12)

        # === PRZEŁĄCZNIK AKTYWACJI OBSZARU 2 ===
        toggle_layout = QHBoxLayout()
        self.area2_enabled_switch = ToggleSwitch()

        toggle_label = QLabel("Aktywuj obszar 2")
        toggle_help = HelpIcon(TOOLTIPS["screen_area2_toggle"])

        label_container = QWidget()
        label_layout = QHBoxLayout(label_container)
        label_layout.setContentsMargins(0, 0, 0, 0)
        label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
        label_layout.addWidget(toggle_label)
        label_layout.addWidget(toggle_help)

        toggle_layout.addWidget(self.area2_enabled_switch)
        toggle_layout.addWidget(label_container)
        toggle_layout.addStretch()

        self.area2_enabled_switch.stateChanged.connect(
            self._update_area2_state
        )
        area2_layout.addLayout(toggle_layout)

        # === SEPARATOR ===
        separator = QWidget()
        separator.setProperty("class", "separator")
        area2_layout.addWidget(separator)

        self.area2_layout, self.area2_fields = self._build_area_inputs(2)
        area2_layout.addLayout(self.area2_layout)

        # === PRZYCISK WYBORU OBSZARU 2 ===
        area2_button_layout = QHBoxLayout()

        self.select_area2_btn = QPushButton("Zaznacz obszar 2")
        self.select_area2_btn.setCursor(Qt.PointingHandCursor)
        self.select_area2_btn.setProperty("class", "preset")
        self.select_area2_btn.clicked.connect(self._select_area2_requested)

        area2_button_layout.addWidget(self.select_area2_btn)
        area2_button_layout.addStretch()
        area2_layout.addLayout(area2_button_layout)
        self._update_area2_state()

        main_layout.addWidget(area1_group)
        main_layout.addWidget(area2_group)
        main_layout.addStretch()
        self._load_state_to_ui()

        self._area_selector = None

    # =====================================================
    # HELPERS
    # =====================================================
    def _get_selected_screen(self):
        screens = QGuiApplication.screens()

        idx = state.selected_screen_monitor - 1
        if 0 <= idx < len(screens):
            return screens[idx]

        # fallback bezpieczeństwa
        return QGuiApplication.primaryScreen()

    # === BUDOWA PÓL WEJŚCIOWYCH OBSZARU ===
    def _build_area_inputs(self, area_index: int):
        layout = QHBoxLayout()
        layout.setSpacing(12)

        inputs = {}

        for name, tooltip_key in (
            ("Górna krawędź", "screen_area_top"),
            ("Lewa krawędź", "screen_area_left"),
            ("Wysokość", "screen_area_height"),
            ("Szerokość", "screen_area_width"),
        ):
            box = QVBoxLayout()

            label = QLabel(name)
            help_icon = HelpIcon(TOOLTIPS[tooltip_key])

            label_container = QWidget()
            label_container.setSizePolicy(
                QSizePolicy.Maximum, QSizePolicy.Fixed
            )

            label_layout = QHBoxLayout(label_container)
            label_layout.setContentsMargins(0, 0, 0, 0)
            label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
            label_layout.addWidget(label)
            label_layout.addWidget(help_icon)

            field = QLineEdit()
            field.editingFinished.connect(
                lambda ai=area_index, n=name.lower(), f=field:
                    self._on_area_field_changed(ai, n, f)
            )
            field.setPlaceholderText("0")
            field.setFixedWidth(110)
            if name in ("Górna krawędź", "Lewa krawędź"):
                field.setValidator(QIntValidator(0, 100000, field))
            else:
                field.setValidator(QIntValidator(1, 100000, field))

            field.setProperty("class", "path-field")

            box.addWidget(label_container)
            box.addWidget(field)

            layout.addLayout(box)
            inputs[name.lower()] = {
                "field": field,
                "label": label
            }

        layout.addStretch()

        return layout, inputs

    # === AKTUALIZACJA STANU OBSZARU 2 (WŁ / WYŁ) ===
    def _update_area2_state(self):
        enabled = self.area2_enabled_switch.isChecked()
        state.monitor2_enabled = enabled

        for item in self.area2_fields.values():
            item["field"].setEnabled(enabled)
            item["label"].setProperty(
                "state", "disabled" if not enabled else ""
            )
            item["label"].style().unpolish(item["label"])
            item["label"].style().polish(item["label"])

        self.select_area2_btn.setEnabled(enabled)

    def _load_state_to_ui(self):
        # === OBSZAR 1 ===
        self.area1_fields["górna krawędź"]["field"].setText(str(state.monitor["top"]))
        self.area1_fields["lewa krawędź"]["field"].setText(str(state.monitor["left"]))
        self.area1_fields["wysokość"]["field"].setText(str(state.monitor["height"]))
        self.area1_fields["szerokość"]["field"].setText(str(state.monitor["width"]))

        # === OBSZAR 2 ===
        self.area2_enabled_switch.setChecked(state.monitor2_enabled)

        self.area2_fields["górna krawędź"]["field"].setText(str(state.monitor2_top))
        self.area2_fields["lewa krawędź"]["field"].setText(str(state.monitor2_left))
        self.area2_fields["wysokość"]["field"].setText(str(state.monitor2_height))
        self.area2_fields["szerokość"]["field"].setText(str(state.monitor2_width))

        # Odśwież dostępność pól
        self._update_area2_state()

    # === ZAPISYWANIE DO STATE ===
    def _on_area_field_changed(self, area_index: int, name: str, field: QLineEdit):
        try:
            value = int(field.text())
        except ValueError:
            return

        key = FIELD_NAME_MAP.get(name)
        if not key:
            return

        if area_index == 1:
            state.monitor[key] = value

        elif area_index == 2:
            setattr(state, f"monitor2_{key}", value)

        # === RUNTIME BAZA (TYLKO GDY NIE MA PRESETU) ===
        if not state.preset_path:
            state.runtime_base_resolution = state.resolution
            state.runtime_base_monitor = state.monitor.copy()

            if state.monitor2_enabled:
                state.runtime_base_monitor2 = {
                    "top": state.monitor2_top,
                    "left": state.monitor2_left,
                    "width": state.monitor2_width,
                    "height": state.monitor2_height,
                }
            else:
                state.runtime_base_monitor2 = None

    # =====================================================
    # BACKEND HOOKS
    # =====================================================
    # === ŻĄDANIE WYBORU OBSZARU 1 ===
    def _select_area1_requested(self):
        screen = self._get_selected_screen()

        self._minimize_main_window()

        self._area_selector = ScreenAreaSelector(
            screen=screen,
            initial_rect=state.monitor,
            area_index=1
        )

        self._area_selector.areaSelected.connect(self._apply_area1)
        self._area_selector.cancelled.connect(self._clear_area_selector)

    # === ZASTOSOWANIE WYBRANEGO OBSZARU 1 ===
    def _apply_area1(self, rect: dict):
        state.monitor = rect

        # === RUNTIME BAZA (TYLKO GDY NIE MA PRESETU) ===
        if not state.preset_path:
            state.runtime_base_resolution = state.resolution
            state.runtime_base_monitor = rect.copy()
            if state.monitor2_enabled:
                state.runtime_base_monitor2 = {
                    "top": state.monitor2_top,
                    "left": state.monitor2_left,
                    "width": state.monitor2_width,
                    "height": state.monitor2_height,
                }
            else:
                state.runtime_base_monitor2 = None

        self.window().reload_ui_from_state()

        self.area1_fields["górna krawędź"]["field"].setText(str(rect["top"]))
        self.area1_fields["lewa krawędź"]["field"].setText(str(rect["left"]))
        self.area1_fields["wysokość"]["field"].setText(str(rect["height"]))
        self.area1_fields["szerokość"]["field"].setText(str(rect["width"]))

        self._restore_main_window()
        self._area_selector = None

    # === ŻĄDANIE WYBORU OBSZARU 2 ===
    def _select_area2_requested(self):
        screen = self._get_selected_screen()

        initial = {
            "top": state.monitor2_top,
            "left": state.monitor2_left,
            "width": state.monitor2_width,
            "height": state.monitor2_height,
        }

        self._minimize_main_window()

        self._area_selector = ScreenAreaSelector(
            screen=screen,
            initial_rect=initial,
            area_index=2
        )

        self._area_selector.areaSelected.connect(self._apply_area2)
        self._area_selector.cancelled.connect(self._clear_area_selector)

    # === ZASTOSOWANIE WYBRANEGO OBSZARU 2 ===
    def _apply_area2(self, rect: dict):
        state.monitor2_top = rect["top"]
        state.monitor2_left = rect["left"]
        state.monitor2_width = rect["width"]
        state.monitor2_height = rect["height"]

        # === RUNTIME BAZA (TYLKO GDY NIE MA PRESETU) ===
        if not state.preset_path:
            state.runtime_base_resolution = state.resolution
            state.runtime_base_monitor = state.monitor.copy()
            state.runtime_base_monitor2 = rect.copy()

        self.area2_fields["górna krawędź"]["field"].setText(str(rect["top"]))
        self.area2_fields["lewa krawędź"]["field"].setText(str(rect["left"]))
        self.area2_fields["wysokość"]["field"].setText(str(rect["height"]))
        self.area2_fields["szerokość"]["field"].setText(str(rect["width"]))

        self._restore_main_window()
        self._area_selector = None

    # === WYCZYSZCZENIE REFERENCJI DO SELECTORA ===
    def _clear_area_selector(self):
        self._restore_main_window()
        self._area_selector = None

    # === PUBLICZNE API: ODŚWIEŻENIE ZAKŁADKI ===
    def reload_from_state(self):
        self._load_state_to_ui()

    # =====================================================
    # MINIMALIZOWANIE OKNA PRZY ZAZNACZANIU OBSZARU
    # =====================================================
    def _minimize_main_window(self):
        win = self.window()
        if win:
            win.showMinimized()

    def _restore_main_window(self):
        win = self.window()
        if not win:
            return

        # jeśli było zmaksymalizowane – wróć do max
        if win.isMaximized():
            win.showMaximized()
        else:
            win.showNormal()

        win.raise_()
        win.activateWindow()