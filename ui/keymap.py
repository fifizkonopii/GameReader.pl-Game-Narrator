
# ============================================================
# MAPOWANIE SPECJALNYCH KLAWISZY (QT → WEWNĘTRZNY FORMAT)
# ============================================================
SPECIAL_KEYS = {
    "Del": "delete",
    "Delete": "delete",
    "PgUp": "page_up",
    "PageUp": "page_up",
    "PgDown": "page_down",
    "PageDown": "page_down",
    "Esc": "esc",
    "Escape": "esc",
    "Ins": "insert",
    "Insert": "insert",
    "Home": "home",
    "End": "end",
    "Tab": "tab",
    "Backspace": "backspace",
    "Space": "space",
}

# ============================================================
# KOLEJNOŚĆ MODYFIKATORÓW
# ============================================================
_MOD_ORDER = [
    "ctrl",
    "alt",
    "shift",
]

# ============================================================
# FUNKCJE POMOCNICZE
# ============================================================

# === NORMALIZACJA SEKWENCJI SKRÓTU QT ===
def normalize_qt_sequence(seq: str | None) -> str | None:
    if not seq:
        return None

    parts = seq.lower().split("+")
    mods = []
    key = None

    for part in parts:
        if part in _MOD_ORDER:
            mods.append(part)
        else:
            key = part

    if not key:
        return None

    mods_sorted = [m for m in _MOD_ORDER if m in mods]
    return "+".join(mods_sorted + [key])


# === FORMATOWANIE SKRÓTU DO POSTACI CZYTELNEJ DLA UI ===
def pretty_shortcut(seq: str | None) -> str:
    if not seq:
        return "—"

    parts = seq.split("+")
    pretty = []

    for part in parts:
        if part == "ctrl":
            pretty.append("Ctrl")
        elif part == "alt":
            pretty.append("Alt")
        elif part == "shift":
            pretty.append("Shift")
        else:
            pretty.append(part.upper())

    return " + ".join(pretty)