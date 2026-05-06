
# ============================================================
# IMPORTY
# ============================================================
from dataclasses import dataclass
from threading import Lock
from collections import deque
from datetime import datetime
from typing import List

from core.constants import DEBUG_MAX_ENTRIES

# ============================================================
# POZIOMY LOGÓW
# ============================================================
DEBUG = "DEBUG"
INFO = "INFO"
WARNING = "WARNING"
ERROR = "ERROR"

LEVELS = (DEBUG, INFO, WARNING, ERROR)

# ============================================================
# MODEL WPISU
# ============================================================
@dataclass(frozen=True)
class DebugEntry:
    seq: int
    timestamp: datetime
    level: str
    source: str
    message: str

    def format_line(self) -> str:
        time_str = self.timestamp.strftime("%H:%M:%S")
        return f"{time_str} [{self.level}] [{self.source}] - {self.message}"


# ============================================================
# DEBUG SERVICE
# ============================================================
_lock = Lock()
_seq = 0
_entries: deque[DebugEntry] = deque(maxlen=DEBUG_MAX_ENTRIES)

def log(level: str, source: str, message: str):
    global _seq

    if level not in LEVELS:
        level = DEBUG

    with _lock:
        _seq += 1
        entry = DebugEntry(
            seq=_seq,
            timestamp=datetime.now(),
            level=level,
            source=str(source).upper(),
            message=str(message),
        )
        _entries.append(entry)

def get_entries() -> List[DebugEntry]:
    with _lock:
        return list(_entries)

def clear():
    with _lock:
        _entries.clear()

def save_to_file(entries: List[DebugEntry], path: str):
    with open(path, "w", encoding="utf-8") as f:
        for entry in entries:
            f.write(entry.format_line() + "\n")
