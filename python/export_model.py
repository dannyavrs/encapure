"""
Export BAAI/bge-reranker-v2-m3 to INT8 quantized ONNX format.

Usage:
    pip install -r requirements.txt
    python export_model.py

Output:
    ../models/model_quantized.onnx
    ../models/tokenizer.json
"""

import os
import shutil
from pathlib import Path

from optimum.onnxruntime import ORTModelForSequenceClassification, ORTQuantizer
from optimum.onnxruntime.configuration import AutoQuantizationConfig
from transformers import AutoTokenizer

MODEL_ID = "BAAI/bge-reranker-v2-m3"
OUTPUT_DIR = Path(__file__).parent.parent / "models"


def main() -> None:
    print(f"Exporting {MODEL_ID} to ONNX with INT8 quantization...")

    # Ensure output directory exists
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # Step 1: Download and export to ONNX
    print("Step 1/4: Downloading model and exporting to ONNX...")
    model = ORTModelForSequenceClassification.from_pretrained(
        MODEL_ID,
        export=True,
        provider="CPUExecutionProvider",
    )

    # Save unquantized model temporarily
    temp_dir = OUTPUT_DIR / "temp_onnx"
    model.save_pretrained(temp_dir)

    # Step 2: Save tokenizer
    print("Step 2/4: Saving tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_ID)
    tokenizer.save_pretrained(OUTPUT_DIR)

    # Step 3: INT8 Dynamic Quantization
    print("Step 3/4: Applying INT8 dynamic quantization...")
    quantizer = ORTQuantizer.from_pretrained(temp_dir)

    # Try AVX512 first (best performance on modern CPUs), fallback to AVX2
    try:
        qconfig = AutoQuantizationConfig.avx512_vnni(is_static=False)
        print("  Using AVX512-VNNI quantization config")
    except Exception:
        qconfig = AutoQuantizationConfig.avx2(is_static=False)
        print("  Using AVX2 quantization config (AVX512 not available)")

    quantizer.quantize(
        save_dir=OUTPUT_DIR,
        quantization_config=qconfig,
    )

    # Step 4: Cleanup and rename
    print("Step 4/4: Cleaning up...")

    # The quantizer outputs model_quantized.onnx, rename to our expected name
    quantized_model_path = OUTPUT_DIR / "model_quantized.onnx"
    if not quantized_model_path.exists():
        # Sometimes it's just model.onnx after quantization
        alt_path = OUTPUT_DIR / "model.onnx"
        if alt_path.exists():
            alt_path.rename(quantized_model_path)

    # Remove temporary unquantized model
    if temp_dir.exists():
        shutil.rmtree(temp_dir)

    # Verify outputs
    final_model = OUTPUT_DIR / "model_quantized.onnx"
    final_tokenizer = OUTPUT_DIR / "tokenizer.json"

    if not final_model.exists():
        raise FileNotFoundError(f"Expected quantized model at {final_model}")
    if not final_tokenizer.exists():
        raise FileNotFoundError(f"Expected tokenizer at {final_tokenizer}")

    model_size_mb = final_model.stat().st_size / (1024 * 1024)
    print(f"\nExport complete!")
    print(f"  Model: {final_model} ({model_size_mb:.1f} MB)")
    print(f"  Tokenizer: {final_tokenizer}")


if __name__ == "__main__":
    main()
