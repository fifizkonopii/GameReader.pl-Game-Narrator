
# ============================================================
# IMPORTY
# ============================================================
from ui.theme.theme import *

# ============================================================
# GLOBAL QSS
# ============================================================
def build_global_qss() -> str:
    return f"""

    /* =======================================================
     * MAIN WINDOW
     * ======================================================= */

    QMainWindow {{
        background: transparent;
        font-family: {FONT_FAMILY};
        font-size: {FONT_BASE}px;
    }}


    /* =======================================================
    * APP FRAME (zaokrąglone rogi + border)
    * ======================================================= */

    QFrame#app-frame {{
        background-color: {BG_APP};
        border: 1px solid rgba(180, 160, 255, 0.13);
        border-radius: {RADIUS_LG}px;
    }}

    /* =======================================================
    * DEBUG WINDOW
    * ======================================================= */

    QWidget#DebugWindow {{
        background-color: {BG_APP};
    }}


    /* =======================================================
     * DIALOGS
     * ======================================================= */

    QDialog {{
        background-color: {BG_APP};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_LG}px;
    }}

    /* =======================================================
    * NOTIFICATIONS
    * ======================================================= */

    QDialog.notification {{
        background-color: {BG_PANEL};
        border-radius: {RADIUS_LG}px;
        border: 1px solid {BORDER};
    }}

    QLabel.notification-title {{
        font-size: {FONT_LARGE}px;
        font-weight: {FONT_WEIGHT_BOLD};
    }}

    QLabel.notification-message {{
        font-size: {FONT_BASE}px;
        color: {TEXT_PRIMARY};
    }}

    /* =======================================================
     * LIST WIDGET (SIDEBAR)
     * ======================================================= */

    QListWidget {{
        background: {BG_PANEL};
        border: none;
        outline: 0;
        color: {TEXT_PRIMARY};
        font-size: {FONT_BASE}px;
        font-weight: 600;
    }}

    QListWidget::item {{
        padding: {SPACE_SM}px {SPACE_MD}px;
        border-radius: {RADIUS_MD}px;
        border-left: 3px solid transparent;
    }}

    QListWidget::item:hover {{
        background: {BG_HOVER};
        border-left: 3px solid {BORDER_FOCUS};
    }}

    QListWidget::item:selected,
    QListWidget::item:selected:!active {{
        background: {BG_HOVER};
        color: {WHITE};
        border-left: 3px solid {ACCENT};
    }}


    /* =======================================================
     * SCROLLBAR VERTICAL
     * ======================================================= */

    QScrollBar:vertical {{
        background: transparent;
        width: 14px;
        margin: {SPACE_XS}px {SPACE_XXS}px;
    }}

    QScrollBar::handle:vertical {{
        background-color: {BORDER};
        min-height: 32px;
        border-radius: {RADIUS_MD}px;
        margin: 2px;
    }}

    QScrollBar::handle:vertical:hover {{
        background-color: {ACCENT};
    }}

    QScrollBar::handle:vertical:pressed {{
        background-color: {ACCENT_DARK};
    }}

    QScrollBar::add-line:vertical,
    QScrollBar::sub-line:vertical {{
        height: 0;
        background: none;
    }}

    QScrollBar::add-page:vertical,
    QScrollBar::sub-page:vertical {{
        background: none;
    }}


    /* =======================================================
     * SCROLLBAR HORIZONTAL
     * ======================================================= */

    QScrollBar:horizontal {{
        background: transparent;
        height: 10px;
        margin: {SPACE_XXS}px {SPACE_XS}px;
    }}

    QScrollBar::handle:horizontal {{
        background-color: {BORDER};
        min-width: 32px;
        border-radius: {RADIUS_MD}px;
        margin: 2px;
    }}

    QScrollBar::handle:horizontal:hover {{
        background-color: {ACCENT};
    }}

    QScrollBar::handle:horizontal:pressed {{
        background-color: {ACCENT_DARK};
    }}

    QScrollBar::add-line:horizontal,
    QScrollBar::sub-line:horizontal {{
        width: 0;
        background: none;
    }}

    QScrollBar::add-page:horizontal,
    QScrollBar::sub-page:horizontal {{
        background: none;
    }}


    /* =======================================================
     * LABEL
     * ======================================================= */

    QLabel {{
        color: {TEXT_PRIMARY};
    }}


    /* =======================================================
     * GROUP BOX — styl karty
     * ======================================================= */

    QGroupBox {{
        margin-top: {SPACE_MD}px;
        padding: {SPACE_LG}px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_LG}px;
        color: {WHITE};
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QGroupBox::title {{
        subcontrol-origin: margin;
        subcontrol-position: top left;
        margin-left: {SPACE_SM}px;
        padding: 0 {SPACE_SM}px;
        color: {WHITE};
        font-size: {FONT_SMALL}px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QGroupBox QLabel {{
        font-size: {FONT_BASE}px;
        font-weight: {FONT_WEIGHT_REGULAR};
        color: {TEXT_PRIMARY};
    }}


    /* =======================================================
     * PUSH BUTTON
     * ======================================================= */

    QPushButton {{
        padding: {SPACE_SM}px {SPACE_MD}px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER_SUBTLE};
        border-radius: {RADIUS_MD}px;
        color: {WHITE};
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QPushButton:hover {{
        background-color: {ACCENT};
        border-color: {ACCENT};
    }}

    QPushButton:pressed {{
        background-color: {ACCENT_DARK};
        border-color: {ACCENT_DARK};
    }}


    /* =======================================================
     * PRESET BUTTON
     * ======================================================= */

    QPushButton.preset {{
        min-height: {BUTTON_HEIGHT_LG}px;
        padding: 0 {SPACE_MD}px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_MD}px;
        color: {TEXT_PRIMARY};
        font-size: {FONT_BASE}px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QPushButton.preset[size="sm"] {{
        min-height: {BUTTON_HEIGHT_SM}px;
        padding: 0 {SPACE_SM}px;
        font-size: {FONT_SMALL}px;
    }}

    QPushButton.preset:hover {{
        background-color: {BG_HOVER};
        border-color: {ACCENT};
    }}

    QPushButton.preset:pressed {{
        background-color: {ACCENT_DARK};
        color: {WHITE};
    }}

    QPushButton.preset:checked {{
        background-color: {ACCENT};
        border-color: {ACCENT};
        color: {WHITE};
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QPushButton.preset:disabled {{
        background-color: {BG_APP};
        border-color: {BORDER};
        color: {TEXT_MUTED};
    }}

    QPushButton.preset:disabled:hover {{
        background-color: {BG_APP};
        border-color: {BORDER};
    }}


    /* =======================================================
     * COMBO BOX
     * ======================================================= */

    QComboBox {{
        padding: {SPACE_SM}px {SPACE_MD}px;
        padding-right: 28px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER_SUBTLE};
        border-radius: {RADIUS_MD}px;
        color: {TEXT_PRIMARY};
        font-size: {FONT_BASE}px;
    }}

    QComboBox:hover,
    QComboBox:focus {{
        border-color: {ACCENT};
    }}

    QComboBox::drop-down {{
        subcontrol-origin: padding;
        subcontrol-position: top right;
        width: 24px;
        border-left: 1px solid {BORDER_SUBTLE};
    }}

    QComboBox::down-arrow {{
        image: none;
    }}


    /* =======================================================
     * COMBO BOX POPUP
     * ======================================================= */

    QComboBox QAbstractItemView {{
        padding: {SPACE_XXS}px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_MD}px;
        color: {TEXT_PRIMARY};
        selection-background-color: {BG_HOVER};
        selection-color: {WHITE};
        outline: 0;
    }}

    QComboBox QAbstractItemView::viewport {{
        background-color: {BG_CARD};
        border-radius: {RADIUS_MD}px;
    }}

    QComboBox QAbstractItemView::item {{
        padding: {SPACE_XS}px {SPACE_MD}px;
        border-radius: {RADIUS_SM}px;
    }}

    QComboBox QAbstractItemView::item:hover {{
        background-color: {BG_HOVER};
    }}

    QComboBox QAbstractItemView::item:selected {{
        background-color: {ACCENT_DARK};
        color: {WHITE};
    }}

    /* =======================================================
     * INFO / SEPARATOR / STATE
     * ======================================================= */

    QWidget QLabel.info {{
        padding: {SPACE_MD}px;
        margin-bottom: {SPACE_MD}px;
        background-color: {BG_CARD};
        border-left: 3px solid {ACCENT};
        border-radius: {RADIUS_SM}px;
        color: {TEXT_PRIMARY};
        font-size: {FONT_BASE}px;
    }}

    QWidget.separator {{
        min-height: 1px;
        margin: {SPACE_XXS}px 0 {SPACE_XS}px 0;
        background-color: {BORDER};
    }}

    QLabel[state="disabled"] {{
        color: {TEXT_MUTED};
    }}


    /* =======================================================
     * PATH / SHORTCUT FIELDS
     * ======================================================= */

    QLineEdit.path-field,
    QLabel.path-field,
    QLabel.shortcut-field {{
        padding: {SPACE_XS}px {SPACE_SM}px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_MD}px;
        color: {TEXT_PRIMARY};
        font-size: {FONT_BASE}px;
    }}

    QLineEdit.path-field:disabled,
    QLabel.path-field[state="disabled"] {{
        background-color: {BG_APP};
        color: {TEXT_MUTED};
    }}

    QLabel.shortcut-field:hover {{
        border-color: {ACCENT};
    }}

    QLabel.shortcut-field[state="conflict"] {{
        background-color: {DANGER_BG};
        border-color: {DANGER};
        color: {DANGER};
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QLineEdit.path-field[state="error"] {{
        background-color: {DANGER_BG};
        border-color: {DANGER};
        color: {DANGER};
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}


    /* =======================================================
     * OVERLAY / FIXED
     * ======================================================= */

    QFrame.shortcut-overlay {{
        background-color: rgba(15, 15, 20, 0.94);
    }}

    QFrame.shortcut-overlay QLabel {{
        color: {WHITE};
        font-size: 18px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QLabel.fixed-md {{
        min-width: {LABEL_FIXED_WIDTH_MD}px;
        max-width: {LABEL_FIXED_WIDTH_MD}px;
    }}


    /* =======================================================
    * RECENT PRESETS LIST
    * ======================================================= */

    QListWidget.recent-presets {{
        font-weight: 400;
    }}


    /* =======================================================
     * ABOUT TAB
     * ======================================================= */

    QLabel.info-tile {{
        padding: {SPACE_MD}px {SPACE_LG}px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_LG}px;
        color: {TEXT_PRIMARY};
        font-size: {FONT_BASE}px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}

    QLabel.info-tile:hover {{
        background-color: {BG_HOVER};
        border-color: {ACCENT};
    }}

    QLabel.about-version,
    QLabel.about-text,
    QLabel.about-text-muted {{
        font-size: {FONT_SMALL}px;
        font-weight: {FONT_WEIGHT_REGULAR};
    }}

    QLabel.about-version,
    QLabel.about-text-muted {{
        color: {TEXT_MUTED};
    }}

    QLabel.about-text {{
        color: {TEXT_PRIMARY};
    }}


    /* =======================================================
    * TRAY / CONTEXT MENU
    * ======================================================= */

    QMenu {{
        background-color: {BG_CARD};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_LG}px;
        padding: {SPACE_XS}px;
        color: {TEXT_PRIMARY};
        font-size: {FONT_SMALL}px;
    }}

    QMenu::item {{
        padding: {SPACE_XS}px {SPACE_MD}px;
        border-radius: {RADIUS_SM}px;
        background-color: transparent;
    }}

    QMenu::item:selected {{
        background-color: {BG_HOVER};
        color: {WHITE};
    }}

    QMenu::item:pressed {{
        background-color: {ACCENT_DARK};
        color: {WHITE};
    }}

    QMenu::item:disabled {{
        color: {TEXT_MUTED};
        background-color: transparent;
    }}\n
    QMenu::separator {{
        height: 1px;
        background: {BORDER};
        margin: {SPACE_XS}px {SPACE_SM}px;
    }}


    /* =======================================================
    * HOTKEYS OVERLAY HUD
    * ======================================================= */

    QWidget#hotkeys-overlay-panel {{
        background-color: rgba(0, 0, 0, 140);
        border-radius: {RADIUS_LG}px;
    }}

    QWidget#hotkeys-overlay-panel QLabel[role="overlay-title"] {{
        color: {TEXT_PRIMARY};
        font-size: {FONT_LARGE}px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
        letter-spacing: 1px;
    }}

    QWidget#hotkeys-overlay-panel QLabel[role="overlay-key"] {{
        color: {ACCENT};
        font-size: {FONT_TITLE}px;
        font-weight: {FONT_WEIGHT_BOLD};
    }}

    QWidget#hotkeys-overlay-panel QLabel[role="overlay-text"] {{
        color: {TEXT_PRIMARY};
        font-size: {FONT_EXTRALARGE}px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
    }}


    /* =======================================================
    * DEBUG TABLE VIEW
    * ======================================================= */

    QTableView {{
        background-color: {BG_APP};
        color: {TEXT_PRIMARY};
        border: none;
        outline: 0;
        font-family: {FONT_FAMILY};
        font-size: {FONT_SMALL}px;
    }}

    QHeaderView::section {{
        padding: 0px;
        border: none;
        background-color: {BG_PANEL};
        color: {TEXT_MUTED};
    }}

    QTableView::item {{
        padding: 3px 3px;
        border: none;
        outline: 0;
    }}

    QTableView::item:selected {{
        background-color: {BG_HOVER};
        color: {WHITE};
    }}

    QTableView::item:selected:!active {{
        background-color: {BG_HOVER};
        color: {WHITE};
    }}


    /* =======================================================
    * DEBUG WINDOW — COMBOBOX
    * ======================================================= */

    QWidget#DebugWindow QComboBox QAbstractItemView {{
        padding: {SPACE_XXS}px;
        background-color: {BG_CARD};
        border: 1px solid {BORDER};
        border-radius: {RADIUS_MD}px;
        color: {TEXT_PRIMARY};
        selection-background-color: {BG_HOVER};
        selection-color: {WHITE};
        outline: 0;
    }}

    QWidget#DebugWindow QComboBox QListView {{
        background-color: {BG_CARD};
    }}

    QWidget#DebugWindow QComboBox:hover,
    QWidget#DebugWindow QComboBox:focus {{
        border-color: {ACCENT};
    }}

    QWidget#DebugWindow QComboBox::drop-down {{
        subcontrol-origin: padding;
        subcontrol-position: top right;
        width: 24px;
        border-left: 1px solid {BORDER};
    }}

    QWidget#DebugWindow QComboBox::down-arrow {{
        image: none;
    }}

    QWidget#DebugWindow QComboBox QAbstractItemView::viewport {{
        background-color: {BG_CARD};
        border-radius: {RADIUS_MD}px;
    }}

    QWidget#DebugWindow QComboBox QAbstractItemView::item {{
        padding: {SPACE_XS}px {SPACE_MD}px;
        border-radius: {RADIUS_SM}px;
    }}

    QWidget#DebugWindow QComboBox QAbstractItemView::item:hover {{
        background-color: {BG_HOVER};
    }}

    QWidget#DebugWindow QComboBox QAbstractItemView::item:selected {{
        background-color: {ACCENT_DARK};
        color: {WHITE};
    }}


    /* =======================================================
    * LOADING OVERLAY
    * ======================================================= */

    QFrame.loading-overlay {{
        background-color: rgba(0, 0, 0, 160);
    }}

    QFrame.loading-overlay QFrame#loading-panel {{
        background-color: rgba(23, 23, 31, 0.97);
        border: 1px solid {BORDER};
        border-radius: {RADIUS_LG}px;
    }}

    QLabel.loading-label {{
        color: {WHITE};
        font-size: {FONT_BASE}px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
        letter-spacing: 0.5px;
    }}

    QProgressBar.loading-spinner {{
        border: none;
        border-radius: {RADIUS_SM}px;
        background: {BG_HOVER};
    }}

    QProgressBar.loading-spinner::chunk {{
        border-radius: {RADIUS_SM}px;
        background-color: {ACCENT};
    }}


    /* =======================================================
    * TITLE BAR
    * ======================================================= */

    QWidget#titlebar {{
        background: transparent;
    }}

    QWidget#titlebar-left {{
        background-color: {BG_PANEL};
        border-top-left-radius: {RADIUS_LG}px;
    }}

    QWidget#titlebar-right {{
        background-color: {BG_APP};
        border-top-right-radius: {RADIUS_LG}px;
    }}

    QLabel#titlebar-title {{
        color: {TEXT_PRIMARY};
        font-size: {FONT_BASE}px;
        font-weight: {FONT_WEIGHT_SEMIBOLD};
        letter-spacing: 0.3px;
    }}

    QPushButton#titlebar-btn {{
        background: transparent;
        border: none;
        border-radius: {RADIUS_SM}px;
        padding: 4px;
    }}

    QPushButton#titlebar-btn:hover {{
        background: {BG_HOVER};
    }}

    QPushButton#titlebar-btn:pressed {{
        background: {BG_CARD};
    }}

    QPushButton#titlebar-close-btn {{
        background: transparent;
        border: none;
        border-radius: {RADIUS_SM}px;
        padding: 4px;
    }}

    QPushButton#titlebar-close-btn:hover {{
        background: #c0392b;
    }}

    QPushButton#titlebar-close-btn:pressed {{
        background: #922b21;
    }}

    QWidget#simple-titlebar {{
        background-color: {BG_PANEL};
        border-top-left-radius: {RADIUS_LG}px;
        border-top-right-radius: {RADIUS_LG}px;
    }}

    /* =======================================================
    * SIDEBAR CONTAINER
    * ======================================================= */

    QWidget#sidebar-container {{
        background-color: {BG_PANEL};
        border-bottom-left-radius: {RADIUS_LG}px;
    }}
    """
