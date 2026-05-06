
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtCore import Qt
from PySide6.QtWidgets import (
    QWidget, QLabel, QLineEdit, QPushButton,
    QVBoxLayout, QHBoxLayout, QGroupBox,
    QSizePolicy, QFileDialog
)

from ui.theme.theme import (
    TAB_MARGIN_H,
    TAB_MARGIN_V,
    TAB_SPACING,
    SPACE_MD,
    OPTION_LABEL_ICON_SPACING,
)

from ui.widgets import ToggleSwitch, HelpIcon, FocusClearingTab
from ui.tooltips import TOOLTIPS

from core import state
from presets import manager as presets

# ============================================================
# ZAKŁADKA: FOLDERY I PLIKI
# ============================================================
class FilesTab(FocusClearingTab):
    def __init__(self):
        super().__init__()

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(
            TAB_MARGIN_H,
            TAB_MARGIN_V,
            TAB_MARGIN_H,
            TAB_MARGIN_V
        )
        main_layout.setSpacing(TAB_SPACING)

        # === INFORMACJA ===
        info_label = QLabel(
            "Aby poprawnie korzystać z programu, przypisz poniższe ścieżki "
            "oraz wybierz rozdzielczość gry w zakładce Szybki start.\n"
            "Dopiero po wykonaniu tych czynności zapisz lub nadpisz swój preset."
        )
        info_label.setWordWrap(True)
        info_label.setProperty("class", "info")
        main_layout.addWidget(info_label)

        # === ŚCIEŻKI ===
        paths_group = QGroupBox("Ścieżki dostępu")
        paths_layout = QVBoxLayout(paths_group)
        paths_layout.setSpacing(12)

        # === OPCJE GLOBALNE DLA PLIKÓW ===
        remove_names_layout = QHBoxLayout()
        self.remove_names_switch = ToggleSwitch()

        remove_names_label = QLabel("Usuń nazwy postaci")
        remove_names_help = HelpIcon(TOOLTIPS["files_remove_character_names"])

        label_container = QWidget()
        label_layout = QHBoxLayout(label_container)
        label_layout.setContentsMargins(0, 0, 0, 0)
        label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
        label_layout.addWidget(remove_names_label)
        label_layout.addWidget(remove_names_help)

        remove_names_layout.addWidget(self.remove_names_switch)
        remove_names_layout.addWidget(label_container)
        remove_names_layout.addStretch()

        screenshots_layout = QHBoxLayout()
        self.save_screenshots_switch = ToggleSwitch()

        screenshots_label = QLabel("Zapisuj zrzuty ekranu")
        screenshots_help = HelpIcon(TOOLTIPS["files_save_screenshots"])

        label_container = QWidget()
        label_layout = QHBoxLayout(label_container)
        label_layout.setContentsMargins(0, 0, 0, 0)
        label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
        label_layout.addWidget(screenshots_label)
        label_layout.addWidget(screenshots_help)

        screenshots_layout.addWidget(self.save_screenshots_switch)
        screenshots_layout.addWidget(label_container)
        screenshots_layout.addStretch()

        paths_layout.addLayout(remove_names_layout)
        paths_layout.addLayout(screenshots_layout)

        separator = QWidget()
        separator.setProperty("class", "separator")

        paths_layout.addWidget(separator)

        # === DEFINICJA ŚCIEŻEK PLIKÓW ===
        self.audio_folder = self._build_path_row(
            "Folder plików audio",
            self._select_audio_folder,
            "files_audio_folder"
        )

        self.subtitles_file = self._build_path_row(
            "Plik z napisami (subtitles)",
            self._select_subtitles_file,
            "files_subtitles_file"
        )

        self.characters_file = self._build_path_row(
            "Plik z nazwami postaci",
            self._select_characters_file,
            "files_characters_file"
        )

        self.screenshots_folder = self._build_path_row(
            "Folder zrzutów ekranu",
            self._select_screenshots_folder,
            "files_screenshots_folder"
        )

        # === LOGIKA: ZALEŻNOŚCI PRZEŁĄCZNIKÓW ===
        self.remove_names_switch.stateChanged.connect(
            self._update_characters_file_state
        )
        self.save_screenshots_switch.stateChanged.connect(
            self._update_screenshots_folder_state
        )

        self.remove_names_switch.stateChanged.connect(
            lambda v: setattr(state, "ENABLE_REMOVE_CHARACTER_NAME", v)
        )

        self.save_screenshots_switch.stateChanged.connect(
            lambda v: setattr(state, "ENABLE_SCREENSHOTS", v)
        )

        # === STAN POCZĄTKOWY UI ===
        self._update_characters_file_state()
        self._update_screenshots_folder_state()

        paths_layout.addWidget(self.audio_folder)
        paths_layout.addWidget(self.subtitles_file)
        paths_layout.addWidget(self.characters_file)
        paths_layout.addWidget(self.screenshots_folder)

        main_layout.addWidget(paths_group)
        main_layout.addStretch()

        self._load_from_state()

    # =====================================================
    # HELPERS
    # =====================================================

    # === BUILDER: POJEDYNCZY WIERSZ ŚCIEŻKI ===
    def _build_path_row(self, label_text: str, callback, tooltip_key: str):
        container = QWidget()
        layout = QHBoxLayout(container)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(SPACE_MD)

        label = QLabel(label_text)
        help_icon = HelpIcon(TOOLTIPS[tooltip_key])

        label_container = QWidget()
        label_container.setFixedWidth(220)

        label_layout = QHBoxLayout(label_container)
        label_layout.setContentsMargins(0, 0, 0, 0)
        label_layout.setSpacing(OPTION_LABEL_ICON_SPACING)
        label_layout.addWidget(label)
        label_layout.addWidget(help_icon)
        label_layout.addStretch()

        # === POLE ŚCIEŻKI ===
        field = QLineEdit()
        field.setReadOnly(True)
        field.setPlaceholderText("nie ustawiono")
        field.setMinimumWidth(520)
        field.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Fixed)
        field.setProperty("class", "path-field")

        # === PRZYCISK ===
        button = QPushButton("Wybierz")
        button.setCursor(Qt.PointingHandCursor)
        button.setProperty("class", "preset")
        button.setProperty("size", "sm")
        button.clicked.connect(callback)

        # === LAYOUT ===
        layout.addWidget(label_container)
        layout.addWidget(field, 1)
        layout.addWidget(button)

        # === REFERENCJE POMOCNICZE ===
        container.field = field
        container.label = label
        container.button = button

        return container
    
    # === LOGIKA UI: PLIK NAZW POSTACI ===
    def _update_characters_file_state(self):
        enabled = self.remove_names_switch.isChecked()

        self.characters_file.setEnabled(enabled)
        self.characters_file.field.setEnabled(enabled)

        btn = self.characters_file.button
        btn.setEnabled(enabled)

        self.characters_file.label.setProperty(
            "state", "disabled" if not enabled else ""
        )
        self.characters_file.label.style().unpolish(self.characters_file.label)
        self.characters_file.label.style().polish(self.characters_file.label)

    # === LOGIKA UI: FOLDER ZRZUTÓW EKRANU ===
    def _update_screenshots_folder_state(self):
        enabled = self.save_screenshots_switch.isChecked()

        self.screenshots_folder.setEnabled(enabled)
        self.screenshots_folder.field.setEnabled(enabled)

        btn = self.screenshots_folder.button
        btn.setEnabled(enabled)

        self.screenshots_folder.label.setProperty(
            "state", "disabled" if not enabled else ""
        )
        self.screenshots_folder.label.style().unpolish(self.screenshots_folder.label)
        self.screenshots_folder.label.style().polish(self.screenshots_folder.label)

    # === ZAŁADOWANIE STANU APLIKACJI DO UI ===
    def _load_from_state(self):
        self.remove_names_switch.setChecked(
            bool(state.ENABLE_REMOVE_CHARACTER_NAME)
        )

        self.save_screenshots_switch.setChecked(
            bool(state.ENABLE_SCREENSHOTS)
        )

        self.audio_folder.field.setText(
            state.audio_dir or "nie ustawiono"
        )

        self.subtitles_file.field.setText(
            state.text_file_path or "nie ustawiono"
        )

        self.characters_file.field.setText(
            state.names_file_path or "nie ustawiono"
        )

        self.screenshots_folder.field.setText(
            state.screenshot_dir or "nie ustawiono"
        )

        self._update_characters_file_state()
        self._update_screenshots_folder_state()

    # =====================================================
    # BACKEND HOOKS
    # =====================================================
    # === WYBÓR FOLDERU AUDIO ===
    def _select_audio_folder(self):
        path = QFileDialog.getExistingDirectory(
            self,
            "Wybierz folder plików audio",
            state.audio_dir or ""
        )

        if not path:
            return

        state.audio_dir = path
        self.audio_folder.field.setText(path)

        presets.reload_dialogs_and_names(verbose=True)

    # === WYBÓR PLIKU DIALOGÓW ===
    def _select_subtitles_file(self):
        path, _ = QFileDialog.getOpenFileName(
            self,
            "Wybierz plik z napisami",
            state.text_file_path or "",
            "Pliki tekstowe (*.txt)"
        )

        if not path:
            return

        state.text_file_path = path
        self.subtitles_file.field.setText(path)

        presets.reload_dialogs_and_names(verbose=True)

    # === WYBÓR PLIKU Z NAZWAMI POSTACI ===
    def _select_characters_file(self):
        path, _ = QFileDialog.getOpenFileName(
            self,
            "Wybierz plik z nazwami postaci",
            state.names_file_path or "",
            "Pliki tekstowe (*.txt)"
        )

        if not path:
            return

        state.names_file_path = path
        self.characters_file.field.setText(path)

        presets.reload_dialogs_and_names(verbose=True)

    # === WYBÓR FOLDERU ZRZUTÓW EKRANU ===
    def _select_screenshots_folder(self):
        path = QFileDialog.getExistingDirectory(
            self,
            "Wybierz folder zrzutów ekranu",
            state.screenshot_dir or ""
        )

        if not path:
            return

        state.screenshot_dir = path
        self.screenshots_folder.field.setText(path)

    # === PUBLICZNE API: ODŚWIEŻENIE ZAKŁADKI ===
    def reload_from_state(self):
        self._load_from_state()

    def reload_ui_from_state(self):
        self._load_from_state()
