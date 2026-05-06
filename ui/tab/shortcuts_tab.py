
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtCore import Qt, Signal
from PySide6.QtWidgets import (
    QWidget, QLabel, QFrame, QPushButton,
    QVBoxLayout, QHBoxLayout, QGroupBox
)

from ui.theme.theme import (
    TAB_MARGIN_H,
    TAB_MARGIN_V,
    TAB_SPACING,
    BUTTON_GROUP_SPACING,
    OPTION_LABEL_ICON_SPACING,
)

from ui.tooltips import TOOLTIPS
from ui.widgets import HelpIcon
from ui.keymap import normalize_qt_sequence, pretty_shortcut

from core import state
from core.constants import DEFAULT_KEY_BINDINGS
from input import hotkeys


# ============================================================
# WIDGET: POLE SKRÓTU
# ============================================================
class ShortcutField(QLabel):
    clicked = Signal()

    # === INICJALIZACJA POLA SKRÓTU ===
    def __init__(self, action_key: str):
        super().__init__("brak")
        self.action_key = action_key
        self.shortcut = None
        self.setAlignment(Qt.AlignCenter)
        self.setFixedWidth(160)
        self.setCursor(Qt.PointingHandCursor)
        self.setProperty("class", "shortcut-field")
        self.setProperty("state", "")

    # === ZDARZENIE QT: KLIKNIĘCIE POLA ===
    def mousePressEvent(self, event):
        if event.button() == Qt.LeftButton:
            self.clicked.emit()
        event.accept()

    # === USTAWIENIE SKRÓTU ===
    def set_shortcut(self, seq: str | None):
        self.shortcut = seq
        self.setText(pretty_shortcut(seq))
        self.set_conflict(False)

    # === OZNACZENIE KONFLIKTU SKRÓTU ===
    def set_conflict(self, state: bool):
        if state and self.shortcut:
            self.setText(f"⚠️ {pretty_shortcut(self.shortcut)}")
            self.setProperty("state", "conflict")
        else:
            # konflikt zniknął -> przywróć normalny tekst
            if self.shortcut:
                self.setText(pretty_shortcut(self.shortcut))
            else:
                self.setText("brak")
            self.setProperty("state", "")

        self.style().unpolish(self)
        self.style().polish(self)
        self.update()

# ============================================================
# OVERLAY: PRZECHWYTYWANIE SKRÓTU
# ============================================================
class ShortcutCaptureOverlay(QFrame):
    shortcutCaptured = Signal(str)

    # === INICJALIZACJA OVERLAYA ===
    def __init__(self, parent):
        super().__init__(parent)

        self.setAttribute(Qt.WA_DeleteOnClose)
        self.setFocusPolicy(Qt.StrongFocus)
        self.setProperty("class", "shortcut-overlay")

        layout = QVBoxLayout(self)
        layout.setAlignment(Qt.AlignCenter)

        label = QLabel("Programuj skrót klawiszowy\n\nESC – anuluj")
        label.setAlignment(Qt.AlignCenter)
        layout.addWidget(label)

        self.grabKeyboard()
        hotkeys.suspend()

    # === OBSŁUGA WCIŚNIĘCIA KLAWISZA ===
    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Escape:
            self.releaseKeyboard()
            hotkeys.resume()
            self.close()
            return

        # === IGNORUJ SAME MODYFIKATORY ===
        if event.key() in (
            Qt.Key_Control,
            Qt.Key_Shift,
            Qt.Key_Alt,
            Qt.Key_Meta,
            Qt.Key_CapsLock,
            Qt.Key_NumLock,
            Qt.Key_ScrollLock,
        ):
            return

        mods = []
        if event.modifiers() & Qt.ControlModifier:
            mods.append("ctrl")
        if event.modifiers() & Qt.AltModifier:
            mods.append("alt")
        if event.modifiers() & Qt.ShiftModifier:
            mods.append("shift")

        key_name = None

        try:
            vk = int(event.nativeVirtualKey())
        except Exception:
            vk = 0

        if 0x41 <= vk <= 0x5A:
            key_name = chr(ord("a") + (vk - 0x41))

        elif 0x30 <= vk <= 0x39:
            key_name = chr(ord("0") + (vk - 0x30))

        else:
            k = event.key()

            if Qt.Key_A <= k <= Qt.Key_Z:
                key_name = chr(ord("a") + (k - Qt.Key_A))
            elif Qt.Key_0 <= k <= Qt.Key_9:
                key_name = chr(ord("0") + (k - Qt.Key_0))
            elif Qt.Key_F1 <= k <= Qt.Key_F12:
                key_name = f"f{k - Qt.Key_F1 + 1}"
            elif k == Qt.Key_Home:
                key_name = "home"
            elif k == Qt.Key_End:
                key_name = "end"
            elif k == Qt.Key_Insert:
                key_name = "insert"
            elif k == Qt.Key_Delete:
                key_name = "delete"
            elif k == Qt.Key_PageUp:
                key_name = "page_up"
            elif k == Qt.Key_PageDown:
                key_name = "page_down"
            else:
                shifted_symbol_to_digit = {
                    Qt.Key_Exclam: "1",
                    Qt.Key_At: "2",
                    Qt.Key_NumberSign: "3",
                    Qt.Key_Dollar: "4",
                    Qt.Key_Percent: "5",
                    Qt.Key_AsciiCircum: "6",
                    Qt.Key_Ampersand: "7",
                    Qt.Key_Asterisk: "8",
                    Qt.Key_ParenLeft: "9",
                    Qt.Key_ParenRight: "0",
                }
                if k in shifted_symbol_to_digit:
                    key_name = shifted_symbol_to_digit[k]

        if not key_name:
            return

        seq = "+".join(mods + [key_name])

        self.releaseKeyboard()
        self.shortcutCaptured.emit(seq)
        hotkeys.resume()
        self.close()

    # === ZDARZENIE QT: ZAMKNIĘCIE OVERLAYA ===
    def closeEvent(self, event):
        hotkeys.resume()
        super().closeEvent(event)

# ============================================================
# ZAKŁADKA: SKRÓTY
# ============================================================
class ShortcutsTab(QWidget):
    def __init__(self):
        super().__init__()
        self.shortcut_fields = {}

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(
            TAB_MARGIN_H, TAB_MARGIN_V,
            TAB_MARGIN_H, TAB_MARGIN_V
        )
        main_layout.setSpacing(TAB_SPACING)

        # === GŁÓWNE SKRÓTY ===
        main_group = QGroupBox("Główne skróty klawiszowe")
        main_layout_group = QVBoxLayout(main_group)
        main_layout_group.setSpacing(10)

        main_shortcuts = [
            ("Włącz / Wyłącz lektora", "toggle_reader"),
            ("Zwiększ głośność lektora", "volume_up"),
            ("Zmniejsz głośność lektora", "volume_down"),
            ("Przerwij kwestię lektora", "interrupt_audio"),
            ("Tekst lektora", "test_sound"),
            ("Prędkość lektora + 0.01", "base_speed_up"),
            ("Prędkość lektora - 0.01", "base_speed_down"),
            ("Prędkość przyśpieszona + 0.01", "overlap_speed_up"),
            ("Prędkość przyśpieszona - 0.01", "overlap_speed_down"),
        ]

        for label, key in main_shortcuts:
            main_layout_group.addLayout(self._build_row(label, key))

        # === NAWIGACJA ===
        nav_group = QGroupBox("Nawigacja i inne")
        nav_layout = QVBoxLayout(nav_group)
        nav_layout.setSpacing(10)

        nav_shortcuts = [
            ("Pokaż / Ukryj obszary", "toggle_areas"),
            ("Przełącz obszar", "switch_monitor_toggle"),
            ("Wróć do okna ustawień", "open_settings"),
            ("Konsola debug", "debug_console"),
        ]

        for label, key in nav_shortcuts:
            nav_layout.addLayout(self._build_row(label, key))

        main_layout.addWidget(main_group)
        main_layout.addWidget(nav_group)

        # === PRZYCISKI ===
        clear_btn = QPushButton("Resetuj")
        clear_btn.setCursor(Qt.PointingHandCursor)
        clear_btn.setProperty("class", "preset")
        clear_btn.clicked.connect(self._clear_shortcuts_requested)

        reset_btn = QPushButton("Przywróć domyślne")
        reset_btn.setCursor(Qt.PointingHandCursor)
        reset_btn.setProperty("class", "preset")
        reset_btn.clicked.connect(self._reset_shortcuts_requested)

        reset_outer = QHBoxLayout()
        reset_inner = QHBoxLayout()

        reset_inner.setSpacing(BUTTON_GROUP_SPACING)
        reset_inner.addWidget(clear_btn)
        reset_inner.addWidget(reset_btn)

        reset_outer.addStretch()
        reset_outer.addLayout(reset_inner)

        main_layout.addLayout(reset_outer)
        main_layout.addStretch()

        self._load_shortcuts_from_state()

    # =====================================================
    # HELPERY
    # =====================================================

    # === BUDOWA JEDNEGO WIERSZA SKRÓTU ===
    def _build_row(self, text: str, action_key: str):
        row = QHBoxLayout()
        row.setSpacing(BUTTON_GROUP_SPACING)

        label = QLabel(text)
        help_icon = HelpIcon(TOOLTIPS.get(action_key, ""))

        label_box = QHBoxLayout()
        label_box.addWidget(label)
        label_box.addWidget(help_icon)

        field = ShortcutField(action_key)
        self.shortcut_fields[action_key] = field
        field.clicked.connect(lambda f=field: self._start_capture(f))

        row.addLayout(label_box)
        row.addStretch()
        row.addWidget(field)
        return row

    # === ROZPOCZĘCIE PRZECHWYTYWANIA SKRÓTU ===
    def _start_capture(self, field: ShortcutField):
        overlay = ShortcutCaptureOverlay(self)
        overlay.setGeometry(self.rect())
        overlay.shortcutCaptured.connect(
            lambda seq: self._assign_shortcut(field, seq)
        )
        overlay.show()

    # === PRZYPISANIE SKRÓTU DO AKCJI ===
    def _assign_shortcut(self, field: ShortcutField, seq: str):
        action = field.action_key
        normalized = normalize_qt_sequence(seq)

        state.key_bindings[action] = normalized
        field.set_shortcut(normalized)

        self._update_conflicts()

        hotkeys.stop_listener()
        hotkeys.start_listener()

    # === PRZYWRÓCENIE DOMYŚLNYCH SKRÓTÓW ===
    def _reset_shortcuts_requested(self):
        state.key_bindings = DEFAULT_KEY_BINDINGS.copy()
        self._load_shortcuts_from_state()

        hotkeys.stop_listener()
        hotkeys.start_listener()

    # === WYCZYSZCZENIE WSZYSTKICH SKRÓTÓW ===
    def _clear_shortcuts_requested(self):
        for action in list(state.key_bindings.keys()):
            state.key_bindings[action] = None

        for field in self.shortcut_fields.values():
            field.set_shortcut(None)
            field.set_conflict(False)

        self._update_conflicts()
        hotkeys.stop_listener()
        hotkeys.start_listener()

    # === WCZYTANIE SKRÓTÓW ZE STANU APLIKACJI ===
    def _load_shortcuts_from_state(self):
        for k, field in self.shortcut_fields.items():
            field.set_shortcut(
                normalize_qt_sequence(state.key_bindings.get(k))
            )

        self._update_conflicts()

    # === WYKRYWANIE KONFLIKTÓW SKRÓTÓW ===
    def _update_conflicts(self):
        reverse = {}

        for action, field in self.shortcut_fields.items():
            seq = field.shortcut
            if not seq:
                continue
            reverse.setdefault(seq, []).append(field)

        for fields in reverse.values():
            conflict = len(fields) > 1
            for field in fields:
                field.set_conflict(conflict)

    # === PUBLICZNE API: ODŚWIEŻENIE ZAKŁADKI ===
    def reload_from_state(self):
        self._load_shortcuts_from_state()
        hotkeys.stop_listener()
        hotkeys.start_listener()