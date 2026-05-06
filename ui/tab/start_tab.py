
# ============================================================
# IMPORTY
# ============================================================
import os
import json

from PySide6.QtCore import Qt, QObject, QThread, Signal
from PySide6.QtGui import QGuiApplication, QPixmap
from PySide6.QtWidgets import (
    QWidget, QLabel, QPushButton,
    QVBoxLayout, QHBoxLayout,
    QGroupBox, QFileDialog
)

from ui.theme.theme import (
    TAB_MARGIN_H,
    TAB_MARGIN_V,
    TAB_SPACING,
    BUTTON_GROUP_SPACING,
    OPTION_LABEL_ICON_SPACING,
)

from ui.widgets import (
    ToggleSwitch,
    IconComboBox,
    HelpIcon,
    NotificationDialog,
    FocusClearingTab,
    show_validation_result,
    RecentPresetsDialog,
)

from ui.tooltips import TOOLTIPS

from core import state, debug
from core import app as core_app
from core.state import get_selected_screen_geometry
from core.paths import asset_path
from core.constants import SUPPORTED_RESOLUTIONS

from core.resolution_scaling import recalculate_for_resolution
from ui.widgets import LegacyPresetDialog
from presets.manager import convert_legacy_preset_in_place, reload_dialogs_and_names

from presets import manager as presets


# ============================================================
# WORKER DO WCZYTYWANIA PRESETU (ASYNC)
# ============================================================
class PresetLoadWorker(QObject):
    finished = Signal(str, object)
    failed = Signal(str)

    def __init__(self, file_path: str):
        super().__init__()
        self.file_path = file_path

    def run(self):
        try:
            with open(self.file_path, "r", encoding="utf-8") as f:
                data = json.load(f)

            result = presets.load_preset_from_data(data, self.file_path)
            reload_dialogs_and_names(verbose=False)
            self.finished.emit(self.file_path, result)

        except Exception as e:
            self.failed.emit(str(e))

# ============================================================
# ZAKŁADKA: SZYBKI START
# ============================================================
class StartTab(FocusClearingTab):
    def __init__(self):
        super().__init__()
        self.setObjectName("StartTab")

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(
            TAB_MARGIN_H,
            TAB_MARGIN_V,
            TAB_MARGIN_H,
            TAB_MARGIN_V,
        )
        main_layout.setSpacing(TAB_SPACING)

        # === INFORMACJA ===
        info_label = QLabel(
            "Program GameReader działa najlepiej, gdy gra jest uruchomiona "
            "w trybie okienkowym lub w oknie bez ramek (bordless windows)."
        )
        info_label.setWordWrap(True)
        main_layout.addWidget(info_label)
        info_label.setProperty("class", "info")

        # === USTAWIENIA ROZDZIELCZOŚCI ===
        resolution_group = QGroupBox("Ustawienia rozdzielczości")
        resolution_layout = QVBoxLayout(resolution_group)
        resolution_layout.setSpacing(12)

        # === OPCJE GLOBALNE (skalowanie / blokada) ===
        options_container = QWidget()
        options_layout = QHBoxLayout(options_container)
        options_layout.setContentsMargins(6, 4, 6, 8)
        options_layout.setSpacing(10)

        self.lock_scaling_switch = ToggleSwitch()

        # === LABEL + HELP ICON ===
        lock_label = QLabel("Zablokuj przeliczanie")
        lock_help = HelpIcon(TOOLTIPS["lock_scaling"])

        label_container = QWidget()
        label_layout = QHBoxLayout(label_container)
        label_layout.setContentsMargins(0, 0, 0, 0)
        label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
        label_layout.addWidget(lock_label)
        label_layout.addWidget(lock_help)

        options_layout.addWidget(self.lock_scaling_switch)

        self.lock_scaling_switch.setChecked(state.lock_scaling)

        self.lock_scaling_switch.stateChanged.connect(
            lambda checked: setattr(state, "lock_scaling", checked)
        )

        options_layout.addWidget(label_container)
        options_layout.addStretch()

        # === SEPARATOR ===
        separator = QWidget()
        separator.setProperty("class", "separator")

        # === WYBÓR ROZDZIELCZOŚCI GRY ===
        res_layout = QHBoxLayout()

        res_label = QLabel("Wybierz rozdzielczość gry:")
        res_help = HelpIcon(TOOLTIPS["resolution"])

        res_label_container = QWidget()
        res_label_layout = QHBoxLayout(res_label_container)
        res_label_layout.setContentsMargins(0, 0, 0, 0)
        res_label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
        res_label_layout.addWidget(res_label)
        res_label_layout.addWidget(res_help)

        self.resolution_combo = IconComboBox()

        for key in SUPPORTED_RESOLUTIONS.keys():
            label = key.replace("x", " x ")
            self.resolution_combo.addItem(label)

        res_layout.addWidget(res_label_container)
        res_layout.addStretch()
        res_layout.addWidget(self.resolution_combo)

        # === WYBÓR MONITORA ===
        monitor_layout = QHBoxLayout()

        monitor_label = QLabel("Wybierz monitor (ekran):")
        monitor_help = HelpIcon(TOOLTIPS["monitor"])

        monitor_label_container = QWidget()
        monitor_label_layout = QHBoxLayout(monitor_label_container)
        monitor_label_layout.setContentsMargins(0, 0, 0, 0)
        monitor_label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
        monitor_label_layout.addWidget(monitor_label)
        monitor_label_layout.addWidget(monitor_help)

        self.monitor_combo = IconComboBox()

        for idx, screen in enumerate(QGuiApplication.screens(), start=1):
            geo = screen.geometry()
            # FIX: HIGH DPI SCALING
            scale = screen.devicePixelRatio()
            w = int(geo.width() * scale)
            h = int(geo.height() * scale)
            self.monitor_combo.addItem(
                f"Monitor {idx} ({w}x{h})"
            )

        monitor_layout.addWidget(monitor_label_container)
        monitor_layout.addStretch()
        monitor_layout.addWidget(self.monitor_combo)

        resolution_layout.addWidget(options_container)
        resolution_layout.addWidget(separator)
        resolution_layout.addLayout(res_layout)
        resolution_layout.addLayout(monitor_layout)

        # =====================================================
        # PRESETY
        # =====================================================
        preset_group = QGroupBox("Zarządzanie presetami")
        preset_layout = QVBoxLayout(preset_group)
        preset_layout.setSpacing(12)

        # === AKTUALNIE AKTYWNY PRESET ===
        active_layout = QHBoxLayout()
        active_label = QLabel("Aktywny preset:")
        self.active_preset_value = QLabel("brak")
        self.active_preset_value.setProperty("class", "preset-active")

        active_layout.addWidget(active_label)
        active_layout.addWidget(self.active_preset_value)
        active_layout.addStretch()

        # === AKCJE PRESETÓW ===
        buttons_layout = QHBoxLayout()
        buttons_layout.setSpacing(BUTTON_GROUP_SPACING)

        self.load_preset_btn = QPushButton("Wczytaj preset")
        self.reload_preset_btn = QPushButton("Przeładuj preset")
        self.save_preset_btn = QPushButton("Zapisz aktywny preset")
        self.recent_presets_btn = QPushButton("Ostatnio używane")

        for btn in (
            self.load_preset_btn,
            self.reload_preset_btn,
            self.save_preset_btn,
            self.recent_presets_btn,
        ):
            btn.setCursor(Qt.PointingHandCursor)
            btn.setProperty("class", "preset")
            buttons_layout.addWidget(btn)

        self.load_preset_btn.clicked.connect(self._load_preset_requested)
        self.reload_preset_btn.clicked.connect(self._reload_preset_requested)
        self.save_preset_btn.clicked.connect(self._save_preset_requested)
        self.recent_presets_btn.clicked.connect(self._open_recent_presets)

        buttons_layout.addStretch()

        preset_layout.addLayout(active_layout)
        preset_layout.addSpacing(15)
        preset_layout.addLayout(buttons_layout)

        main_layout.addWidget(resolution_group)
        main_layout.addWidget(preset_group)

        # =====================================================
        # LOGO
        # =====================================================
        logo_label = QLabel()
        logo_label.setAlignment(Qt.AlignCenter)

        logo_path = asset_path("images", "logo2.png")

        pixmap = QPixmap(logo_path)

        if pixmap.isNull():
            logo_label.setText("❌ LOGO NIE ZAŁADOWANE")
        else:
            logo_label.setPixmap(
                pixmap.scaled(
                    275, 103,
                    Qt.KeepAspectRatio,
                    Qt.SmoothTransformation
                )
            )

        main_layout.addWidget(logo_label)
        main_layout.addStretch()

        self._load_state_to_ui()

        self.resolution_combo.currentTextChanged.connect(
            self._resolution_changed
        )

        self.monitor_combo.currentIndexChanged.connect(
            self._monitor_changed
        )
        
    # === CALLBACK: ZMIANA ROZDZIELCZOŚCI ===
    def _resolution_changed(self, text: str):
        new_resolution = text.replace(" ", "")
        recalculate_for_resolution(new_resolution)
        state.resolution = new_resolution
        self.window().reload_ui_from_state()

        # === NATYCHMIAST ODBIERZ FOCUS ===
        self.resolution_combo.clearFocus()

    # === CALLBACK: ZMIANA MONITORA ===
    def _monitor_changed(self, index: int):
        state.selected_screen_monitor = index + 1
        state.refresh_selected_mss_monitor_rect()

        geo = get_selected_screen_geometry()

        debug.log(
            debug.INFO,
            "SCREEN",
            f"Wybrano monitor {state.selected_screen_monitor} | "
            f"geometry=({geo.left()},{geo.top()},{geo.width()}x{geo.height()})"
        )

        self.window().reload_ui_from_state()
        self.monitor_combo.clearFocus()

    # === ZAŁADOWANIE STANU APLIKACJI DO UI ===
    def _load_state_to_ui(self):
        if state.resolution:
            res_text = state.resolution.replace("x", " x ")
            idx = self.resolution_combo.findText(res_text)
            if idx >= 0:
                self.resolution_combo.setCurrentIndex(idx)

        monitor_index = max(0, state.selected_screen_monitor - 1)
        if monitor_index < self.monitor_combo.count():
            self.monitor_combo.setCurrentIndex(monitor_index)

        if state.preset_filename:
            name = state.preset_filename

            if getattr(state, "_legacy_mode", False):
                name += " (Legacy Mode)"

            self.active_preset_value.setText(name)
        else:
            self.active_preset_value.setText("brak")

        self._update_reload_preset_button_state()

    # === AKTUALIZACJA STANU PRZYCISKU „PRZEŁADUJ PRESET” ===
    def _update_reload_preset_button_state(self):
        has_preset = bool(state.preset_path)
        self.reload_preset_btn.setEnabled(has_preset)

    # =====================================================
    # BACKEND HOOKS
    # =====================================================

    # === WCZYTANIE PRESETU Z PLIKU (ASYNC + SPINNER) ===
    def _load_preset_from_path(self, file_path: str):

        # === ZABEZPIECZENIE PRZED WIELOKROTNYM KLIKNIĘCIEM PRZYCISKU WCZYTYWANIA PRESETU ===
        if getattr(self, "_preset_thread", None):
            return
        
        win = self.window()
        if win and hasattr(win, "show_loading"):
            win.show_loading("Wczytywanie presetu…")

        # === ASYNC LOAD PRESET ===
        self._preset_thread = QThread(self)
        self._preset_worker = PresetLoadWorker(file_path)
        self._preset_worker.moveToThread(self._preset_thread)

        # === START PRACY WORKERA PO ODPOWIEDNIM ODPAKOWANIU WĄTKU ===
        self._preset_thread.started.connect(self._preset_worker.run)

        # === OBSŁUGA WYNIKU PRACY WORKERA ===
        self._preset_worker.finished.connect(self._on_preset_loaded)
        self._preset_worker.failed.connect(self._on_preset_load_failed)

        # === SPRZĄTANIE WĄTKU I OBIEKTÓW ===
        self._preset_worker.finished.connect(self._preset_thread.quit)
        self._preset_worker.failed.connect(self._preset_thread.quit)
        self._preset_worker.finished.connect(self._preset_worker.deleteLater)
        self._preset_worker.failed.connect(self._preset_worker.deleteLater)
        self._preset_thread.finished.connect(self._preset_thread.deleteLater)
        self._preset_thread.finished.connect(lambda: setattr(self, "_preset_thread", None))

        self._preset_thread.start()

    def _on_preset_load_failed(self, msg: str):
        win = self.window()
        if win and hasattr(win, "hide_loading"):
            win.hide_loading()

        NotificationDialog(
            message=f"Nie udało się wczytać presetu:\n{msg}",
            kind=NotificationDialog.ERROR_TYPE,
            parent=self
        ).exec()

    def _on_preset_loaded(self, file_path: str, result):
        win = self.window()
        if win and hasattr(win, "hide_loading"):
            win.hide_loading()

        # === LEGACY PRESET (< 0.9.3) ===
        if getattr(result, "legacy_type", None) == "C":
            dlg = LegacyPresetDialog(self)
            dlg.exec()

            if dlg.convert:
                convert_legacy_preset_in_place(file_path)
                self._load_preset_from_path(file_path)
                return

        if not show_validation_result(
            result,
            parent=self,
            error_context="Nie można wczytać presetu.",
            warning_context="Preset został wczytany z ostrzeżeniem."
        ):
            return

        state.preset_path = file_path
        state.preset_filename = os.path.basename(file_path)

        core_app.update_tray_status()
        self._update_reload_preset_button_state()
        self.window().reload_ui_from_state()
        self._load_state_to_ui()

    # === ŻĄDANIE WCZYTANIA PRESETU ===
    def _load_preset_requested(self):
        file_path, _ = QFileDialog.getOpenFileName(
            self,
            "Wczytaj preset",
            "",
            "Preset (*.json)"
        )
        if not file_path:
            return

        self._load_preset_from_path(file_path)

    # === ŻĄDANIE OTWARCIA OSTATNIO UŻYWANYCH PRESETÓW ===
    def _open_recent_presets(self):
        dlg = RecentPresetsDialog(self.window())
        dlg.loadRequested.connect(self._load_preset_from_path)
        dlg.exec()

    # === ŻĄDANIE PRZEŁADOWANIA PRESETU ===
    def _reload_preset_requested(self):
        if not state.preset_path:
            return
        self._load_preset_from_path(state.preset_path)

    # === ŻĄDANIE ZAPISU PRESETU ===
    def _save_preset_requested(self):
        file_path, _ = QFileDialog.getSaveFileName(
            self,
            "Zapisz preset",
            "",
            "Preset (*.json)"
        )

        if not file_path:
            return

        if not file_path.lower().endswith(".json"):
            file_path += ".json"

        # === ZAPIS PRESETU + WALIDACJA (SOFT) ===
        try:
            result = presets.save_preset(file_path)

        except Exception as e:
            NotificationDialog(
                f"Nie udało się zapisać presetu:\n{e}",
                kind=NotificationDialog.ERROR_TYPE,
                parent=self
            ).exec()
            return

        if result:
            ok = show_validation_result(
                result,
                parent=self,
                error_context="Nie można zapisać presetu.",
                warning_context="Preset zapisany z ostrzeżeniem."
            )
            if not ok:
                return

        state.preset_path = file_path
        state.preset_filename = file_path.split("/")[-1]
        self.active_preset_value.setText(state.preset_filename)
        self._update_reload_preset_button_state()
        core_app.update_tray_status()

    # === ODŚWIEŻENIE ZAKŁADKI NA PODSTAWIE STATE ===
    def reload_from_state(self):
        self._load_state_to_ui()