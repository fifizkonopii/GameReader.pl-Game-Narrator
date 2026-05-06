
# ============================================================
# IMPORTY
# ============================================================
from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QPixmap, QDesktopServices
from PySide6.QtWidgets import (
    QWidget, QLabel,
    QVBoxLayout, QHBoxLayout
)

from ui.theme.theme import TAB_MARGIN_H, TAB_MARGIN_V, TAB_SPACING

from core.paths import asset_path
from core.constants import APP_VERSION, APP_VERSION_TAG

# ============================================================
# ZAKŁADKA: O PROGRAMIE
# ============================================================
class AboutTab(QWidget):
    LINK_STYLE = """
    <style>
    a {
        color: #b2bec3;
        text-decoration: none;
        font-weight: 600;
    }
    a:hover {
        color: #74b9ff;
    }
    a:visited {
        color: #b2bec3;
    }
    </style>
    """

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

        # =====================================================
        # LOGO
        # =====================================================
        logo_label = QLabel()
        logo_label.setAlignment(Qt.AlignCenter)

        logo_path = asset_path("images", "logo.png")
        pixmap = QPixmap(logo_path)

        if pixmap.isNull():
            logo_label.setText("❌ LOGO NIE ZAŁADOWANE")
        else:
            logo_label.setPixmap(
                pixmap.scaledToWidth(
                    210,
                    Qt.SmoothTransformation
                )
            )

        # =====================================================
        # WERSJA
        # =====================================================
        version_text = f"v{APP_VERSION}"
        if APP_VERSION_TAG:
            version_text += f" ({APP_VERSION_TAG})"

        version_label = QLabel(f"Wersja programu: <b>{version_text}</b>")
        version_label.setProperty("class", "about-version")
        version_label.setAlignment(Qt.AlignCenter)

        # =====================================================
        # OPIS KRÓTKI
        # =====================================================
        short_desc = QLabel(
            "GameReader to nie tylko filmowe doświadczenie, ale także realne wsparcie\n"
            "dla osób z niepełnosprawnościami oraz osób starszych, dla których czytanie\n"
            "napisów dialogowych w grach stanowi duże utrudnienie."
        )
        short_desc.setProperty("class", "about-text")
        short_desc.setWordWrap(True)
        short_desc.setAlignment(Qt.AlignCenter)

        # =====================================================
        # HISTORIA / IDEA
        # =====================================================
        long_desc = QLabel(
            "<b>Wdzięczność społeczności niepełnosprawnych</b><br>"
            "Najbardziej wzruszającym aspektem projektu jest reakcja użytkowników. Choć pierwotnie chodziło głównie o nostalgiczne<br>"
            "wrażenia i przyjemność artystyczną, GameReader szybko okazał się mieć ogromne znaczenie społeczne, ułatwiając<br>"
            "dostępność gier osobom z trudnościami w czytaniu lub śledzeniu napisów."
        )
        long_desc.setProperty("class", "about-text")
        long_desc.setWordWrap(True)
        long_desc.setAlignment(Qt.AlignCenter)

        team_label = QLabel(
            """
            <!-- TABELA 1: PIERWSZA LINIA (3 OSOBY) -->
            <table align="center" cellspacing="20" style="margin-top:2px">
                <tr>
                    <td width="180" style="text-align:center">
                        <b>RafkoStyle</b><br>
                        <span style="color:#dfe6e9">Rafał Kobyliński</span><br>
                        <span style="font-size:13px; color:#b2bec3">
                            Project Lead | Developer
                        </span>
                    </td>

                    <td width="180" style="text-align:center">
                        <b>FiFizKonopii</b><br>
                        <span style="color:#dfe6e9">Filip Frontczak</span><br>
                        <span style="font-size:13px; color:#b2bec3">
                            Co-Founder | Developer
                        </span>
                    </td>

                    <td width="180" style="text-align:center">
                        <b>MAZNET</b><br>
                        <span style="color:#dfe6e9">Mateusz Mazur</span><br>
                        <span style="font-size:13px; color:#b2bec3">
                            Developer | UX Designer
                        </span>
                    </td>
                </tr>
            </table>

            <!-- TABELA 2: DRUGA LINIA (2 OSOBY) -->
            <table align="center" cellspacing="20" style="margin-top:8px">
                <tr>
                    <td width="180" style="text-align:center">
                        <b>Niko</b><br>
                        <span style="font-size:13px; color:#b2bec3">
                            Linux Support
                        </span>
                    </td>

                    <td width="180" style="text-align:center">
                        <b>Skipper499</b><br>
                        <span style="font-size:13px; color:#b2bec3">
                            Discord Support
                        </span>
                    </td>
                </tr>
            </table>
            """
        )

        team_label.setContentsMargins(0, 0, 0, 0)
        team_label.setAlignment(Qt.AlignCenter)
        team_label.setTextFormat(Qt.RichText)

        # =====================================================
        # KAFELKI – SPOŁECZNOŚĆ I ZASOBY (2x2)
        # =====================================================
        tiles_container = QVBoxLayout()
        tiles_container.setSpacing(12)

        # === RZĄD 1 ===
        tiles_row_1 = QHBoxLayout()
        tiles_row_1.setSpacing(14)

        tiles_row_1.addWidget(
            self._info_tile(
                "Strona projektu",
                "www.gamereader.pl",
                "https://www.gamereader.pl"
            )
        )

        tiles_row_1.addWidget(
            self._info_tile(
                "Discord",
                "Dołącz do społeczności",
                "https://discord.com/invite/AuyrJdahfA"
            )
        )

        # === RZĄD 2 ===
        tiles_row_2 = QHBoxLayout()
        tiles_row_2.setSpacing(14)

        tiles_row_2.addWidget(
            self._info_tile(
                "Biblioteka gier",
                "Oficjalna lista wspieranych tytułów",
                "https://www.gamereader.pl/biblioteka-gier"
            )
        )

        tiles_row_2.addWidget(
            self._info_tile(
                "Gry społeczności",
                "Projekty tworzone przez graczy",
                "https://www.gamereader.pl/wasze-gry"
            )
        )

        tiles_container.addLayout(tiles_row_1)
        tiles_container.addLayout(tiles_row_2)


        # =====================================================
        # WSPARCIE
        # =====================================================
        support_label = QLabel(
            self.LINK_STYLE +
            "<b>Jeśli chcesz wesprzeć rozwój projektu:</b><br>"
            "❤️ <a href='https://patronite.pl/rafkostyle'>Patronite</a> | "
            "☕ <a href='https://buycoffee.to/rafkostyle'>BuyCoffee</a> | "
            "▶️ <a href='https://www.youtube.com/@rafkostyle'>YouTube</a><br>"
            "<span style='color:#b2bec3'>Kontakt: gamereader.pl@gmail.com</span>"
        )
        support_label.setAlignment(Qt.AlignCenter)
        support_label.setTextFormat(Qt.RichText)
        support_label.setOpenExternalLinks(True)

        main_layout.addWidget(logo_label)
        main_layout.addWidget(version_label)
        main_layout.addWidget(short_desc)
        main_layout.addWidget(long_desc)
        separator = QWidget()
        separator.setProperty("class", "separator")

        main_layout.addWidget(separator)
        main_layout.addWidget(team_label)
        main_layout.addSpacing(16)
        main_layout.addLayout(tiles_container)
        main_layout.addSpacing(18)
        separator = QWidget()
        separator.setProperty("class", "separator")

        main_layout.addWidget(separator)
        main_layout.addWidget(support_label)
        main_layout.addStretch()

    # =====================================================
    # HELPERS
    # =====================================================
    def _info_tile(self, title: str, subtitle: str, url: str) -> QLabel:
        label = QLabel(
            f"<b>{title}</b><br><span style='font-size:13px'>{subtitle}</span>"
        )
        label.setProperty("class", "info-tile")
        label.setAlignment(Qt.AlignCenter)
        label.setCursor(Qt.PointingHandCursor)
        label.setTextFormat(Qt.RichText)

        label.mousePressEvent = lambda e: QDesktopServices.openUrl(QUrl(url))
        return label
    
    def _team_tile(self, nick: str, name: str, role: str, color: str) -> QLabel:
        label = QLabel(
            f"""
            <div style="line-height:1.35">
                <b>{nick}</b><br>
                <span style="font-size:13px; color:#dfe6e9">{name}</span><br>
                <span style="
                    display:inline-block;
                    margin-top:4px;
                    padding:2px 8px;
                    border-radius:10px;
                    font-size:11px;
                    background:{color};
                    color:#2d3436;
                    font-weight:600;
                ">
                    {role}
                </span>
            </div>
            """
        )
        label.setAlignment(Qt.AlignCenter)
        label.setProperty("class", "info-tile")
        label.setTextFormat(Qt.RichText)
        return label
