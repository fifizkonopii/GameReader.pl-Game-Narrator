
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtCore import Qt, QRect, QPoint, Signal
from PySide6.QtGui import QPainter, QColor, QPen
from PySide6.QtWidgets import QWidget

import qtawesome as qta

from ui.theme.theme import (
    DEBUG_AREA_1_COLOR,
    DEBUG_AREA_2_COLOR,
    TEXT_PRIMARY,
    FONT_LARGE,
    FONT_BASE,
    OVERLAY_BG,
    OVERLAY_BG_ALPHA,
    OVERLAY_PANEL_BG,
    OVERLAY_PANEL_ALPHA,
    OVERLAY_GRID_ALPHA,
    OVERLAY_ICON_BASELINE_OFFSET,
)
from ui.theme.global_qss import build_global_qss

# ============================================================
# STAŁE: UCHWYTY
# ============================================================
_HANDLE_SIZE  = 8
_HANDLE_HIT   = 11   # promień detekcji kliknięcia
_MOVE_BAR_H   = 18   # wysokość paska do przesuwania (góra prostokąta)

_HANDLE_CURSORS = {
    "TL": Qt.SizeFDiagCursor,
    "BR": Qt.SizeFDiagCursor,
    "TR": Qt.SizeBDiagCursor,
    "BL": Qt.SizeBDiagCursor,
    "T":  Qt.SizeVerCursor,
    "B":  Qt.SizeVerCursor,
    "L":  Qt.SizeHorCursor,
    "R":  Qt.SizeHorCursor,
}

# ============================================================
# WIDGET: SCREEN AREA SELECTOR
# ============================================================
class ScreenAreaSelector(QWidget):
    areaSelected = Signal(dict)
    cancelled = Signal()

    def __init__(self, screen, initial_rect: dict | None = None, area_index: int = 1):
        super().__init__()
        self.area_index = area_index
        self.screen = screen
        self.screen_geometry = screen.geometry()
        self._scale = screen.devicePixelRatio()

        # Aktywne zaznaczenie w pikselach logicznych
        if initial_rect:
            s = self._scale
            self._sel = QRect(
                int(initial_rect["left"]   / s),
                int(initial_rect["top"]    / s),
                int(initial_rect["width"]  / s),
                int(initial_rect["height"] / s),
            )
        else:
            self._sel = None

        # Tryb interakcji: idle | drawing | moving | resizing
        self._mode          = "idle"
        self._drag_start    = None
        self._current_pos   = None
        self._sel_at_drag   = None
        self._resize_handle = None

        self.setWindowFlags(Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint)
        self.setAttribute(Qt.WA_TranslucentBackground)
        self.setGeometry(self.screen_geometry)
        self.setMouseTracking(True)
        self.setStyleSheet(build_global_qss())

        self.show()
        self.raise_()
        self.activateWindow()

    # =====================================================
    # HELPERS: UCHWYTY
    # =====================================================
    def _handles(self, r: QRect) -> dict:
        cx, cy = r.center().x(), r.center().y()
        return {
            "TL": QPoint(r.left(),  r.top()),
            "T":  QPoint(cx,        r.top()),
            "TR": QPoint(r.right(), r.top()),
            "R":  QPoint(r.right(), cy),
            "BR": QPoint(r.right(), r.bottom()),
            "B":  QPoint(cx,        r.bottom()),
            "BL": QPoint(r.left(),  r.bottom()),
            "L":  QPoint(r.left(),  cy),
        }

    def _hit_handle(self, pos: QPoint) -> str | None:
        if not self._sel:
            return None
        for name, pt in self._handles(self._sel).items():
            if (pos - pt).manhattanLength() <= _HANDLE_HIT:
                return name
        return None

    def _move_bar(self, r: QRect) -> QRect:
        """Pasek na górze zaznaczenia służący do przesuwania."""
        return QRect(r.left() + _HANDLE_HIT, r.top() + _HANDLE_HIT,
                     r.width() - 2 * _HANDLE_HIT, _MOVE_BAR_H)

    def _apply_resize(self, pos: QPoint):
        r = QRect(self._sel_at_drag)
        h = self._resize_handle
        if "L" in h: r.setLeft(pos.x())
        if "R" in h: r.setRight(pos.x())
        if "T" in h: r.setTop(pos.y())
        if "B" in h: r.setBottom(pos.y())
        self._sel = r.normalized()

    def _emit_sel(self):
        r = self._sel
        s = self._scale
        self.areaSelected.emit({
            "top":    int(r.top()    * s),
            "left":   int(r.left()   * s),
            "width":  int(r.width()  * s),
            "height": int(r.height() * s),
        })
        self.close()

    # =====================================================
    # ZDARZENIA WEJŚCIA (MYSZ / KLAWIATURA)
    # =====================================================
    def mousePressEvent(self, event):
        if event.button() != Qt.LeftButton:
            return
        pos = event.pos()

        if self._sel:
            # Klik na uchwyt → resize
            h = self._hit_handle(pos)
            if h:
                self._mode          = "resizing"
                self._resize_handle = h
                self._drag_start    = pos
                self._sel_at_drag   = QRect(self._sel)
                return

            # Klik w pasek przesuwania → move
            if self._move_bar(self._sel).contains(pos):
                self._mode        = "moving"
                self._drag_start  = pos
                self._sel_at_drag = QRect(self._sel)
                return

        # Klik poza paskiem / poza zaznaczeniem → rysuj nowy
        self._mode        = "drawing"
        self._sel         = None
        self._drag_start  = pos
        self._current_pos = pos
        self.update()

    def mouseMoveEvent(self, event):
        pos = event.pos()

        if self._mode == "idle":
            if self._sel:
                h = self._hit_handle(pos)
                if h:
                    self.setCursor(_HANDLE_CURSORS[h])
                elif self._move_bar(self._sel).contains(pos):
                    self.setCursor(Qt.SizeAllCursor)
                else:
                    self.setCursor(Qt.CrossCursor)
            else:
                self.setCursor(Qt.CrossCursor)

        elif self._mode == "drawing":
            self._current_pos = pos
            self.update()

        elif self._mode == "moving":
            self._sel = self._sel_at_drag.translated(pos - self._drag_start)
            self.update()

        elif self._mode == "resizing":
            self._apply_resize(pos)
            self.update()

    def mouseReleaseEvent(self, event):
        if event.button() != Qt.LeftButton:
            return
        pos = event.pos()

        if self._mode == "drawing":
            if self._drag_start and self._current_pos:
                rect = QRect(self._drag_start, self._current_pos).normalized()
                if rect.width() >= 10 and rect.height() >= 10:
                    self._sel  = rect
                    self._mode = "idle"
                    self._emit_sel()   # rysowanie nowego → auto-zamknij
                    return
            self._mode        = "idle"
            self._drag_start  = None
            self._current_pos = None
            self.update()

        elif self._mode in ("moving", "resizing"):
            # Nie zamykaj — zostaw otwarte, potwierdź Enterem
            self._mode = "idle"
            self.update()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Escape:
            self.cancelled.emit()
            self.close()
        elif event.key() in (Qt.Key_Return, Qt.Key_Enter):
            if self._sel and self._sel.width() >= 10 and self._sel.height() >= 10:
                self._emit_sel()

    # =====================================================
    # PAINTING
    # =====================================================
    def paintEvent(self, event):
        painter = QPainter(self)
        try:
            painter.setRenderHint(QPainter.Antialiasing)

            area_color = QColor(
                DEBUG_AREA_1_COLOR if self.area_index == 1 else DEBUG_AREA_2_COLOR
            )

            # === TŁO OVERLAY ===
            overlay_bg = QColor(OVERLAY_BG)
            overlay_bg.setAlpha(OVERLAY_BG_ALPHA)
            painter.fillRect(self.rect(), overlay_bg)

            # === SIATKA ===
            grid_color = QColor(255, 255, 255, OVERLAY_GRID_ALPHA)
            painter.setPen(QPen(grid_color, 1))
            for x in range(0, self.width(), 50):
                painter.drawLine(x, 0, x, self.height())
            for y in range(0, self.height(), 50):
                painter.drawLine(0, y, self.width(), y)

            # === NAGŁÓWEK Z INSTRUKCJAMI ===
            self._draw_header(painter, area_color)

            # === RYSOWANIE NOWEGO OBSZARU ===
            if self._mode == "drawing" and self._drag_start and self._current_pos:
                r = QRect(self._drag_start, self._current_pos).normalized()
                painter.setPen(QPen(area_color, 3))
                painter.setBrush(Qt.NoBrush)
                painter.drawRect(r)
                painter.setPen(QColor(TEXT_PRIMARY))
                painter.drawText(r.adjusted(6, -24, 0, 0), f"{r.width()} × {r.height()} px")

            # === AKTYWNE ZAZNACZENIE Z UCHWYTAMI ===
            if self._sel:
                self._draw_selection(painter, self._sel, area_color)

        finally:
            painter.end()

    def _draw_header(self, painter: QPainter, area_color: QColor):
        header_width  = 620
        header_height = 185
        header_x = (self.width() - header_width) // 2
        header_y = 30
        header_rect = QRect(header_x, header_y, header_width, header_height)

        panel_bg = QColor(OVERLAY_PANEL_BG)
        panel_bg.setAlpha(OVERLAY_PANEL_ALPHA)
        painter.setBrush(panel_bg)
        painter.setPen(QPen(area_color, 2))
        painter.drawRoundedRect(header_rect, 10, 10)

        # tytuł
        font = painter.font()
        font.setBold(True)
        font.setPointSize(FONT_LARGE)
        painter.setFont(font)
        painter.setPen(area_color)
        title_y = header_rect.top() + 20
        painter.drawText(
            QRect(header_rect.left(), title_y, header_rect.width(), 30),
            Qt.AlignHCenter,
            f"ZAZNACZANIE OBSZARU {self.area_index}"
        )

        # instrukcje zależne od stanu
        has_sel = self._sel is not None
        if has_sel:
            instructions = [
                (qta.icon("fa5s.arrows-alt",   color=TEXT_PRIMARY), "Przeciągnij niebieski pasek na górze obszaru aby go przesunąć"),
                (qta.icon("fa5s.expand-alt",   color=TEXT_PRIMARY), "Przeciągnij narożnik lub krawędź aby zmienić rozmiar"),
                (qta.icon("fa5s.check-circle", color=TEXT_PRIMARY), "Naciśnij Enter aby zatwierdzić"),
                (qta.icon("fa5s.keyboard",     color=TEXT_PRIMARY), "ESC aby anulować  |  Kliknij poza paskiem aby narysować nowy obszar"),
            ]
        else:
            instructions = [
                (qta.icon("fa5s.mouse-pointer", color=TEXT_PRIMARY), "Przeciągnij myszą aby zaznaczyć nowy obszar"),
                (qta.icon("fa5s.keyboard",      color=TEXT_PRIMARY), "Naciśnij ESC aby anulować"),
                (qta.icon("fa5s.check-circle",  color=TEXT_PRIMARY), "Puść przycisk myszy aby zatwierdzić"),
            ]

        font.setBold(False)
        font.setPointSize(FONT_BASE)
        painter.setFont(font)
        painter.setPen(QColor(TEXT_PRIMARY))

        line_height  = 28
        icon_size    = 18
        text_offset  = 12
        start_y      = title_y + 30 + 24
        base_x       = header_rect.left() + 40

        for i, (icon, text) in enumerate(instructions):
            y       = start_y + i * line_height
            ascent  = painter.fontMetrics().ascent()
            pixmap  = icon.pixmap(icon_size, icon_size)
            icon_y  = y - ascent + (ascent - icon_size) // 2 + OVERLAY_ICON_BASELINE_OFFSET
            painter.drawPixmap(base_x, icon_y, pixmap)
            painter.drawText(base_x + icon_size + text_offset, y, text)

    def _draw_selection(self, painter: QPainter, rect: QRect, area_color: QColor):
        # główna ramka
        painter.setPen(QPen(area_color, 3))
        painter.setBrush(Qt.NoBrush)
        painter.drawRect(rect)

        # pasek do przesuwania
        bar = self._move_bar(rect)
        bar_color = QColor(area_color)
        bar_color.setAlpha(160)
        painter.setBrush(bar_color)
        painter.setPen(Qt.NoPen)
        painter.drawRoundedRect(bar, 3, 3)
        # ikona ⠿ w pasku
        painter.setPen(QColor(255, 255, 255, 200))
        font = painter.font()
        font.setPointSize(8)
        painter.setFont(font)
        painter.drawText(bar, Qt.AlignCenter, "⠿ PRZESUŃ")

        # wymiary
        painter.setPen(QColor(TEXT_PRIMARY))
        f2 = painter.font()
        f2.setPointSize(FONT_BASE)
        painter.setFont(f2)
        painter.drawText(rect.adjusted(6, -24, 0, 0), f"{rect.width()} × {rect.height()} px")

        # uchwyty narożniki / krawędzie
        hs = _HANDLE_SIZE
        painter.setPen(QPen(area_color, 2))
        painter.setBrush(QColor(255, 255, 255, 220))
        for pt in self._handles(rect).values():
            painter.drawRect(pt.x() - hs // 2, pt.y() - hs // 2, hs, hs)