
# ============================================================
# IMPORTY
# ============================================================
import sys
from PySide6.QtCore import Qt, QRect, QTimer
from PySide6.QtGui import QPainter, QColor, QPen, QGuiApplication, QFont, QFontMetrics
from PySide6.QtWidgets import QWidget

from core import state
from ui.theme.theme import (
    DEBUG_AREA_1_COLOR,
    DEBUG_AREA_2_COLOR,
    DEBUG_AREA_ALPHA,
    DEBUG_AREA_BORDER_WIDTH,
    DEBUG_GUIDE_LINE_COLOR,
    DEBUG_GUIDE_LINE_ALPHA,
    DEBUG_GUIDE_LINE_WIDTH,
    DEBUG_GUIDE_LINE_STYLE,
    DEBUG_GUIDE_LABEL_BG_ALPHA,
    DEBUG_GUIDE_LABEL_TEXT_ALPHA,
)

# ============================================================
# WIDGET: DebugOverlay
# ============================================================
class DebugOverlay(QWidget):
    def __init__(self):
        super().__init__()

        self.setWindowFlags(
            Qt.FramelessWindowHint |
            Qt.WindowStaysOnTopHint |
            Qt.Tool
        )

        self.setAttribute(Qt.WA_TransparentForMouseEvents)
        self.setAttribute(Qt.WA_NoSystemBackground)
        self.setAttribute(Qt.WA_TranslucentBackground)

        self._visible = False

        # === KLUCZ: overlay na WYBRANY monitor ===
        screens = QGuiApplication.screens()
        idx = state.selected_screen_monitor - 1

        if 0 <= idx < len(screens):
            screen = screens[idx]
        else:
            screen = QGuiApplication.primaryScreen()

        self.setGeometry(screen.geometry())

        self.hide()

        # === AUTO REASSERT (utrzymanie always-on-top) ===
        self._reassert_timer = QTimer(self)
        self._reassert_timer.setInterval(300)
        self._reassert_timer.timeout.connect(self._ensure_on_top)

    # =====================================================
    # SKALOWANIE QT -> FIZYCZNE PIKSELE
    # =====================================================
    def _scale_to_qt(self, value: int) -> int:
        screen = self.screen() or QGuiApplication.primaryScreen()
        scale = screen.devicePixelRatio()
        return int(value / scale)

    # =====================================================
    # API PUBLICZNE
    # =====================================================
    def toggle(self):
        if self._visible:
            self.hide_overlay()
        else:
            self.show_overlay()

    def show_overlay(self):
        self._rebind_to_selected_screen()

        self._visible = True
        state.debug_enabled = True

        self.show()
        self.raise_()
        self._force_windows_topmost()

        self._reassert_timer.start()
        self.update()

    def hide_overlay(self):
        self._visible = False
        state.debug_enabled = False

        self._reassert_timer.stop()
        self.hide()

    def is_visible(self) -> bool:
        return self._visible
    
    def _force_windows_topmost(self):
        if not sys.platform.startswith("win"):
            return

        try:
            import ctypes

            hwnd = int(self.winId())

            HWND_TOPMOST = -1
            SWP_NOMOVE = 0x0002
            SWP_NOSIZE = 0x0001
            SWP_NOACTIVATE = 0x0010
            SWP_SHOWWINDOW = 0x0040

            ctypes.windll.user32.SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW
            )
        except Exception:
            pass

    def _ensure_on_top(self):
        if not self._visible or not self.isVisible():
            return

        self.raise_()
        self._force_windows_topmost()

    def _rebind_to_selected_screen(self):
        screens = QGuiApplication.screens()
        idx = state.selected_screen_monitor - 1

        if 0 <= idx < len(screens):
            screen = screens[idx]
        else:
            screen = QGuiApplication.primaryScreen()

        self.setGeometry(screen.geometry())

    # =====================================================
    # RYSOWANIE: OBSZARY OCR
    # =====================================================
    def _draw_monitor_1(self, painter: QPainter):
        m = state.monitor
        if not m:
            return

        rect = QRect(
            self._scale_to_qt(int(m.get("left", 0))),
            self._scale_to_qt(int(m.get("top", 0))),
            self._scale_to_qt(int(m.get("width", 0))),
            self._scale_to_qt(int(m.get("height", 0))),
        )

        color = QColor(DEBUG_AREA_1_COLOR)
        color.setAlpha(DEBUG_AREA_ALPHA)

        pen = QPen(color)
        pen.setWidth(DEBUG_AREA_BORDER_WIDTH)
        pen.setStyle(Qt.SolidLine)

        painter.setPen(pen)
        painter.setBrush(Qt.NoBrush)
        painter.drawRect(rect)

        self._draw_label(
            painter,
            rect,
            "Obszar dialogów 1",
            color
        )

        self._draw_center_lines_for_monitor(painter, rect)

    def _draw_monitor_2(self, painter: QPainter):
        if not state.monitor2_enabled:
            return

        rect = QRect(
            self._scale_to_qt(int(state.monitor2_left)),
            self._scale_to_qt(int(state.monitor2_top)),
            self._scale_to_qt(int(state.monitor2_width)),
            self._scale_to_qt(int(state.monitor2_height)),
        )

        color = QColor(DEBUG_AREA_2_COLOR)
        color.setAlpha(DEBUG_AREA_ALPHA)

        pen = QPen(color)
        pen.setWidth(DEBUG_AREA_BORDER_WIDTH)
        pen.setStyle(Qt.SolidLine)

        painter.setPen(pen)
        painter.setBrush(Qt.NoBrush)
        painter.drawRect(rect)

        self._draw_label(
            painter,
            rect,
            "Obszar dialogów 2",
            color
        )

        self._draw_center_lines_for_monitor(painter, rect)

    # =====================================================
    # RYSOWANIE: ETYKIETY OBSZARÓW
    # =====================================================
    def _draw_label(self, painter: QPainter, rect: QRect, text: str, color: QColor):
        font = painter.font()
        font.setBold(True)
        font.setPointSize(10)
        painter.setFont(font)

        metrics = QFontMetrics(font)
        padding = 6

        text_width = metrics.horizontalAdvance(text)
        text_height = metrics.height()

        label_rect = QRect(
            rect.left(),
            rect.top() - text_height - padding * 2,
            text_width + padding * 2,
            text_height + padding * 2,
        )

        bg = QColor(0, 0, 0, 160)
        painter.setPen(Qt.NoPen)
        painter.setBrush(bg)
        painter.drawRoundedRect(label_rect, 4, 4)

        text_color = QColor(255, 255, 255, 230)
        painter.setPen(text_color)

        painter.drawText(
            label_rect.adjusted(padding, padding, -padding, -padding),
            Qt.AlignLeft | Qt.AlignVCenter,
            text
        )

    # =====================================================
    # RYSOWANIE: LINIE POMOCNICZE (HUD)
    # =====================================================
    def _draw_vertical_edges(
        self,
        painter: QPainter,
        x: int,
        top: int,
        height: int,
        width: int,
    ) -> tuple[int, int]:

        left_x = x
        right_x = x + width

        color = QColor(DEBUG_GUIDE_LINE_COLOR)
        color.setAlpha(DEBUG_GUIDE_LINE_ALPHA)

        pen = QPen(color)
        pen.setWidth(DEBUG_GUIDE_LINE_WIDTH)

        if DEBUG_GUIDE_LINE_STYLE == "dash":
            pen.setStyle(Qt.DashLine)
        else:
            pen.setStyle(Qt.SolidLine)

        painter.setPen(pen)

        painter.drawLine(left_x, top, left_x, top + height)
        painter.drawLine(right_x, top, right_x, top + height)

        return left_x, right_x

    def _draw_line_label(
        self,
        painter: QPainter,
        text: str,
        band_rect: QRect,
    ):
        font = painter.font()
        font.setBold(True)
        painter.setFont(font)

        metrics = QFontMetrics(font)
        padding = 4

        text_width = metrics.horizontalAdvance(text)
        text_height = metrics.height()

        center_x = band_rect.center().x()
        center_y = band_rect.center().y()

        label_rect = QRect(
            center_x - (text_width + padding * 2) // 2,
            center_y - (text_height + padding * 2) // 2,
            text_width + padding * 2,
            text_height + padding * 2,
        )

        # === TŁO LABELKI ===
        bg = QColor(0, 0, 0)
        bg.setAlpha(DEBUG_GUIDE_LABEL_BG_ALPHA)

        painter.setPen(Qt.NoPen)
        painter.setBrush(bg)
        painter.drawRoundedRect(label_rect, 4, 4)

        # === TEKST LABELKI ===
        text_color = QColor(255, 255, 255)
        text_color.setAlpha(DEBUG_GUIDE_LABEL_TEXT_ALPHA)

        painter.setPen(text_color)
        painter.drawText(
            label_rect.adjusted(padding, padding, -padding, -padding),
            Qt.AlignCenter,
            text
        )

    # =====================================================
    # RYSOWANIE: CENTER LINES (L1/L2/L3)
    # ======================================================
    def _draw_center_lines_for_monitor(self, painter: QPainter, rect: QRect):
        margin = int(state.CENTER_LINE_MARGIN)

        # === LINIA 1 (CENTER) ===
        if state.USE_CENTER_LINE_1:
            center_x = rect.left() + rect.width() // 2
            x = center_x - margin // 2

            left_x, right_x = self._draw_vertical_edges(
                painter,
                x,
                rect.top(),
                rect.height(),
                margin
            )

            label_rect = QRect(
                left_x,
                rect.center().y() - 10,
                margin,
                20
            )
            self._draw_line_label(painter, "L1", label_rect)

        # === LINIA 2 (OFFSET) ===
        if state.USE_CENTER_LINE_2:
            x = rect.left() + int(state.CENTER_LINE_2_START)

            left_x, right_x = self._draw_vertical_edges(
                painter,
                x,
                rect.top(),
                rect.height(),
                margin
            )

            label_rect = QRect(
                left_x,
                rect.center().y() - 10,
                margin,
                20
            )
            self._draw_line_label(painter, "L2", label_rect)

        # === LINIA 3 (RATIO) ===
        if state.USE_CENTER_LINE_3:
            x = rect.left() + int(rect.width() * state.CENTER_LINE_3_START_RATIO)

            left_x, right_x = self._draw_vertical_edges(
                painter,
                x,
                rect.top(),
                rect.height(),
                margin
            )

            label_rect = QRect(
                left_x,
                rect.center().y() - 10,
                margin,
                20
            )
            self._draw_line_label(painter, "L3", label_rect)

    # =====================================================
    # ZDARZENIE QT: RYSOWANIE OKNA (QPainter)
    # =====================================================
    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.Antialiasing)

        self._draw_monitor_1(painter)
        self._draw_monitor_2(painter)

        painter.end()

