"""Runtime smoke tests for the ithmb_core Python bindings.

These exercise the real compiled module (via the venv's installed
ithmb_core) — not just that it compiles. Run from the repo root:

    pymod/.venv/bin/python -m pytest pymod/tests/ -v

Requires the module to be built and installed into pymod/.venv:
    (cd pymod && .venv/bin/python -m pip install -e .)   # after pip exists
    # or: maturin develop --uv (with maturin installed)
"""

import os
import sys
from pathlib import Path

import pytest

import ithmb_core

REPO_ROOT = Path(__file__).resolve().parents[2]
SAMPLE_ITHMB = REPO_ROOT / "samples" / "synthetic" / "sample.ithmb"


def test_module_imports():
    """The compiled extension imports and exposes the public API."""
    for name in ("decode_ithmb", "open_ithmb", "list_profiles"):
        assert hasattr(ithmb_core, name), f"missing export: {name}"


def test_list_profiles_nonempty():
    profiles = ithmb_core.list_profiles()
    assert isinstance(profiles, (list, tuple)), "list_profiles should return a list"
    assert len(profiles) > 0, "expected at least one known profile"


def test_decode_ithmb_returns_image():
    """Decode the real synthetic sample and verify the returned dict."""
    assert SAMPLE_ITHMB.exists(), f"sample not found: {SAMPLE_ITHMB}"
    raw = SAMPLE_ITHMB.read_bytes()

    result = ithmb_core.decode_ithmb(raw)

    assert isinstance(result, dict), "decode_ithmb should return a dict"
    width = result["width"]
    height = result["height"]
    assert width > 0 and height > 0, f"expected nonzero dims, got {width}x{height}"

    data = result["data"]
    assert len(data) == width * height * 4, (
        f"pixel buffer length {len(data)} != {width}x{height}x4 = {width * height * 4}"
    )
    assert result["format"] == "BGRA"


def test_decode_ithmb_produces_varied_pixels():
    """The sample is a generated pattern — it must not be flat/empty."""
    raw = SAMPLE_ITHMB.read_bytes()
    result = ithmb_core.decode_ithmb(raw)
    data = result["data"]

    # Sample a grid of pixels; expect more than one distinct color.
    width = result["width"]
    colors = set()
    step = 16
    for y in range(0, result["height"], step):
        for x in range(0, width, step):
            i = (y * width + x) * 4
            colors.add((data[i], data[i + 1], data[i + 2]))
    assert len(colors) >= 2, f"expected varied pattern, got {len(colors)} colors"


def test_decode_garbage_raises():
    garbage = b"\xff" * 64
    with pytest.raises((ValueError, RuntimeError)):
        ithmb_core.decode_ithmb(garbage)


def test_open_ithmb_exists():
    """open_ithmb should be callable (smoke — exact behavior varies by input)."""
    assert callable(ithmb_core.open_ithmb), "open_ithmb must be callable"


if __name__ == "__main__":
    sys.exit(pytest.main([__file__, "-v"]))
