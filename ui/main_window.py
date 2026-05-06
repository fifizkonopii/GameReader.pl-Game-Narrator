
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtGui import QIcon
from PySide6.QtCore import Qt, QSize, Slot, QTimer, QPropertyAnimation, QEasingCurve
from PySide6.QtWidgets import (
    QApplication,
    QMainWindow, QWidget,
    QHBoxLayout, QVBoxLayout,
    QListWidget, QListWidgetItem,
    QGraphicsOpacityEffect,
    QPushButton, QLabel
)

import qtawesome as qta
from core.paths import SOUNDS_DIR

from ui.theme.global_qss import build_global_qss
from ui.theme.theme import (
    ACCENT,
    SUCCESS,
    DANGER,
    TEXT_MUTED,
    TAB_START,
    TAB_SHORTCUTS,
    TAB_FILES,
    TAB_SCREEN,
    TAB_ADVANCED,
    TAB_ABOUT,
    SIDEBAR_WIDTH,
    SIDEBAR_ITEM_HEIGHT,
    MAIN_ACTION_BUTTON_HEIGHT,
    TITLEBAR_HEIGHT,
)

from ui.widgets import (
    SidebarItemDelegate,
    RecentPresetsDialog,
    show_validation_result,
    LoadingOverlay,
    AppFrame,
    AnimatedStackedWidget,
)

from ui.tab.start_tab import StartTab
from ui.tab.shortcuts_tab import ShortcutsTab
from ui.tab.files_tab import FilesTab
from ui.tab.screen_tab import ScreenTab
from ui.tab.advanced_tab import AdvancedTab
from ui.tab.about_tab import AboutTab

from ui.overlay.hotkeys_overlay import HotkeysOverlay
from ui.debug_window import DebugWindow

from input import hotkeys
from presets import manager as presets
from audio import player as audio

from core import app as core_app
from core.constants import APP_NAME
from core.paths import ICON_PATH


# ============================================================
# TITLE BAR
# ============================================================
class TitleBar(QWidget):
    def __init__(self, window: "MainWindow"):
        super().__init__(window)
        self.setObjectName("titlebar")
        self.setFixedHeight(TITLEBAR_HEIGHT)
        self._drag_pos = None
        self._window = window

        root = QHBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)
        root.setSpacing(0)

        # === LEWA SEKCJA: logo + nazwa (tło jak sidebar) ===
        left = QWidget()
        left.setObjectName("titlebar-left")
        left.setFixedWidth(SIDEBAR_WIDTH)
        left_layout = QHBoxLayout(left)
        left_layout.setContentsMargins(0, 0, 0, 0)
        left_layout.setAlignment(Qt.AlignCenter)
        left_layout.setSpacing(8)

        icon_lbl = QLabel()
        icon_lbl.setPixmap(QIcon(ICON_PATH).pixmap(22, 22))

        title_lbl = QLabel(APP_NAME)
        title_lbl.setObjectName("titlebar-title")

        left_layout.addWidget(icon_lbl)
        left_layout.addWidget(title_lbl)

        # === PRAWA SEKCJA: tylko przyciski okna ===
        right = QWidget()
        right.setObjectName("titlebar-right")
        right_layout = QHBoxLayout(right)
        right_layout.setContentsMargins(0, 0, 4, 0)
        right_layout.setSpacing(0)
        right_layout.addStretch()

        self.min_btn   = self._make_btn("mdi.window-minimize", "titlebar-btn")
        self.max_btn   = self._make_btn("mdi.window-maximize", "titlebar-btn")
        self.close_btn = self._make_btn("mdi.window-close",    "titlebar-close-btn")

        self.min_btn.clicked.connect(window.showMinimized)
        self.max_btn.clicked.connect(self._toggle_maximize)
        self.close_btn.clicked.connect(window.close)

        for btn in (self.min_btn, self.max_btn, self.close_btn):
            right_layout.addWidget(btn)

        root.addWidget(left)
        root.addWidget(right)

    def _make_btn(self, icon_name: str, obj_name: str) -> QPushButton:
        btn = QPushButton()
        btn.setObjectName(obj_name)
        btn.setIcon(qta.icon(icon_name, color=TEXT_MUTED))
        btn.setFixedSize(40, TITLEBAR_HEIGHT - 4)
        btn.setCursor(Qt.ArrowCursor)
        return btn

    def _toggle_maximize(self):
        if self._window.isMaximized():
            self._window.showNormal()
            self.max_btn.setIcon(qta.icon("mdi.window-maximize", color=TEXT_MUTED))
        else:
            self._window.showMaximized()
            self.max_btn.setIcon(qta.icon("mdi.window-restore", color=TEXT_MUTED))

    def mousePressEvent(self, event):
        if event.button() == Qt.LeftButton and not self._window.isMaximized():
            event.accept()
            self._window.windowHandle().startSystemMove()
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event):
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event):
        super().mouseReleaseEvent(event)

    def mouseDoubleClickEvent(self, event):
        if event.button() == Qt.LeftButton:
            self._toggle_maximize()
        super().mouseDoubleClickEvent(event)


# ============================================================
# GŁÓWNE OKNO APLIKACJI
# ============================================================
class MainWindow(QMainWindow):
    # === INICJALIZACJA GŁÓWNEGO OKNA ===
    def __init__(self):
        super().__init__()
        HotkeysOverlay.init()
        presets.load_recent_presets()

        self.setWindowIcon(QIcon(ICON_PATH))
        self.setWindowTitle(APP_NAME)
        self.setWindowFlags(Qt.Window | Qt.FramelessWindowHint)
        self.setAttribute(Qt.WA_TranslucentBackground)
        self.resize(1200, 750)

        self.init_ui()
        self.apply_style()

        self.debug_window = DebugWindow()
        self.debug_window.hide()

        core_app.register_main_window(self)
        core_app.ui_bridge.toggle_debug_console.connect(
            self._toggle_debug_console
        )
        core_app.register_capture_state_callback(self.on_capture_state_changed)
        core_app.setup_tray()
        core_app.initialize_backend()

        self._show_recent_presets_dialog()

    # === BUDOWA INTERFEJSU UŻYTKOWNIKA ===
    def init_ui(self):
        # === CENTRALNY WIDGET I UKŁAD ===
        central = QWidget()
        central.setAttribute(Qt.WA_TranslucentBackground)
        central_layout = QVBoxLayout(central)
        central_layout.setContentsMargins(0, 0, 0, 0)
        central_layout.setSpacing(0)

        self._app_frame = AppFrame()
        central_layout.addWidget(self._app_frame)

        root_layout = QVBoxLayout(self._app_frame)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        self.title_bar = TitleBar(self)
        root_layout.addWidget(self.title_bar)

        content = QWidget()
        layout = QHBoxLayout(content)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)
        root_layout.addWidget(content)

        # === SIDEBAR – KONTENER ===
        sidebar_container = QWidget()
        sidebar_container.setObjectName("sidebar-container")
        sidebar_container.setFixedWidth(SIDEBAR_WIDTH)

        sidebar_layout = QVBoxLayout(sidebar_container)
        sidebar_layout.setContentsMargins(0, 0, 0, 0)
        sidebar_layout.setSpacing(6)

        # === SIDEBAR: LISTA ZAKŁADEK ===
        self.sidebar = QListWidget()
        self.sidebar.setIconSize(QSize(22, 22))
        self.sidebar.setItemDelegate(SidebarItemDelegate())
        self.sidebar.setUniformItemSizes(True)
        self.sidebar.setSpacing(4)
        self.sidebar.setFocusPolicy(Qt.NoFocus)
        self.sidebar.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)

        # === PRZYCISKI AKCJI (RUN / STOP) ===
        actions_container = QWidget()
        actions_layout = QVBoxLayout(actions_container)
        actions_layout.setContentsMargins(12, 6, 12, 12)
        actions_layout.setSpacing(6)
        actions_layout.setAlignment(Qt.AlignBottom)

        # === PRZYCISKI LOGICZNE (HOOKI BACKENDU) ===
        self.run_btn = QPushButton()
        self.stop_btn = QPushButton()

        # === PRZYCISK START / STOP ===
        self.action_btn = QPushButton()
        self.action_btn.setCursor(Qt.PointingHandCursor)
        self.action_btn.setMinimumHeight(MAIN_ACTION_BUTTON_HEIGHT)

        actions_layout.addWidget(self.action_btn)

        sidebar_layout.addWidget(self.sidebar)
        sidebar_layout.addWidget(actions_container)

        # === OBSZAR TREŚCI ===
        self.pages = AnimatedStackedWidget()
        self.tabs  = []

        start = StartTab()
        shortcuts = ShortcutsTab()
        files = FilesTab()
        screen = ScreenTab()
        advanced = AdvancedTab()
        about = AboutTab()

        self.tabs.extend([start, shortcuts, files, screen, advanced, about])

        self.add_page("Szybki start", "fa5s.play", TAB_START, start)
        self.add_page("Skróty klawiszowe", "fa5s.keyboard", TAB_SHORTCUTS, shortcuts)
        self.add_page("Foldery i pliki", "fa5s.folder-open", TAB_FILES, files)
        self.add_page("Obszar ekranu", "fa5s.crop-alt", TAB_SCREEN, screen)
        self.add_page("Zaawansowane", "fa5s.cogs", TAB_ADVANCED, advanced)
        self.add_page("O programie", "fa5s.info-circle", TAB_ABOUT, about)

        self.sidebar.itemClicked.connect(
            lambda item: self.pages.setCurrentIndex(self.sidebar.row(item))
        )
        self.sidebar.setCurrentRow(0)

        layout.addWidget(sidebar_container)
        layout.addWidget(self.pages)

        self.is_running = False
        self.update_action_button()
        self.action_btn.clicked.connect(self.on_action_clicked)
        self.run_btn.clicked.connect(self.on_run_requested)
        self.stop_btn.clicked.connect(self.on_stop_requested)

        self.setCentralWidget(central)

        # === LOADING OVERLAY (spinner) ===
        self.loading_overlay = LoadingOverlay(self._app_frame)
        self.loading_overlay.hide()

    # === DODANIE ZAKŁADKI DO SIDEBARA ===
    def add_page(self, title, icon, color, widget):
        item = QListWidgetItem(qta.icon(icon, color=color), title)
        item.setSizeHint(QSize(200, SIDEBAR_ITEM_HEIGHT))
        self.sidebar.addItem(item)
        self.pages.addWidget(widget)

    # === AKTUALIZACJA PRZYCISKU RUN / STOP ===
    def update_action_button(self):
        if self.is_running:
            # === STOP ===
            self.action_btn.setText(" STOP")
            self.action_btn.setIcon(
                qta.icon("fa5s.stop", color=DANGER)
            )
        else:
            # === RUN ===
            self.action_btn.setText(" URUCHOM")
            self.action_btn.setIcon(
                qta.icon("fa5s.play", color=SUCCESS)
            )

    # === CALLBACK: ZMIANA STANU PRZECHWYTYWANIA ===
    def on_capture_state_changed(self, enabled: bool):
        self.is_running = enabled
        self.update_action_button()
        hotkeys.on_reader_state_changed(enabled)

    # === ROUTER AKCJI RUN / STOP ===
    def on_action_clicked(self):
        # === ANULOWANIE ODLICZANIA ===
        if getattr(self, "_countdown_active", False):
            self._cancel_countdown()
            return

        if self.is_running:
            self.stop_btn.clicked.emit()
            return

        # === WALIDACJA PRZED ODLICZANIEM ===
        if not core_app.can_enable_reader():
            return

        self._start_countdown()

    def _cancel_countdown(self):
        self._countdown_active = False
        self.countdown_step = -1
        self.update_action_button()

    def _start_countdown(self):
        self._countdown_active = True
        self.countdown_step = 3
        
        def step():
            if not getattr(self, "_countdown_active", False):
                return

            if self.countdown_step > 0:
                self.action_btn.setText(f" {self.countdown_step}")
                try:
                    audio.play_system_sound(
                        audio.find_system_sound("ping")
                    )
                except Exception:
                    pass
                
                self.countdown_step -= 1
                QTimer.singleShot(1000, step)
            else:
                self._countdown_active = False
                self.update_action_button()
                self.run_btn.clicked.emit()
        
        step()

    # =====================================================
    # GLOBALNY STYL (QSS)
    # =====================================================
    def apply_style(self):
        app = QApplication.instance()
        if app:
            app.setStyleSheet(build_global_qss())

    def show_loading(self, text="Wczytywanie presetu…"):
        if getattr(self, "loading_overlay", None):
            self.loading_overlay.set_message(text)
            self.loading_overlay.attach_to_parent()
            self.loading_overlay.show()
            self.loading_overlay.raise_()

    def hide_loading(self):
        if getattr(self, "loading_overlay", None):
            self.loading_overlay.hide()

    def resizeEvent(self, event):
        super().resizeEvent(event)
        if getattr(self, "loading_overlay", None) and self.loading_overlay.isVisible():
            self.loading_overlay.attach_to_parent()

    def showEvent(self, event):
        super().showEvent(event)
        # self._apply_win32_shadow()
        if not getattr(self, "_shown_once", False):
            self._shown_once = True
            from PySide6.QtCore import QTimer as _QTimer
            _QTimer.singleShot(0, self._window_fade_in)

    def _window_fade_in(self):
        eff = QGraphicsOpacityEffect(self._app_frame)
        eff.setOpacity(0.0)
        self._app_frame.setGraphicsEffect(eff)
        anim = QPropertyAnimation(eff, b"opacity", self)
        anim.setDuration(300)
        anim.setStartValue(0.0)
        anim.setEndValue(1.0)
        anim.setEasingCurve(QEasingCurve.OutCubic)
        anim.finished.connect(lambda: self._app_frame.setGraphicsEffect(None))
        anim.start()
        self._win_anim = anim
    def _apply_win32_shadow(self):
        """Add native drop shadow via DWM on Windows."""
        try:
            import ctypes
            import ctypes.wintypes

            class MARGINS(ctypes.Structure):
                _fields_ = [("cxLeftWidth",   ctypes.c_int),
                            ("cxRightWidth",  ctypes.c_int),
                            ("cyTopHeight",   ctypes.c_int),
                            ("cyBottomHeight",ctypes.c_int)]

            margins = MARGINS(1, 1, 1, 1)
            ctypes.windll.dwmapi.DwmExtendFrameIntoClientArea(
                ctypes.c_int(int(self.winId())), ctypes.byref(margins)
            )
        except Exception:
            pass

    # =====================================================
    # RESIZE (Qt-native, DPI-aware — bez nativeEvent)
    # =====================================================
    _RESIZE_B = 6  # px border dla resize

    def _is_in_titlebar(self, widget) -> bool:
        tb = getattr(self, 'title_bar', None)
        if tb is None:
            return False
        return widget is tb or tb.isAncestorOf(widget)

    def eventFilter(self, obj, event):
        from PySide6.QtCore import QEvent
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
        L  = Qt.Edge.LeftEdge.value
        R  = Qt.Edge.RightEdge.value
        T  = Qt.Edge.TopEdge.value
        B_ = Qt.Edge.BottomEdge.value
        if   (e & T and e & L) or (e & B_ and e & R): cur = Qt.SizeFDiagCursor
        elif (e & T and e & R) or (e & B_ and e & L): cur = Qt.SizeBDiagCursor
        elif e & (L | R):                               cur = Qt.SizeHorCursor
        elif e & (T | B_):                              cur = Qt.SizeVerCursor
        else:                                           cur = Qt.ArrowCursor
        if self.cursor().shape() != cur:
            self.setCursor(cur)

    # =====================================================
    # BACKEND HOOKS
    # =====================================================
    # === ŻĄDANIE URUCHOMIENIA PRZETWARZANIA ===
    def on_run_requested(self):
        core_app.enable_reader()

    # === ŻĄDANIE ZATRZYMANIA PRZETWARZANIA ===
    def on_stop_requested(self):
        core_app.disable_reader()

    # === WYŚWIETLENIE DIALOGU OSTATNICH PRESETÓW ===
    def _show_recent_presets_dialog(self):
        if not presets.has_recent_presets():
            return

        dialog = RecentPresetsDialog(self)
        dialog.loadRequested.connect(self.on_preset_load)

        dialog.exec()

    # === WCZYTANIE PRESETU ===
    def on_preset_load(self, preset_path: str):
        start_tab = next(
            (tab for tab in self.tabs if isinstance(tab, StartTab)),
            None
        )
        if not start_tab:
            return

        start_tab._load_preset_from_path(preset_path)

    # === PRZEŁĄCZENIE KONSOLI DEBUG ===
    def _toggle_debug_console(self):
        if self.debug_window.isVisible():
            self.debug_window.hide()
        else:
            self.debug_window.show()
            self.debug_window.raise_()
            self.debug_window.activateWindow()

    # === ODŚWIEŻENIE UI NA PODSTAWIE STATE ===
    @Slot()
    def reload_ui_from_state(self):
        for tab in self.tabs:
            if hasattr(tab, "reload_ui_from_state"):
                tab.reload_ui_from_state()
            elif hasattr(tab, "reload_from_state"):
                tab.reload_from_state()

    # === ZAMKNIĘCIE OKNA APLIKACJI ===
    def closeEvent(self, event):
        try:
            if self.debug_window:
                self.debug_window.close()
        except Exception:
            pass

        try:
            core_app.disable_reader()
        except Exception:
            pass

        event.accept()
