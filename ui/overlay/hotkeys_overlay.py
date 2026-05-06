
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtCore import Qt, QTimer, QMetaObject, Q_ARG, Slot
from PySide6.QtGui import QGuiApplication
from PySide6.QtWidgets import QApplication, QWidget, QLabel, QVBoxLayout, QSizePolicy

from core import state
from ui.theme.theme import (
    ACCENT,
    TEXT_PRIMARY,
    FONT_LARGE,
    FONT_BASE,
    RADIUS_MD,
)

# ============================================================
# WIDGET: HotkeysOverlay
# ============================================================
class HotkeysOverlay(QWidget):
    FIXED_WIDTH = 520
    _instance = None

    # =====================================================
    # PUBLIC API
    # =====================================================
    @classmethod
    def init(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    @classmethod
    def show_overlay(cls, key: str, text: str, duration_ms: int = 1500):
        if cls._instance is None:
            app = QApplication.instance()
            if app is None:
                return

            QMetaObject.invokeMethod(app, lambda: cls.init(), Qt.QueuedConnection)
            return

        QMetaObject.invokeMethod(
            cls._instance,
            "_show_queued",
            Qt.QueuedConnection,
            Q_ARG(str, key),
            Q_ARG(str, text),
            Q_ARG(int, duration_ms),
        )

    # =====================================================
    # INIT
    # =====================================================
    def __init__(self):
        super().__init__(
            None,
            Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
        )

        self.setObjectName("hotkeys-overlay")

        self.setAttribute(Qt.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WA_TransparentForMouseEvents)

        self.setAttribute(Qt.WA_TranslucentBackground, True)
        self.setAttribute(Qt.WA_NoSystemBackground, True)

        self._build_ui()
        self.setFixedWidth(self.FIXED_WIDTH)

        self._timer = QTimer(self)
        self._timer.setSingleShot(True)
        self._timer.timeout.connect(self.hide)

    # =====================================================
    # UI
    # =====================================================
    def _build_ui(self):
        root = QVBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)
        root.setAlignment(Qt.AlignCenter)

        self.panel = QWidget(self)
        self.panel.setObjectName("hotkeys-overlay-panel")
        self.panel.setFixedWidth(self.FIXED_WIDTH)

        panel_layout = QVBoxLayout(self.panel)
        panel_layout.setContentsMargins(24, 18, 24, 18)
        panel_layout.setSpacing(10)
        panel_layout.setAlignment(Qt.AlignCenter)

        # === NAGŁÓWEK ===
        self.title_label = QLabel("GameReader", self.panel)
        self.title_label.setAlignment(Qt.AlignCenter)
        self.title_label.setProperty("role", "overlay-title")

        self.key_label = QLabel("", self.panel)
        self.key_label.setAlignment(Qt.AlignCenter)
        self.key_label.setProperty("role", "overlay-key")

        self.text_label = QLabel("", self.panel)
        self.text_label.setAlignment(Qt.AlignCenter)
        self.text_label.setWordWrap(False)

        self.text_label.setFixedHeight(
            self.text_label.fontMetrics().height()
        )

        self.text_label.setSizePolicy(
            QSizePolicy.Expanding,
            QSizePolicy.Fixed
        )
        self.text_label.setProperty("role", "overlay-text")

        panel_layout.addWidget(self.title_label)
        panel_layout.addWidget(self.key_label)
        panel_layout.addWidget(self.text_label)

        root.addWidget(self.panel)

    # =====================================================
    # SLOT (GUI THREAD)
    # =====================================================
    @Slot(str, str, int)
    def _show_queued(self, key: str, text: str, duration_ms: int):
        self.key_label.setText(key)
        self.text_label.setText(text)

        self._move_to_top_center()

        super().show()
        self.raise_()

        self._timer.stop()
        self._timer.start(duration_ms)

    # === POZYCJONOWANIE ===
    def _get_target_screen(self):
        screens = QGuiApplication.screens()
        idx = int(getattr(state, "selected_screen_monitor", 1)) - 1
        if 0 <= idx < len(screens):
            return screens[idx]
        return QGuiApplication.primaryScreen()

    def _move_to_top_center(self):
        screen = self._get_target_screen()
        if not screen:
            return

        try:
            self.winId()
            handle = self.windowHandle()
            if handle is not None:
                handle.setScreen(screen)
        except Exception:
            pass

        geo = screen.availableGeometry()
        x = geo.center().x() - self.width() // 2
        y = geo.top() + 30
        self.move(x, y)
