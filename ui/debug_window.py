
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtGui import QIcon, QFontMetrics
from PySide6.QtCore import (
    Qt, QTimer,
    QAbstractTableModel,
    QModelIndex,
    QSortFilterProxyModel
)
from PySide6.QtWidgets import (
    QVBoxLayout, QHBoxLayout,
    QGroupBox,
    QPushButton, QFileDialog,
    QTableView,
    QHeaderView
)

from ui.widgets import IconComboBox, FocusClearingTab, AppFrame, DialogTitleBar

from core import debug
from core.debug import DEBUG, INFO, WARNING, ERROR, DebugEntry
from core.paths import ICON_PATH

from ui.theme.theme import (
    BG_APP, TEXT_PRIMARY,
    TAB_MARGIN_H, TAB_MARGIN_V, TAB_SPACING,
    BUTTON_GROUP_SPACING
)

# ============================================================
# MODEL: TABELA LOGÓW DEBUG
# ============================================================
class DebugTableModel(QAbstractTableModel):
    COL_TIME = 0
    COL_LEVEL = 1
    COL_SOURCE = 2
    COL_MESSAGE = 3

    # === INICJALIZACJA MODELU ===
    def __init__(self):
        super().__init__()
        self.entries: list[DebugEntry] = []

    # === LICZBA WIERSZY ===
    def rowCount(self, parent=QModelIndex()):
        return len(self.entries)

    # === LICZBA KOLUMN ===
    def columnCount(self, parent=QModelIndex()):
        return 4

    # === DANE KOMÓRKI ===
    def data(self, index: QModelIndex, role=Qt.DisplayRole):
        if not index.isValid():
            return None

        entry = self.entries[index.row()]
        col = index.column()

        if role == Qt.DisplayRole:
            if col == self.COL_TIME:
                return entry.timestamp.strftime("%H:%M:%S")
            if col == self.COL_LEVEL:
                return entry.level
            if col == self.COL_SOURCE:
                return entry.source
            if col == self.COL_MESSAGE:
                return entry.message

        return None

    # === NAGŁÓWKI (UKRYTE W UI) ===
    def headerData(self, section, orientation, role):
        return None

    # === USTAWIENIE NOWYCH WPISÓW ===
    def set_entries(self, entries: list[DebugEntry]):
        self.beginResetModel()
        self.entries = entries
        self.endResetModel()

    def append_entries(self, new_entries: list[DebugEntry], max_entries: int):
        if not new_entries:
            return

        # 1) dodaj nowe na koniec
        start = len(self.entries)
        end = start + len(new_entries) - 1
        self.beginInsertRows(QModelIndex(), start, end)
        self.entries.extend(new_entries)
        self.endInsertRows()

        # 2) utnij z początku, jeśli przekroczono limit
        overflow = len(self.entries) - max_entries
        if overflow > 0:
            self.beginRemoveRows(QModelIndex(), 0, overflow - 1)
            del self.entries[0:overflow]
            self.endRemoveRows()

# ============================================================
# PROXY: FILTROWANIE LOGÓW
# ============================================================
class DebugFilterProxy(QSortFilterProxyModel):
    # === INICJALIZACJA PROXY ===
    def __init__(self):
        super().__init__()
        self.allowed_levels: set[str] = set(debug.LEVELS)
        self.allowed_sources: set[str] = set()

    # === LOGIKA FILTROWANIA WIERSZY ===
    def filterAcceptsRow(self, row, parent):
        model = self.sourceModel()

        if row < 0 or row >= len(model.entries):
            return False

        entry = model.entries[row]

        if entry.level not in self.allowed_levels:
            return False

        if self.allowed_sources and entry.source not in self.allowed_sources:
            return False

        return True

# ============================================================
# OKNO: DEBUG CONSOLE
# ============================================================
class DebugWindow(FocusClearingTab):
    # === INICJALIZACJA OKNA DEBUG ===
    def __init__(self):
        super().__init__()
        self.setWindowIcon(QIcon(ICON_PATH))
        self.setAttribute(Qt.WA_QuitOnClose, False)
        self.setWindowFlags(Qt.Window | Qt.FramelessWindowHint)
        self.setAttribute(Qt.WA_TranslucentBackground)

        self._last_seen_seq = 0

        self.setWindowTitle("GameReader - Debug Console")
        self.resize(1100, 650)
        self.setObjectName("DebugWindow")

        self._build_ui()
        self._setup_timer()

        from PySide6.QtWidgets import QApplication
        QApplication.instance().installEventFilter(self)

    # =====================================================
    # BUDOWA INTERFEJSU UŻYTKOWNIKA
    # =====================================================
    def _build_ui(self):
        outer = QVBoxLayout(self)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)

        self._app_frame = AppFrame()
        outer.addWidget(self._app_frame)

        frame_layout = QVBoxLayout(self._app_frame)
        frame_layout.setContentsMargins(0, 0, 0, 0)
        frame_layout.setSpacing(0)

        frame_layout.addWidget(DialogTitleBar("GameReader - Debug Console", self))

        main = QVBoxLayout()
        main.setContentsMargins(
            TAB_MARGIN_H, TAB_MARGIN_V,
            TAB_MARGIN_H, TAB_MARGIN_V
        )
        main.setSpacing(TAB_SPACING)
        frame_layout.addLayout(main)

        # === FILTRY ===
        filters = QGroupBox("Filtry")
        fl = QHBoxLayout(filters)

        self.level_combo = IconComboBox()
        self.level_combo.addItems(["ALL"] + list(debug.LEVELS))
        self.level_combo.currentTextChanged.connect(self._on_level_changed)

        self.source_combo = IconComboBox()
        self.source_combo.addItem("ALL")
        self.source_combo.currentTextChanged.connect(self._on_source_changed)

        fl.addWidget(self.level_combo)
        fl.addWidget(self.source_combo)
        fl.addStretch()

        main.addWidget(filters)

        # === TABELA LOGÓW ===
        self.model = DebugTableModel()
        self.proxy = DebugFilterProxy()
        self.proxy.setSourceModel(self.model)

        self.table = QTableView()
        self.table.setModel(self.proxy)
        self.table.setSelectionBehavior(QTableView.SelectRows)
        self.table.setSelectionMode(QTableView.ExtendedSelection)
        self.table.setShowGrid(False)
        self.table.verticalHeader().hide()
        self.table.horizontalHeader().hide()
        self.table.setWordWrap(True)
        self.table.setTextElideMode(Qt.ElideNone)
        self.table.setAlternatingRowColors(False)
        self.table.setEditTriggers(QTableView.NoEditTriggers)
        self.table.setFocusPolicy(Qt.NoFocus)
        self.table.verticalHeader().setSectionResizeMode(QHeaderView.Fixed)
        self.table.horizontalHeader().setSectionResizeMode(
            QHeaderView.Fixed
        )

        # TIME
        self.table.setColumnWidth(0, 75)
        # LEVEL (INFO / ERROR / WARNING)
        self.table.setColumnWidth(1, 105)
        # SOURCE
        self.table.setColumnWidth(2, 110)
        # MESSAGE – reszta miejsca
        self.table.horizontalHeader().setStretchLastSection(True)

        logs_box = QGroupBox("Logi debug")
        logs_layout = QVBoxLayout(logs_box)
        logs_layout.setContentsMargins(6, 6, 6, 6)
        logs_layout.addWidget(self.table)

        main.addWidget(logs_box, stretch=1)

        # === AKCJE ===
        actions = QHBoxLayout()
        actions.setSpacing(BUTTON_GROUP_SPACING)

        clear_btn = QPushButton("Wyczyść")
        clear_btn.setProperty("class", "preset")
        clear_btn.clicked.connect(self._clear)

        save_btn = QPushButton("Zapisz do pliku")
        save_btn.setProperty("class", "preset")
        save_btn.clicked.connect(self._save)

        actions.addStretch()
        actions.addWidget(clear_btn)
        actions.addWidget(save_btn)

        main.addLayout(actions)

    # =====================================================
    # TIMER / ODŚWIEŻANIE
    # =====================================================
    def _setup_timer(self):
        self.timer = QTimer(self)
        self.timer.setInterval(150)
        self.timer.timeout.connect(self._refresh)
        self.timer.start()

    def showEvent(self, event):
        super().showEvent(event)
        try:
            import ctypes
            class MARGINS(ctypes.Structure):
                _fields_ = [("cxLeftWidth",    ctypes.c_int),
                             ("cxRightWidth",   ctypes.c_int),
                             ("cyTopHeight",    ctypes.c_int),
                             ("cyBottomHeight", ctypes.c_int)]
            margins = MARGINS(1, 1, 1, 1)
            ctypes.windll.dwmapi.DwmExtendFrameIntoClientArea(
                ctypes.c_int(int(self.winId())), ctypes.byref(margins))
        except Exception:
            pass

    # =====================================================
    # RESIZE (Qt-native, DPI-aware — bez nativeEvent)
    # =====================================================
    _RESIZE_B = 6

    def _is_in_titlebar(self, widget) -> bool:
        from ui.widgets import DialogTitleBar
        return isinstance(widget, DialogTitleBar) or isinstance(widget.parent(), DialogTitleBar)

    def eventFilter(self, obj, event):
        from PySide6.QtCore import QEvent, Qt
        from PySide6.QtWidgets import QWidget
        if isinstance(obj, QWidget) and obj.window() is self:
            t = event.type()
            if t == QEvent.Type.MouseButtonPress and event.button() == Qt.LeftButton:
                if self._is_in_titlebar(obj):
                    return False
                from PySide6.QtGui import QCursor
                lpos = self.mapFromGlobal(QCursor.pos())
                edge = self._resize_edge(lpos)
                if edge:
                    self.windowHandle().startSystemResize(Qt.Edges(edge))
                    return True
            elif t == QEvent.Type.MouseMove:
                from PySide6.QtGui import QCursor
                lpos = self.mapFromGlobal(QCursor.pos())
                self._set_resize_cursor(lpos)
        return False

    def _resize_edge(self, pos) -> int:
        from PySide6.QtCore import Qt
        B = self._RESIZE_B
        x, y, w, h = pos.x(), pos.y(), self.width(), self.height()
        e = 0
        if x < B:     e |= Qt.Edge.LeftEdge.value
        if x > w - B: e |= Qt.Edge.RightEdge.value
        if y > h - B: e |= Qt.Edge.BottomEdge.value
        return e

    def _set_resize_cursor(self, pos):
        from PySide6.QtCore import Qt
        e = self._resize_edge(pos)
        L, R, T, B_ = Qt.Edge.LeftEdge.value, Qt.Edge.RightEdge.value, Qt.Edge.TopEdge.value, Qt.Edge.BottomEdge.value
        if   (e & T and e & L) or (e & B_ and e & R): cur = Qt.SizeFDiagCursor
        elif (e & T and e & R) or (e & B_ and e & L): cur = Qt.SizeBDiagCursor
        elif e & (L | R):                               cur = Qt.SizeHorCursor
        elif e & (T | B_):                              cur = Qt.SizeVerCursor
        else:                                           cur = Qt.ArrowCursor
        if self.cursor().shape() != cur:
            self.setCursor(cur)

    # === ODŚWIEŻENIE DANYCH DEBUG ===
    def _refresh(self):
        # === JEŚLI OKNO NIE JEST WIDOCZNE, POMIŃ ODŚWIEŻANIE ===
        if not self.isVisible():
            return

        entries = debug.get_entries()
        if not entries:
            return

        # === FILTRUJ TYLKO NOWE WPISY PO SEQ ===
        new_entries = [e for e in entries if e.seq > self._last_seen_seq]
        if not new_entries:
            return

        # === SPRAWDŹ, CZY USER BYŁ NA DOLE (AUTO-SCROLL) ===
        scrollbar = self.table.verticalScrollBar()
        was_at_bottom = scrollbar.value() == scrollbar.maximum()

        # === DOPISZ TYLKO NOWE WPISY + UTNIJ DO LIMITU ===
        from core.constants import DEBUG_MAX_ENTRIES
        self.model.append_entries(new_entries, DEBUG_MAX_ENTRIES)

        # === DYNAMICZNE DOPASOWANIE WYSOKOŚCI WIERSZY DO NOWYCH WPISÓW ===
        start_row = self.model.rowCount() - len(new_entries)
        self._resize_rows_for_new_entries(start_row, len(new_entries))

        # === ZAPAMIĘTAJ SEQ OSTATNIEGO WPISU ===
        self._last_seen_seq = new_entries[-1].seq

        # === ZAKTUALIZUJ LISTĘ ŹRÓDEŁ W FILTRZE ===
        self._update_sources({e.source for e in entries})

        # === AUTO-SCROLL TYLKO JEŚLI USER BYŁ NA DOLE ===
        if was_at_bottom:
            self.table.scrollToBottom()

    # =====================================================
    # FILTRY
    # =====================================================
    # === AKTUALIZACJA LISTY ŹRÓDEŁ ===
    def _update_sources(self, sources: set[str]):
        existing = {
            self.source_combo.itemText(i)
            for i in range(self.source_combo.count())
        }
        for src in sorted(sources):
            if src not in existing:
                self.source_combo.addItem(src)

    # === DYNAMICZNE DOPASOWANIE WYSOKOŚCI WIERSZY DO NOWYCH WPISÓW ===
    def _resize_rows_for_new_entries(self, start_row: int, count: int):
        fm = QFontMetrics(self.table.font())
        message_col = 3
        column_width = self.table.columnWidth(message_col)

        for i in range(count):
            row = start_row + i

            index = self.proxy.index(row, message_col)
            text = index.data()

            if not text:
                continue

            rect = fm.boundingRect(
                0, 0,
                column_width,
                10000,
                Qt.TextWordWrap,
                text
            )

            height = rect.height() + 8
            self.table.setRowHeight(row, height)

    # === ZMIANA FILTRA POZIOMU ===
    def _on_level_changed(self, text):
        self.proxy.allowed_levels = (
            set(debug.LEVELS) if text == "ALL" else {text}
        )
        self.proxy.invalidateFilter()
        self.level_combo.clearFocus()

    # === ZMIANA FILTRA ŹRÓDŁA ===
    def _on_source_changed(self, text):
        self.proxy.allowed_sources = (
            set() if text == "ALL" else {text}
        )
        self.proxy.invalidateFilter()
        self.source_combo.clearFocus()

    # =====================================================
    # AKCJE
    # =====================================================

    # === WYCZYSZCZENIE LOGÓW ===
    def _clear(self):
        debug.clear()
        debug.log(INFO, "Debug", "Wyczyszczono bufor logów")
        self.model.set_entries([])
        self._last_seen_seq = 0

    # === ZAPIS LOGÓW DO PLIKU ===
    def _save(self):
        path, _ = QFileDialog.getSaveFileName(
            self,
            "Zapisz logi",
            "debug.log",
            "Log files (*.log *.txt)"
        )
        if not path:
            return

        entries = debug.get_entries()
        debug.save_to_file(entries, path)

        debug.log(INFO, "Debug", f"Zapisano logi do pliku: {path}")
        debug.log(DEBUG, "Debug", f"Liczba zapisanych wpisów: {len(entries)}")
