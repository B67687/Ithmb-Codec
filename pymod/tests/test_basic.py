"""Basic tests for the ithmb-core Python bindings."""

import struct

import pytest

import ithmb_core


def test_list_profiles():
    """list_profiles() returns 54 entries with expected keys."""
    profiles = ithmb_core.list_profiles()
    assert len(profiles) == 54
    for p in profiles:
        assert "name" in p, f"profile missing 'name': {p}"
        assert "width" in p, f"profile missing 'width': {p}"
        assert "height" in p, f"profile missing 'height': {p}"
        assert "encoding" in p, f"profile missing 'encoding': {p}"
        # All fields are populated with meaningful values
        assert p["width"] > 0, f"non-positive width in {p}"
        assert p["height"] > 0, f"non-positive height in {p}"
        assert isinstance(p["name"], str)
        assert isinstance(p["encoding"], str)


def test_decode_small():
    """Decode a known small RGB565 file (profile 1009: 42x30)."""
    prefix = 1009
    width = 42
    height = 30
    pixel_count = width * height
    frame_bytes = pixel_count * 2  # RGB565: 2 bytes per pixel

    # Build a raw .ithmb file: 4-byte prefix + all-white RGB565 pixels
    buf = struct.pack(">i", prefix) + b"\xff" * frame_bytes

    result = ithmb_core.decode_ithmb(buf)
    assert result["width"] == width, f"expected width={width}, got {result['width']}"
    assert result["height"] == height, (
        f"expected height={height}, got {result['height']}"
    )
    assert result["format"] == "BGRA"
    assert result["rotation"] == 0

    # BGRA output: 4 bytes per pixel
    expected_data_len = pixel_count * 4
    assert len(result["data"]) == expected_data_len, (
        f"expected {expected_data_len} bytes, got {len(result['data'])}"
    )

    # All-white RGB565 pixels decode to white BGRA
    for i in range(0, expected_data_len, 4):
        assert result["data"][i : i + 4] == b"\xff\xff\xff\xff", (
            f"pixel at offset {i} is not white"
        )


def test_decode_short_buffer():
    """Feeding short / empty buffers raises ValueError, not crash."""
    # Empty input
    with pytest.raises(ValueError, match="Buffer too short"):
        ithmb_core.decode_ithmb(b"")

    # 3 bytes (below minimum 4)
    with pytest.raises(ValueError, match="Buffer too short"):
        ithmb_core.decode_ithmb(b"\x00\x00\x00")

    # 4 bytes with unknown prefix
    with pytest.raises((ValueError, RuntimeError), match="unknown|Unsupported"):
        ithmb_core.decode_ithmb(b"\xde\xad\xbe\xef")


def test_open_ithmb_bare_file():
    """open_ithmb with a bare .ithmb file returns one frame."""
    prefix = 1009
    width = 42
    height = 30
    frame_bytes = width * height * 2
    buf = struct.pack(">i", prefix) + b"\xff" * frame_bytes

    results = ithmb_core.open_ithmb(buf)
    assert len(results) == 1, f"expected 1 frame, got {len(results)}"
    assert results[0]["width"] == width
    assert results[0]["height"] == height
    assert results[0]["format"] == "BGRA"


def test_open_ithmb_empty():
    """open_ithmb with empty input raises an error."""
    with pytest.raises((ValueError, RuntimeError)):
        ithmb_core.open_ithmb(b"")
