Place OCR models here.

Preferred (FP16 - ~9% faster, ~8% less memory, half the size, no accuracy loss):
- PP-OCRv5_mobile_det_fp16.mnn
- PP-OCRv5_mobile_rec_fp16.mnn
- ppocr_keys_v5.txt

Fallback (standard precision), used only if the FP16 files above are missing:
- PP-OCRv5_mobile_det.mnn
- PP-OCRv5_mobile_rec.mnn
- ppocr_keys_v5.txt

The pipeline loads FP16 first, then falls back to the standard models.

Download from:
- https://github.com/zibo-chen/rust-paddle-ocr/tree/next/models
- https://github.com/RapidAI/RapidOCRDocs/blob/main/docs/blog/mnn_models.md
