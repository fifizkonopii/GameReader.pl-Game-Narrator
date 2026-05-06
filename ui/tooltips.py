
TOOLTIPS = {

    # ======================================================================
    # NAVIGATION BAR (MainWindow)
    # ======================================================================
    "run": (
        "Uruchamia lektora i rozpoczyna analizę obrazu z gry.\n"
        "GameReader zacznie wykrywać i odczytywać dialogi."
    ),
    "stop": (
        "Zatrzymuje lektora i kończy analizę obrazu.\n"
        "Wszystkie aktywne operacje zostaną przerwane."
    ),

    # ======================================================================
    # START TAB
    # ======================================================================
    "resolution": (
        "Wybierz rozdzielczość, w jakiej uruchomiona jest gra."
    ),
    "monitor": (
        "Monitor, na którym wyświetlana jest gra.\n"
        "Ważne przy konfiguracjach z więcej niż jednym monitorem."
    ),
    "lock_scaling": (
        "Blokuje automatyczne przeliczanie obszaru przechwytywania\n"
        "po zmianie rozdzielczości lub monitora."
    ),
    
    # ======================================================================
    # SHORTCUTS TAB
    # ======================================================================
    "toggle_reader": (
        "Włącza lub wyłącza lektora i proces analizy dialogów."
    ),

    "volume_up": (
        "Zwiększa głośność głosu lektora."
    ),
    "volume_down": (
        "Zmniejsza głośność głosu lektora."
    ),

    "interrupt_audio": (
        "Natychmiast przerywa aktualnie czytaną kwestię."
    ),
    "test_sound": (
        "Odtwarza dźwięk testowy lektora lub aktualnie wykrywany dialog."
    ),

    "base_speed_up": (
        "Zwiększa podstawową prędkość mówienia lektora o 0.01."
    ),
    "base_speed_down": (
        "Zmniejsza podstawową prędkość mówienia lektora o 0.01."
    ),

    "overlap_speed_up": (
        "Zwiększa prędkość lektora w trybie przyśpieszonym o 0.01."
    ),
    "overlap_speed_down": (
        "Zmniejsza prędkość lektora w trybie przyśpieszonym o 0.01."
    ),

    "toggle_areas": (
        "Pokazuje lub ukrywa zaznaczone obszary przechwytywania na ekranie.\n"
        "Przydatne do konfiguracji i debugowania."
    ),
    "switch_monitor_toggle": (
        "Przełącza aktualnie aktywny obszar przechwytywania.\n"
        "Ważne, jeżeli są aktywne dwa obszary w ustawieniach programu."
    ),
    "open_settings": (
        "Otwiera główne okno ustawień GameReader.\n"
        "Podczas gry lektor jest włączony następuje automatyczne wyłączenie przechwytywania"
    ),
    "debug_console": (
        "Otwiera konsolę debugowania aplikacji."
    ),

    # ======================================================================
    # FILES TAB
    # ======================================================================
    "files_remove_character_names": (
        "Usuwa imiona postaci z napisów (np. 'Geralt:')\n"
        "Wymaga pliku z listą nazw postaci."
    ),

    "files_save_screenshots": (
        "Zapisuje zrzuty ekranu w momencie wykrycia tekstu.\n"
        "Przydatne do debugowania oraz sprawdzania poprawności działania linii pomocniczych."
    ),

    "files_audio_folder": (
        "Folder z plikami audio odtwarzanymi przez lektora.\n"
        "Pliki muszą być zgodne z formatem GameReader."
    ),

    "files_subtitles_file": (
        "Plik z napisami używanymi do analizy dialogów."
    ),

    "files_characters_file": (
        "Plik zawierający listę nazw postaci.\n"
        "Używany do filtrowania lub usuwania imion z dialogów."
    ),

    "files_screenshots_folder": (
        "Folder, w którym zapisywane są zrzuty ekranu."
    ),

    # ======================================================================
    # SCREEN TAB
    # ======================================================================
    "screen_area_top": (
        "Górna krawędź obszaru przechwytywania.\n"
        "Wartość liczona od górnej krawędzi ekranu (px)."
    ),

    "screen_area_left": (
        "Lewa krawędź obszaru przechwytywania.\n"
        "Wartość liczona od lewej krawędzi ekranu (px)."
    ),

    "screen_area_height": (
        "Wysokość obszaru przechwytywania w pikselach (px)."
    ),

    "screen_area_width": (
        "Szerokość obszaru przechwytywania w pikselach (px)."
    ),

    "screen_area2_toggle": (
        "Aktywuje drugi obszar przechwytywania.\n"
        "Można analizować różne typy tekstów niezależnie."
    ),

    # ======================================================================
    # ADVANCED TAB
    # ======================================================================
    "advanced_ocr_quality": (
        "Skalowanie obrazu używanego do OCR.\n"
        "Niższa wartość = mniejsze obciążenie CPU, gorsza dokładność rozpoznawania.\n"
        "Wyższa wartość = większe obciążenie CPU, lepsza dokładność rozpoznawania."
    ),

    "advanced_capture_interval": (
        "Czas pomiędzy kolejnymi analizami obrazu.\n"
        "Niższa wartość = szybsza reakcja, większe obciążenie CPU."
        "500ms = 0.5 sek."
    ),

    "advanced_min_height": (
        "Minimalna wysokość wykrywanego tekstu (px).\n"
        "Zbyt niska wartość może powodować fałszywe wykrycia."
    ),

    "advanced_max_height": (
        "Maksymalna wysokość wykrywanego tekstu (px)."
    ),

    "advanced_line_width": (
        "Szerokość pionowej linii pomocniczej przechodzącej przez środek obszaru przechwytywania\n"
        "Tekst musi ją przeciąć, aby został uznany za dialog."
    ),

    "advanced_line_left_2": (
        "Pozycja drugiej linii pomocniczej (px od lewej).\n"
        "np. 1 to pierwszy piksel od lewej strony."
    ),

    "advanced_line_left_3": (
        "Pozycja trzeciej linii pomocniczej wyrażona jako procent szerokości obszaru.\n"
        f"np. 0.3 to 30% szerokości obszaru przechwytywania"
    ),

    "advanced_tts_speed": (
        "Podstawowa prędkość mówienia lektora.\n"
        "1.0 = normalna prędkość, 1.2 = maksymalna prędkość"
    ),

    "advanced_tts_boost_speed": (
        "Prędkość mówienia lektora w trybie przyśpieszonym.\n"
        "Gdy lektor nie zdążył przeczytać dialogu do końca, a pojawił się już nastepny dialog\n"
        "wtedy kolejna kwestia czytana jest w przyśpieszonym tempie w celu nadrobienia."
    ),

    "advanced_game_ducking": (
        "Poziom wyciszenia dźwięków gry podczas mówienia lektora.\n"
        "0 = brak wyciszenia, 0.2 = 20% wyciszenia."
    ),

    "advanced_memory_lines": (
        "Maksymalna liczba dialogów, które lektor może zapamiętać.\n"
        "Maksymalna ilość dialogów = 3"
    ),

    "advanced_typewriter_wait": (
        "Tryb animowania napisów.\n"
        "Gdy gra wyświetla tekst animując go znak po znaku od lewej,\n"
        "GameReader czeka aż tekst przestał rosnąć zanim dopasuje dialog.\n"
        "Zapobiega odpalaniu audio w połowie animacji."
    ),

    "advanced_paragraph_ocr": (
        "Wykrywa wiele dialogów jednocześnie na ekranie.\n"
        "OCR dzieli tekst na osobne grupy na podstawie przerw pionowych\n"
        "i dopasowuje każdą grupę niezależnie do listy dialogów."
    ),

    "advanced_helper_line_1": (
        "Aktywuje pierwszą linię pomocniczą (centralną).\n"
        "Filtr dialogów przechodzących przez środkową linię obszaru przechwytywania."
    ),

    "advanced_helper_line_2": (
        "Aktywuje drugą linię pomocniczą.\n"
        "Filtr dialogów przechodzących przez druga linię pomocniczą."
    ),

    "advanced_helper_line_3": (
        "Aktywuje trzecią linię pomocniczą.\n"
        "Filtr dialogów przechodzących przez trzecią linię pomocniczą."
    ),
    "advanced_audio_dynamic": (
        "NOWY SYSTEM\n"
        "Użytkownik sam może dostosować prędkość lektora.\n"
        "Pliki audio są przyśpieszane automatycznie. (Wymaga tylko plików audio 'output1')"
    ),
    "advanced_audio_static": (
        "STARY SYSTEM\n"
        "Gdy lektor nie zdążył odczytać do końca dialogu, a pojawia się kolejna kwestia\n"
        "wówczas kolejny odtworzony plik będzie to wersja przyśpieszona 'output2'\n"
        "Wymaga dwóch typów plików audio 'output1' oraz 'output2'"
    ),
    "advanced_tts_requires_dynamic": (
        "Ta opcja wymaga włączonego systemu dynamicznej prędkości."
    ),

    # ======================================================================
    # ADVANCED TAB - ERROR VALIDATION
    # ======================================================================
    "ERROR_advanced_ocr_quality": (
        "BŁĄD\n"
        "Akceptowany zakres: 0.1 – 1.0\n"
        "Przykład: 0.5 lub 0.55"
    ),

    "ERROR_advanced_capture_interval": (
        "BŁĄD\n"
        "Akceptowany zakres: 100 – 5000 ms\n"
        "Przykład: 500"
    ),

    "ERROR_advanced_min_height": (
        "BŁĄD\n"
        "Akceptowany zakres: 1 – 9999 px\n"
        "Przykład: 1000"
    ),

    "ERROR_advanced_max_height": (
        "BŁĄD\n"
        "Akceptowany zakres: 1 – 9999 px\n"
        "Wartość musi być ≥ wysokości minimalnej.\n"
        "Przykład: 2000"
    ),

    "ERROR_advanced_line_width": (
        "BŁĄD\n"
        "Akceptowany zakres: 1 – 9999 px\n"
        "Przykład: 2000"
    ),

    "ERROR_advanced_line_left_2": (
        "BŁĄD\n"
        "Akceptowany zakres: 1 – 9999 px\n"
        "Przykład: 2000"
    ),

    "ERROR_advanced_line_left_3": (
        "BŁĄD\n"
        "Akceptowany zakres: 0.1 – 1.0 %\n"
        "Przykład: 0.5 lub 0.55"
    ),

    "ERROR_advanced_tts_speed": (
        "BŁĄD\n"
        "Akceptowany zakres: 0.8 – 1.2\n"
        "Przykład: 1 lub 1.10"
    ),

    "ERROR_advanced_tts_boost_speed": (
        "BŁĄD\n"
        "Akceptowany zakres: 1.0 – 3.0\n"
        "Przykład: 1 lub 2.25"
    ),

    "ERROR_advanced_game_ducking": (
        "BŁĄD\n"
        "Akceptowany zakres: 0.0 – 1.0\n"
        "Przykład: 0.1 lub 0.85"
    ),

    # ======================================================================
    # POZOSTAŁE TOOLTIPY W GUI
    # ======================================================================
    "recent_presets_dialog": (
        "To lista ostatnio używanych przez Ciebie presetów.\n\n"
        "Przycisk „Wczytaj” – wczyta wybrany preset\n"
        "Przycisk „Usuń” – usuwa tylko wpis z listy\n"
        "Przycisk „Wyczyść” – czyści całą historię\n"
        "Lub po prostu zamknij to okno, aby przejść dalej.\n\n"
        "Pliki presetów nie są usuwane z dysku!"
    ),
}

