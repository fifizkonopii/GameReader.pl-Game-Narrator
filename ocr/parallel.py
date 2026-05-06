
# ============================================================
# IMPORTY
# ============================================================
from multiprocessing import Pool
import os
from typing import List, Any

# ============================================================
# ZMIENNE GLOBALNE I FUNKCJE POMOCNICZE
# ============================================================
READER = None
THREAD_LIMITS = None

# === Inicjalizator dla każdego procesu roboczego ===
def _init_worker(langs, gpu, threads_per_process):
    global READER, THREAD_LIMITS
    THREAD_LIMITS = int(threads_per_process)

    os.environ["OMP_NUM_THREADS"] = str(THREAD_LIMITS)
    os.environ["OPENBLAS_NUM_THREADS"] = str(THREAD_LIMITS)
    os.environ["MKL_NUM_THREADS"] = str(THREAD_LIMITS)
    os.environ["VECLIB_MAXIMUM_THREADS"] = str(THREAD_LIMITS)
    os.environ["NUMEXPR_NUM_THREADS"] = str(THREAD_LIMITS)

    try:
        import torch
        try:
            torch.set_num_threads(THREAD_LIMITS)
            torch.set_num_interop_threads(THREAD_LIMITS)
        except Exception:
            pass
    except Exception:
        pass

    try:
        import easyocr
        READER = easyocr.Reader(langs, gpu=gpu)
    except Exception as e:
        READER = None
        raise

# === Funkcja do przetwarzania pojedynczego obrazu w procesie roboczym ===
def _process_one(path: str):
    global READER, THREAD_LIMITS
    if READER is None:
        raise RuntimeError("READER not initialized in worker")

    # === Jeśli threadpoolctl jest dostępny, ograniczamy wątki dla BLAS/OpenMP/PyTorch w tym procesie ===
    try:
        from threadpoolctl import threadpool_limits
    except Exception:
        threadpool_limits = None

    if threadpool_limits is not None:
        with threadpool_limits(limits=THREAD_LIMITS):
            return READER.readtext(path)
    else:
        return READER.readtext(path)

# === Główna funkcja do przetwarzania listy obrazów ===
def process_images(paths: List[str], langs: List[str] = ['en'], gpu: bool = False, num_processes: int = 4, threads_per_process: int = 3) -> List[Any]:
    if not paths:
        return []

    # === Uruchamiamy pulę procesów roboczych z inicjalizatorem, który ustawia globalny READER i ogranicza wątki ===
    with Pool(processes=num_processes, initializer=_init_worker, initargs=(langs, gpu, threads_per_process)) as p:
        results = p.map(_process_one, paths)
    return results
