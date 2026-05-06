
# ============================================================
# IMPORTY
# ============================================================
import os
import time

import qtawesome as qta
from PySide6.QtCore import (
    Qt, Signal, QRect, QRectF,
    QPropertyAnimation, QParallelAnimationGroup,
    QEasingCurve, QPoint, Property
)
from PySide6.QtGui import (
    QPainter, QColor, QPen,
    QPainterPath, QRegion,
    QIcon, QFontMetrics, QPixmap
)
from PySide6.QtWidgets import (
    QWidget, QLabel, QLineEdit,
    QComboBox, QStyledItemDelegate, QStyle,
    QGraphicsDropShadowEffect, QGraphicsOpacityEffect,
    QStackedWidget, QApplication,
    QVBoxLayout, QHBoxLayout,
    QDialog, QListWidget, QListWidgetItem,
    QPushButton, QProgressBar, QFrame
)

from core import debug
from core.paths import asset_path
from presets import manager as presets
from audio import player as audio_player

from ui.tooltips import TOOLTIPS
from ui.theme.theme import (
    ACCENT, ACCENT_DARK,
    BG_APP, BG_PANEL, BG_HOVER,
    TEXT_PRIMARY, TEXT_MUTED,
    BORDER, WHITE,
    FONT_SMALL, RADIUS_LG, TITLEBAR_HEIGHT,
    COMBO_ARROW_OFFSET,
    TOOLTIP_SOFT_WRAP_CHARS,
    BUTTON_GROUP_SPACING,
    SPACE_SM, SPACE_XS, SPACE_MD,
    OPTION_LABEL_ICON_SPACING,
    FONT_WEIGHT_SEMIBOLD,
    SUCCESS, DANGER, WARNING, INFO
)

# ============================================================
# BASE WIDGET: FocusClearingTab
# ============================================================
class FocusClearingTab(QWidget):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setFocusPolicy(Qt.StrongFocus)

    def showEvent(self, event):
        super().showEvent(event)
        self.setFocus(Qt.OtherFocusReason)

    def mousePressEvent(self, event):
        self.setFocus(Qt.OtherFocusReason)
        super().mousePressEvent(event)


# ============================================================
# ANIMATED STACKED WIDGET (slide + fade przy zmianie zakładki)
# ============================================================
class AnimatedStackedWidget(QWidget):
    DURATION = 410  # ms

    def __init__(self, parent=None):
        super().__init__(parent)
        inner = QVBoxLayout(self)
        inner.setContentsMargins(0, 0, 0, 0)
        inner.setSpacing(0)

        self._stack = QStackedWidget()
        inner.addWidget(self._stack)

        self._overlay = QLabel(self)
        self._overlay.setAttribute(Qt.WA_TransparentForMouseEvents)
        self._overlay.hide()

        self._busy    = False
        self._grp     = None
        self._fin_anim = None

    # --- publiczne API (kompatybilne z QStackedWidget) ---
    def addWidget(self, w):
        self._stack.addWidget(w)

    def currentIndex(self):
        return self._stack.currentIndex()

    def currentWidget(self):
        return self._stack.currentWidget()

    def setCurrentIndex(self, idx):
        self._stack.setCurrentIndex(idx)

    def resizeEvent(self, event):
        super().resizeEvent(event)


# ============================================================
# APP FRAME (zaokrąglone rogi + border)
# ============================================================
class AppFrame(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("app-frame")

    def resizeEvent(self, event):
        super().resizeEvent(event)


# ============================================================
# DIALOG TITLE BAR (okna pomocnicze)
# ============================================================
class DialogTitleBar(QWidget):
    def __init__(self, title: str, window: QWidget):
        super().__init__(window)
        self.setObjectName("simple-titlebar")
        self.setFixedHeight(TITLEBAR_HEIGHT)
        self._drag_pos = None
        self._window = window

        layout = QHBoxLayout(self)
        layout.setContentsMargins(16, 0, 4, 0)
        layout.setSpacing(0)

        title_lbl = QLabel(title)
        title_lbl.setObjectName("titlebar-title")

        layout.addWidget(title_lbl)
        layout.addStretch()

        self.min_btn   = self._make_btn("mdi.window-minimize", "titlebar-btn")
        self.close_btn = self._make_btn("mdi.window-close",    "titlebar-close-btn")

        self.min_btn.clicked.connect(window.showMinimized)
        self.close_btn.clicked.connect(window.close)

        layout.addWidget(self.min_btn)
        layout.addWidget(self.close_btn)

    def _make_btn(self, icon_name: str, obj_name: str) -> QPushButton:
        btn = QPushButton()
        btn.setObjectName(obj_name)
        btn.setIcon(qta.icon(icon_name, color=TEXT_MUTED))
        btn.setFixedSize(40, TITLEBAR_HEIGHT - 4)
        btn.setCursor(Qt.ArrowCursor)
        return btn

    def mousePressEvent(self, event):
        if event.button() == Qt.LeftButton:
            event.accept()
            self._window.windowHandle().startSystemMove()
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        super().mouseReleaseEvent(event)


# ============================================================
# WIDGET: ToggleSwitch
# ============================================================
class ToggleSwitch(QWidget):
    stateChanged = Signal(bool)

    # === INICJALIZACJA WIDGETU ===
    def __init__(self, parent=None):
        super().__init__(parent)

        self.setCursor(Qt.PointingHandCursor)
        self.setFixedSize(36, 18)

        self._checked = False
        self._offset = 2

        self._on_color = QColor(ACCENT)
        self._off_color = QColor(BORDER)
        self._knob_color = QColor("#ffffff")

        self._animation = QPropertyAnimation(self, b"offset", self)
        self._animation.setDuration(160)

    # === PROPERTY: OFFSET GAŁKI (ANIMACJA) ===
    def getOffset(self):
        return self._offset
    
    # === PROPERTY: OFFSET GAŁKI (ANIMACJA) ===
    def setOffset(self, value):
        self._offset = value
        self.update()

    offset = Property(float, getOffset, setOffset)

    # === API PUBLICZNE ===
    def isChecked(self) -> bool:
        return self._checked

    # === API PUBLICZNE ===
    def setChecked(self, state: bool):
        if self._checked != state:
            self._checked = state
            self._start_animation()
            self.stateChanged.emit(state)

    # === ZDARZENIE QT: ZWOLNIENIE PRZYCISKU MYSZY ===
    def mouseReleaseEvent(self, event):
        if event.button() == Qt.LeftButton:
            self._checked = not self._checked
            self._start_animation()
            self.stateChanged.emit(self._checked)
            event.accept()
        else:
            event.ignore()

    # === LOGIKA WEWNĘTRZNA: ANIMACJA PRZEŁĄCZNIKA ===
    def _start_animation(self):
        self._animation.stop()
        end = self.width() - 14 if self._checked else 2
        self._animation.setStartValue(self._offset)
        self._animation.setEndValue(end)
        self._animation.start()

    # === RYSOWANIE WIDGETU ===
    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.Antialiasing)

        bg_color = self._on_color if self._checked else self._off_color
        painter.setBrush(bg_color)
        painter.setPen(Qt.NoPen)

        painter.drawRoundedRect(
            QRectF(0, 0, self.width(), self.height()),
            self.height() / 2,
            self.height() / 2
        )

        painter.setBrush(self._knob_color)
        painter.drawEllipse(QRectF(self._offset, 2, 14, 14))

        painter.end()

    def closeEvent(self, event):
        if self._animation:
            self._animation.stop()
        super().closeEvent(event)

# ============================================================
# WIDGET: IconComboBox
# ============================================================
class IconComboBox(QComboBox):
    # === INICJALIZACJA WIDGETU ===
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setSizeAdjustPolicy(QComboBox.AdjustToContents)

        self._arrow_icon = qta.icon(
            "fa5s.chevron-down",
            color=TEXT_PRIMARY
        )

    # === RYSOWANIE IKONY DROPDOWN ===
    def paintEvent(self, event):
        super().paintEvent(event)

        painter = QPainter(self)
        rect = self.rect()

        size = 12
        margin = 10

        icon_rect = QRect(
            rect.right() - COMBO_ARROW_OFFSET,
            rect.center().y() - size // 2,
            size,
            size
        )

        self._arrow_icon.paint(painter, icon_rect)
        painter.end()

    # === ZDARZENIE QT: OTWARCIE POPUPU ===
    def showPopup(self):
        super().showPopup()

        popup = self.view().window()

        if not popup.graphicsEffect():
            shadow = QGraphicsDropShadowEffect(popup)
            shadow.setBlurRadius(20)
            shadow.setOffset(0, 6)
            shadow.setColor(QColor(0, 0, 0, 160))
            popup.setGraphicsEffect(shadow)

# ============================================================
# WIDGET: HUDToggle
# ============================================================
class HUDToggle(QWidget):
    toggled = Signal(int)

    # === INICJALIZACJA WIDGETU ===
    def __init__(self, parent=None):
        super().__init__(parent)
        self._value = 0
        self.setFixedSize(70, 18)
        self.setCursor(Qt.PointingHandCursor)

    # === API PUBLICZNE: AKTUALNA WARTOŚĆ ===
    def value(self):
        return self._value

    # === API PUBLICZNE: USTAWIENIE WARTOŚCI ===
    def setValue(self, value: int):
        self._value = 1 if value else 0
        self.update()

    # === ZDARZENIE QT: KLIKNIĘCIE MYSZY ===
    def mousePressEvent(self, event):
        self._value = 1 - self._value
        self.toggled.emit(self._value)
        self.update()

    # === RYSOWANIE WIDGETU ===
    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.Antialiasing)

        # === TŁO ===
        painter.setBrush(QColor(BG_APP))
        painter.setPen(QPen(QColor(ACCENT_DARK), 1))
        painter.drawRoundedRect(self.rect(), 6, 6)

        # === HANDLE ===
        half = self.width() // 2
        handle_rect = QRect(
            1 + self._value * half,
            1,
            half - 2,
            self.height() - 2
        )

        painter.setBrush(QColor(ACCENT))
        painter.setPen(Qt.NoPen)
        painter.drawRoundedRect(handle_rect, 4, 4)

        painter.end()

# ============================================================
# SIDEBAR ITEM DELEGATE
# ============================================================
class SidebarItemDelegate(QStyledItemDelegate):
    # === RYSOWANIE ELEMENTU LISTY SIDEBARA ===
    def paint(self, painter, option, index):
        painter.save()

        # === TŁO (hover / selected) ===
        option.widget.style().drawPrimitive(
            QStyle.PE_PanelItemViewItem,
            option,
            painter,
            option.widget
        )

        rect = option.rect

        # === IKONA ===
        icon_rect = QRect()
        icon = index.data(Qt.DecorationRole)

        if icon:
            icon_size = option.decorationSize
            icon_x = rect.left() + 12
            icon_y = rect.center().y() - icon_size.height() // 2

            icon_rect = QRect(
                icon_x,
                icon_y,
                icon_size.width(),
                icon_size.height()
            )

            mode = QIcon.Selected if option.state & QStyle.State_Selected else QIcon.Normal
            icon.paint(painter, icon_rect, Qt.AlignCenter, mode)

        # === TEKST ===
        text = index.data(Qt.DisplayRole)
        fm = QFontMetrics(option.font)

        text_x = (icon_rect.right() + 10) if not icon_rect.isNull() else rect.left() + 12
        text_y = rect.center().y() + (fm.ascent() - fm.descent()) // 2 + 2

        painter.setFont(option.font)
        painter.setPen(option.palette.text().color())
        painter.drawText(text_x, text_y, text)

        painter.restore()

# ============================================================
# WIDGET: InstantTooltip
# ============================================================
class InstantTooltip(QWidget):
    _FADE_IN_MS  = 140
    _FADE_OUT_MS = 100
    _SLIDE_PX    = 6

    # === INICJALIZACJA WIDGETU ===
    def __init__(self, text: str, parent=None):
        super().__init__(parent, Qt.ToolTip | Qt.FramelessWindowHint)

        self.setAttribute(Qt.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WA_TransparentForMouseEvents)
        self.setAttribute(Qt.WA_TranslucentBackground)

        # === PRZYGOTOWANIE TEKSTU TOOLTIPA ===
        wrapped_text = self._soft_wrap_text(text, TOOLTIP_SOFT_WRAP_CHARS)

        self.label = QLabel(wrapped_text, self)
        self.label.setTextFormat(Qt.PlainText)
        self.label.setWordWrap(False)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(10, 8, 10, 8)
        layout.addWidget(self.label)

        self.setStyleSheet(f"""
            QWidget {{
                background-color: {BG_PANEL};
                border: 1px solid {BORDER};
                border-radius: 8px;
            }}
            QLabel {{
                color: {TEXT_PRIMARY};
                font-size: {FONT_SMALL}px;
            }}
        """)

        self.adjustSize()

        self._eff = QGraphicsOpacityEffect(self)
        self._eff.setOpacity(0.0)
        self.setGraphicsEffect(self._eff)

        self._fade_in  = QPropertyAnimation(self._eff, b"opacity", self)
        self._fade_out = QPropertyAnimation(self._eff, b"opacity", self)
        self._slide    = QPropertyAnimation(self, b"pos", self)

        self._fade_in.setDuration(self._FADE_IN_MS)
        self._fade_in.setStartValue(0.0)
        self._fade_in.setEndValue(1.0)
        self._fade_in.setEasingCurve(QEasingCurve.OutCubic)

        self._fade_out.setDuration(self._FADE_OUT_MS)
        self._fade_out.setStartValue(1.0)
        self._fade_out.setEndValue(0.0)
        self._fade_out.setEasingCurve(QEasingCurve.InCubic)
        self._fade_out.finished.connect(self._on_fade_out_done)

        self._slide.setDuration(self._FADE_IN_MS)
        self._slide.setEasingCurve(QEasingCurve.OutCubic)

        self._target_pos = None

    # === WYŚWIETLENIE TOOLTIPA W POZYCJI GLOBALNEJ ===
    def show_at(self, global_pos):
        self._fade_out.stop()
        self._target_pos = global_pos
        start = QPoint(global_pos.x(), global_pos.y() + self._SLIDE_PX)
        self.move(start)
        self._slide.setStartValue(start)
        self._slide.setEndValue(global_pos)
        self._eff.setOpacity(0.0)
        self.show()
        self._fade_in.start()
        self._slide.start()

    def close(self):
        self._fade_in.stop()
        self._slide.stop()
        self._fade_out.start()

    def _on_fade_out_done(self):
        super().close()

    # === LOGIKA WEWNĘTRZNA: MIĘKKIE ŁAMANIE LINII ===
    @staticmethod
    def _soft_wrap_text(text: str, limit: int) -> str:
        if limit <= 0:
            return text

        out_lines = []
        for line in text.split("\n"):
            s = line.strip()
            while len(s) > limit:
                cut = s.rfind(" ", 0, limit + 1)
                if cut == -1:
                    cut = limit
                out_lines.append(s[:cut].rstrip())
                s = s[cut:].lstrip()
            out_lines.append(s)

        return "\n".join(out_lines)

# ============================================================
# WIDGET: ErrorTooltipLineEdit
# ============================================================
class ErrorTooltipLineEdit(QLineEdit):
    # === INICJALIZACJA WIDGETU ===
    def __init__(self, tooltip_key: str, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._tooltip_key = f"ERROR_{tooltip_key}"
        self._tooltip = None

    def enterEvent(self, event):
        if self.property("state") == "error" and self._tooltip_key in TOOLTIPS:
            if not self._tooltip:
                self._tooltip = InstantTooltip(TOOLTIPS[self._tooltip_key])

            pos = self.mapToGlobal(self.rect().bottomRight())
            self._tooltip.show_at(pos)

        super().enterEvent(event)

    def leaveEvent(self, event):
        if self._tooltip:
            self._tooltip.close()
            self._tooltip = None

        super().leaveEvent(event)

# ============================================================
# WIDGET: HelpIcon
# ============================================================
class HelpIcon(QLabel):
    # === INICJALIZACJA WIDGETU ===
    def __init__(self, tooltip_text: str, parent=None):
        super().__init__(parent)

        self._tooltip_text = tooltip_text
        self._tooltip = None

        self.setText("?")
        self.setAlignment(Qt.AlignCenter)
        self.setFixedSize(14, 14)
        self.setCursor(Qt.PointingHandCursor)
        self.setContentsMargins(0, 0, 0, 0)

        self.setStyleSheet("""
            QLabel {
                color: %s;
                font-weight: 700;
                font-size: 11px;
                background-color: %s;
                border-radius: 7px;
            }
            QLabel:hover {
                background-color: %s;
                color: %s;
            }
        """ % (TEXT_MUTED, BG_HOVER, ACCENT, WHITE))

    def enterEvent(self, event):
        if not self._tooltip:
            self._tooltip = InstantTooltip(self._tooltip_text)

        pos = self.mapToGlobal(self.rect().bottomRight())
        self._tooltip.show_at(pos)
        event.accept()

    def leaveEvent(self, event):
        if self._tooltip:
            self._tooltip.close()
            self._tooltip = None
        event.accept()

# ============================================================
# DIALOG: RecentPresetsDialog
# ============================================================
class RecentPresetsDialog(QDialog):
    loadRequested = Signal(str)
    removeRequested = Signal(str)
    clearRequested = Signal()

    # === INICJALIZACJA DIALOGU ===
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAttribute(Qt.WA_StyledBackground, True)
        self.setWindowFlags(Qt.Dialog | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint)
        self.setAttribute(Qt.WA_TranslucentBackground)
        self._drag_pos = None

        self.setWindowTitle("Ostatnio używane presety")
        self.resize(360, 370)
        self.setMinimumSize(360, 370)
        self.setMaximumSize(360, 370)
        self.setWindowModality(Qt.ApplicationModal)

        outer = QVBoxLayout(self)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)
        self._frame = AppFrame()
        outer.addWidget(self._frame)

        self._build_ui()
        self._load_presets_once()

    # === BUDOWA INTERFEJSU UŻYTKOWNIKA ===
    def _build_ui(self):
        layout = QVBoxLayout(self._frame)
        layout.setSpacing(SPACE_SM)
        layout.setContentsMargins(12, 4, 12, 12)

        # === PASEK GÓRNY (X po prawej) ===
        top_bar = QWidget()
        top_bar_layout = QHBoxLayout(top_bar)
        top_bar_layout.setContentsMargins(0, 0, 0, 0)
        top_bar_layout.setSpacing(0)
        top_bar_layout.addStretch()

        close_btn = QPushButton()
        close_btn.setObjectName("titlebar-close-btn")
        close_btn.setIcon(qta.icon("mdi.window-close", color=TEXT_MUTED))
        close_btn.setFixedSize(28, 28)
        close_btn.setCursor(Qt.PointingHandCursor)
        close_btn.clicked.connect(self.reject)
        top_bar_layout.addWidget(close_btn)

        # === LOGO ===
        logo_label = QLabel()
        logo_label.setAlignment(Qt.AlignCenter)

        logo_path = asset_path("images", "logo.png")
        pixmap = QPixmap(logo_path)

        if not pixmap.isNull():
            logo_label.setPixmap(
                pixmap.scaledToWidth(160, Qt.SmoothTransformation)
            )

        # === TYTUŁ + HELP ===
        title_label = QLabel("Ostatnio używane presety")
        title_label.setStyleSheet(
            f"font-weight: {FONT_WEIGHT_SEMIBOLD};"
        )

        title_help = HelpIcon(TOOLTIPS["recent_presets_dialog"])

        title_container = QWidget()
        title_layout = QHBoxLayout(title_container)
        title_layout.setContentsMargins(0, 0, 0, 0)
        title_layout.setSpacing(OPTION_LABEL_ICON_SPACING)

        title_layout.addStretch()
        title_layout.addWidget(title_label)
        title_layout.addWidget(title_help)
        title_layout.addStretch()

        # === LISTA PRESETÓW ===
        self.list = QListWidget()
        self.list.setFocusPolicy(Qt.NoFocus)
        self.list.setProperty("class", "recent-presets")

        # === PRZYCISKI ===
        self.create_btn = QPushButton("Utwórz")
        self.load_btn = QPushButton("Wczytaj")
        self.remove_btn = QPushButton("Usuń")
        self.clear_btn = QPushButton("Wyczyść")

        for btn in (self.create_btn, self.load_btn, self.remove_btn, self.clear_btn):
            btn.setCursor(Qt.PointingHandCursor)
            btn.setProperty("class", "preset")
            btn.setProperty("size", "sm")

        self.load_btn.setEnabled(False)
        self.remove_btn.setEnabled(False)

        buttons_layout = QHBoxLayout()
        buttons_layout.setSpacing(BUTTON_GROUP_SPACING)

        buttons_layout.addStretch()
        buttons_layout.addWidget(self.create_btn)
        buttons_layout.addWidget(self.load_btn)
        buttons_layout.addWidget(self.remove_btn)
        buttons_layout.addWidget(self.clear_btn)
        buttons_layout.addStretch()

        # === SKŁADANIE LAYOUTU ===
        layout.addWidget(top_bar)
        layout.addWidget(logo_label)
        layout.addSpacing(SPACE_MD)
        layout.addWidget(title_container)
        layout.addWidget(self.list)

        # === SZCZEGÓŁY ZAZNACZONEGO PRESETU ===
        self.details_path = QLabel("Ścieżka: —")
        self.details_last_used = QLabel("Ostatnio używany: —")

        for lbl in (self.details_path, self.details_last_used):
            lbl.setWordWrap(True)
            lbl.setProperty("class", "about-text-muted")

        layout.addWidget(self.details_path)
        layout.addWidget(self.details_last_used)

        layout.addLayout(buttons_layout)

        # === SYGNAŁY ===
        self.create_btn.clicked.connect(self.accept)
        self.load_btn.clicked.connect(self._emit_load)
        self.remove_btn.clicked.connect(self._emit_remove)
        self.clear_btn.clicked.connect(self._emit_clear)

        self.list.currentItemChanged.connect(self._update_details)
        self.list.currentItemChanged.connect(
            lambda current, _: self._update_load_button_state(current)
        )

        self.load_btn.setEnabled(False)

    # === PRZECIĄGANIE OKNA ===
    def mousePressEvent(self, event):
        if event.button() == Qt.LeftButton:
            self._drag_pos = event.globalPosition().toPoint() - self.frameGeometry().topLeft()
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        if event.buttons() == Qt.LeftButton and self._drag_pos is not None:
            self.move(event.globalPosition().toPoint() - self._drag_pos)
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        self._drag_pos = None
        super().mouseReleaseEvent(event)

    # === AKTUALIZACJA SZCZEGÓŁÓW ZAZNACZONEGO PRESETU ===
    def _update_details(self, current, previous=None):
        if not current:
            self.details_path.setText("Ścieżka: —")
            self.details_last_used.setText("Ostatnio używany: —")
            return

        entry = current.data(Qt.UserRole)
        if not entry:
            return

        preset_path = entry.get("path")
        last_used = entry.get("last_used", "—")

        if not preset_path or not os.path.exists(preset_path):
            self.details_path.setText("Ścieżka: brak pliku")
        else:
            self.details_path.setText(f"Ścieżka: {preset_path}")

        self.details_last_used.setText(f"Ostatnio używany: {last_used}")

    # === AKTUALIZACJA STANU PRZYCISKU WCZYTAJ ===
    def _update_load_button_state(self, current):
        if not current:
            self.load_btn.setEnabled(False)
            return

        # === BRAK PLIKU PRESETU ===
        if current.font().strikeOut():
            self.load_btn.setEnabled(False)
            self.remove_btn.setEnabled(True)
        else:
            self.load_btn.setEnabled(True)
            self.remove_btn.setEnabled(True)

    # === JEDNORAZOWE WCZYTANIE LISTY PRESETÓW ===
    def _load_presets_once(self):
        self._refresh_list()
    
    # === ODSWIEŻENIE LISTY PRESETÓW ===
    def _refresh_list(self):
        self.list.clear()

        for entry in presets.get_recent_presets():
            name = entry.get("name")
            path = entry.get("path")

            if not name:
                continue

            item = QListWidgetItem(name)
            item.setData(Qt.UserRole, entry)

            if not path or not os.path.exists(path):
                font = item.font()
                font.setStrikeOut(True)
                item.setFont(font)
                item.setForeground(QColor(TEXT_MUTED))

            self.list.addItem(item)

        self.details_path.setText("Ścieżka: —")
        self.details_last_used.setText("Ostatnio używany: —")
        self.load_btn.setEnabled(False)
        self.remove_btn.setEnabled(False)

    # === EMISJA: NAZWA PRESETU ===
    def _current_name(self):
        item = self.list.currentItem()
        return item.text() if item else None

    # === EMISJA: WCZYTANIE PRESETU ===
    def _emit_load(self):
        item = self.list.currentItem()
        if not item:
            return

        if item.font().strikeOut():
            return

        entry = item.data(Qt.UserRole)
        if not entry:
            return

        preset_path = entry.get("path")
        if not preset_path:
            return

        self.loadRequested.emit(preset_path)
        self.accept()

    # === EMISJA: USUNIĘCIE PRESETU ===
    def _emit_remove(self):
        item = self.list.currentItem()
        if not item:
            return

        entry = item.data(Qt.UserRole)
        if not entry:
            return

        preset_path = entry.get("path")
        if not preset_path:
            return

        presets.remove_recent_preset(preset_path)
        self._refresh_list()

    # === EMISJA: WYCZYSZCZENIE LISTY ===
    def _emit_clear(self):
        presets.clear_recent_presets()
        self._refresh_list()

# ============================================================
# DIALOG: NotificationDialog
# ============================================================
class NotificationDialog(QDialog):
    APP_TITLE = "GameReader"

    INFO_TYPE = "info"
    WARNING_TYPE = "warning"
    ERROR_TYPE = "error"

    _SOUNDS = {
        INFO_TYPE: "announcement",
        WARNING_TYPE: "announcement",
        ERROR_TYPE: "announcement",
    }

    _LAST_SOUND_TS = 0.0
    _SOUND_DEBOUNCE_MS = 600

    _CONFIG = {
        INFO_TYPE: {
            "icon": "fa5s.info-circle",
            "color": INFO,
            "title": "Informacja"
        },
        WARNING_TYPE: {
            "icon": "fa5s.exclamation-triangle",
            "color": WARNING,
            "title": "Ostrzeżenie"
        },
        ERROR_TYPE: {
            "icon": "fa5s.times-circle",
            "color": DANGER,
            "title": "Błąd"
        }
    }

    # === INICJALIZACJA DIALOGU ===
    def __init__(self, message: str, kind=INFO_TYPE, parent=None, title=None):
        super().__init__(parent)

        cfg = self._CONFIG[kind]
        final_title = title or cfg["title"]

        self.setWindowTitle(f"{self.APP_TITLE} - {final_title}")

        self.setProperty("class", "notification")

        self.setWindowModality(Qt.ApplicationModal)
        self.setWindowFlag(Qt.WindowStaysOnTopHint, True)
        self.setMinimumWidth(380)

        self._build_ui(
            final_title,
            message,
            cfg["icon"],
            cfg["color"]
        )

        self._play_notification_sound(kind)

    # === ODTWORZENIE DŹWIĘKU POWIADOMIENIA ===
    def _play_notification_sound(self, kind: str):
        now = time.monotonic()
        min_interval = self._SOUND_DEBOUNCE_MS / 1000.0

        # === DEBOUNCE ===
        if now - self._LAST_SOUND_TS < min_interval:
            return

        self.__class__._LAST_SOUND_TS = now

        sound_name = self._SOUNDS.get(kind)
        if not sound_name:
            return

        try:
            sound_path = audio_player.find_system_sound(sound_name)
            audio_player.play_system_sound(sound_path)
        except Exception as e:
            debug.log(debug.WARNING, "UI", f"Błąd odtwarzania dźwięku powiadomienia: {e}")

    # === BUDOWA INTERFEJSU UŻYTKOWNIKA ===
    def _build_ui(self, title, message, icon_name, accent_color):
        layout = QVBoxLayout(self)
        layout.setSpacing(SPACE_MD)

        # === HEADER ===
        header = QHBoxLayout()
        header.setSpacing(SPACE_SM)

        icon = QLabel()
        icon.setPixmap(
            qta.icon(icon_name, color=accent_color).pixmap(24, 24)
        )

        title_lbl = QLabel(title)
        title_lbl.setProperty("class", "notification-title")

        header.addWidget(icon)
        header.addWidget(title_lbl)
        header.addStretch()

        # === WIADOMOŚĆ ===
        msg = QLabel(message)
        msg.setWordWrap(True)
        msg.setProperty("class", "notification-message")

        # === PRZYCISKI ===
        btn = QPushButton("OK")
        btn.setCursor(Qt.PointingHandCursor)
        btn.setProperty("class", "preset")
        btn.setProperty("size", "sm")
        btn.clicked.connect(self.accept)

        # === ASSEMBLY ===
        layout.addLayout(header)
        layout.addWidget(msg)
        layout.addStretch()
        layout.addWidget(btn, alignment=Qt.AlignRight)

# ============================================================
# DIALOG: LegacyPresetDialog
# ============================================================
class LegacyPresetDialog(QDialog):
    APP_TITLE = "GameReader"
    _LAST_SOUND_TS = 0.0
    _SOUND_DEBOUNCE_MS = 600

    # === INICJALIZACJA DIALOGU ===
    def __init__(self, parent=None):
        super().__init__(parent)

        self.convert = False

        self.setWindowTitle(f"{self.APP_TITLE} - Informacja")
        self.setProperty("class", "notification")

        self.setWindowModality(Qt.ApplicationModal)
        self.setWindowFlag(Qt.WindowStaysOnTopHint, True)
        self.setMinimumWidth(420)

        self._build_ui()
        self._play_notification_sound()

    # === ODTWORZENIE DŹWIĘKU POWIADOMIENIA ===
    def _play_notification_sound(self):
        now = time.monotonic()
        min_interval = self._SOUND_DEBOUNCE_MS / 1000.0

        # DEBOUNCE
        if now - self._LAST_SOUND_TS < min_interval:
            return

        self.__class__._LAST_SOUND_TS = now

        try:
            sound_path = audio_player.find_system_sound("announcement")
            audio_player.play_system_sound(sound_path)
        except Exception as e:
            debug.log(debug.WARNING, "UI", f"Błąd odtwarzania dźwięku powiadomienia: {e}")

    # === BUDOWA INTERFEJSU UŻYTKOWNIKA ===
    def _build_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(SPACE_MD)

        # === HEADER ===
        header = QHBoxLayout()
        header.setSpacing(SPACE_SM)

        icon = QLabel()
        icon.setPixmap(
            qta.icon("fa5s.info-circle", color=INFO).pixmap(24, 24)
        )

        title_lbl = QLabel("Informacja")
        title_lbl.setProperty("class", "notification-title")

        header.addWidget(icon)
        header.addWidget(title_lbl)
        header.addStretch()

        # === WIADOMOŚĆ ===
        msg = QLabel(
            "Wykryto preset utworzony w starszej wersji programu (< 0.9.3).\n"
            "Ustawienia brakujących funkcji zostaną uzupełnione wartościami domyślnymi.\n\n"
            "Czy chcesz zaktualizować strukturę presetu do najnowszej wersji?"
        )
        msg.setWordWrap(True)
        msg.setProperty("class", "notification-message")

        # === PRZYCISKI ===
        buttons = QHBoxLayout()
        buttons.addStretch()

        yes_btn = QPushButton("TAK")
        no_btn = QPushButton("NIE")

        for btn in (yes_btn, no_btn):
            btn.setCursor(Qt.PointingHandCursor)
            btn.setProperty("class", "preset")
            btn.setProperty("size", "sm")

        yes_btn.clicked.connect(self._yes)
        no_btn.clicked.connect(self._no)

        buttons.addWidget(yes_btn)
        buttons.addWidget(no_btn)

        # === ASSEMBLY ===
        layout.addLayout(header)
        layout.addWidget(msg)
        layout.addStretch()
        layout.addLayout(buttons)

    # === AKCEPTACJA KONWERSJI PRESETU ===
    def _yes(self):
        self.convert = True
        self.accept()

    # === ODRZUCENIE KONWERSJI PRESETU ===
    def _no(self):
        self.convert = False
        self.accept()

# ============================================================
# OVERLAY: WCZYTYWANIE PRESETU
# ============================================================
class LoadingOverlay(QFrame):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAttribute(Qt.WA_StyledBackground, True)
        self.setWindowFlags(Qt.Widget)
        self.setVisible(False)
        self.setProperty("class", "loading-overlay")

        layout = QVBoxLayout(self)
        layout.setAlignment(Qt.AlignCenter)

        # === PANEL CENTRALNY ===
        self.panel = QFrame()
        self.panel.setObjectName("loading-panel")
        self.panel.setFixedWidth(380)   # 👈 kluczowe
        self.panel.setMaximumWidth(420)

        panel_layout = QVBoxLayout(self.panel)
        panel_layout.setAlignment(Qt.AlignCenter)
        panel_layout.setSpacing(18)
        panel_layout.setContentsMargins(28, 26, 28, 26)

        self.label = QLabel("Wczytywanie presetu…")
        self.label.setAlignment(Qt.AlignCenter)
        self.label.setProperty("class", "loading-label")

        self.bar = QProgressBar()
        self.bar.setRange(0, 0)
        self.bar.setFixedHeight(8)      # 👈 cieńszy
        self.bar.setTextVisible(False)
        self.bar.setProperty("class", "loading-spinner")

        panel_layout.addWidget(self.label)
        panel_layout.addWidget(self.bar)

        layout.addWidget(self.panel)

    def set_message(self, text: str):
        self.label.setText(text)

    def attach_to_parent(self):
        if self.parent():
            self.setGeometry(self.parent().rect())

    def showEvent(self, e):
        self.attach_to_parent()
        super().showEvent(e)

# ============================================================
# FUNKCJE POMOCNICZE
# ============================================================
def show_validation_result(
    result,
    parent=None,
    *,
    error_context="",
    warning_context=""
):
    if not result:
        return False

    if result.errors:
        messages = "\n".join(f"• {e.message}" for e in result.errors)
        NotificationDialog(
            message=(
                (error_context + "\n\n" if error_context else "")
                + "Wykryto błędy:\n\n"
                + messages
            ),
            kind=NotificationDialog.ERROR_TYPE,
            parent=parent
        ).exec()
        return False

    if result.warnings:
        messages = "\n".join(f"• {w.message}" for w in result.warnings)
        NotificationDialog(
            message=(
                (warning_context + "\n\n" if warning_context else "")
                + "Wykryto ostrzeżenia:\n\n"
                + messages
            ),
            kind=NotificationDialog.WARNING_TYPE,
            parent=parent
        ).exec()

    return True