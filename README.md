GameReader 1.0.0 Pre-Release
🚧 Pre-Release

GameReader 1.0.0 to pierwsza wersja całkowicie przepisana od zera. Projekt został zbudowany na nowoczesnym stosie technologicznym:

Rust
Tauri
TypeScript
HTML

Po ludzku: zamiast robić kolejny męczący remont i bez końca łatać stary dom, po prostu powstał nowy, nowoczesny dom zbudowany od fundamentów. Funkcjonalność pozostała ta sama, ale stabilność, wydajność i jakość wykonania są na zupełnie innym poziomie.

✨ Najważniejsze zmiany
🔍 Nowy silnik OCR

System rozpoznawania tekstu został całkowicie przebudowany.

dokładniejsze odczytywanie tekstu,
znacznie mniejsze zużycie procesora,
szybsza reakcja programu.
🎯 Przechwytywanie pojedynczego okna

GameReader analizuje wyłącznie wybrane okno gry zamiast skanować cały pulpit.

Korzyści:

niższe zużycie zasobów,
większa stabilność,
lepsza dokładność działania.
📐 Automatyczne dopasowanie rozdzielczości

Program automatycznie dostosowuje swoje parametry do aktualnej rozdzielczości gry, dzięki czemu konfiguracja jest prostsza i bardziej niezawodna.

🔊 Usprawniony system audio

Usunięto stary system audio oraz zbędne pliki (w tym output2).

Funkcja redukcji głośności:

wycisza wyłącznie grę,
nie wpływa na głośność całego systemu Windows.
📖 Obsługa wielu obszarów tekstowych

Jeżeli preset gry posiada dwa zdefiniowane obszary tekstowe, GameReader automatycznie odczytuje oba jednocześnie.

👤 Nowy system usuwania imion

Mechanizm usuwania imion został całkowicie przebudowany.

Program automatycznie usuwa imiona z tekstu niezależnie od tego, czy plik z listą istnieje.

⌨️ Poprawione skróty klawiszowe

Hotkeye działają szybciej i stabilniej, również wtedy, gdy gra działa w tle.

🌙 Nowy interfejs użytkownika

Dodano obsługę motywów:

Dark Mode
Light Mode

Interfejs został odświeżony i dostosowany do nowej architektury programu.

🧠 Optymalizacja pamięci RAM

Naprawiono problem wycieku pamięci (memory leak), który u części użytkowników powodował nadmierne zużycie RAM.

Nowa wersja zapewnia:

stabilne działanie,
niski pobór pamięci,
lepszą wydajność podczas długiej pracy.
🚀 I wiele więcej

GameReader 1.0.0 to nie tylko lista zmian widocznych na pierwszy rzut oka. Pod maską znalazły się dziesiątki mniejszych poprawek, optymalizacji i usprawnień, które mają na celu zwiększenie stabilności oraz komfortu użytkowania.

⚠️ Ważna informacja

To nadal wersja pre-release.

Mogą występować błędy, niedziałające funkcje lub problemy, które nie zostały jeszcze wykryte podczas testów.

Jeżeli zauważysz nieprawidłowe działanie programu:

napisz na Discordzie na kanale #praca-w-toku,
lub wyślij zgłoszenie na adres e-mail projektu.

Każde zgłoszenie pomaga ulepszać GameReadera i przygotować stabilną wersję końcową.