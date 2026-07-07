# Contributions to the .ithmb Ecosystem

Beyond building a working codec, this project made several original contributions to the .ithmb reverse-engineering space.

## Format Research

**Multi-OSS format consolidation** — Extracted format tables from **22 independent implementations** (iOpenPod, libgpod, Keith's iPod Photo Reader, clickwheel, gnupod, pygpod, ithmb-rs, OrgZ, andrewmalta/ithmb, wrinklykong/pyithmb, Reuhno, podkit, Steee29, keyj (Jeff Luyten), Mixtape, and more) and cross-referenced them against each other and against existing tables. Found **15 dimension discrepancies** across device profiles, including inverted Nano 5G/6G profiles, wrong Nano 3G formats, and iOS 1.x profile corrections from actual iPhone 2G (iOS 1.1.4) samples. Identified **18 format IDs** not present in any single implementation's table. The consolidated cross-reference covers **54 unique format IDs** — the most complete public reference.

**Device-specific format tables** — All prior tools maintain a flat list of all known format IDs. This project mapped which formats each of **18 iPod/iPhone generations** actually requires for thumbnail display and cover art, enabling per-device profile selection for sync tools.

**BGR15 iPhone channel ordering** — Confirmed via real iPod Classic 6G samples (Reuhno) that iPhone thumbnails use reversed channel order (`xBBBBBGGGGGRRRRR` instead of standard `xRRRRRGGGGGBBBBB`). Added `SwapRgbChannels` flag — the first decoder to distinguish iPhone pixel layout from iPod's.

**Speculative profile corrections** — The F1064 profile (320×240 YCbCr) circulated in community speculation for years. Cross-checked against every public implementation: none has it. Disabled with rationale. Also corrected CLCL nibble scaling from ×17 (original 2005 Whirlpool RE) to ×16, cross-validated against 2 independent implementations.

**32 MB file size limit** — All prior decoders cite libgpod's 256 MB limit, but no evidence confirms it as a real firmware constant. This project derived 32 MB independently: max frame size across 54 profiles is 829 KB, multi-frame concatenation from 5 RE tools never exceeded ~40 frames, and a public .ithmb file survey found zero files >1 MB. 32 MB is a power of 2 and covers all known data.

## PhotoDB / ArtworkDB

**Read-write support** — Every existing tool is read-only (extract thumbnails from a device's Photo Database). This project implements `TryBuildPhotoDb`, capable of writing a valid ArtworkDB binary from scratch — enabling artwork sync to iPod without iTunes.

## Verification Infrastructure

**Hardware validation** — Initiated cross-project collaboration with iOpenPod, whose developer purchased multiple iPod models and validated decoders across firmware generations. This closed a long-standing gap: none of the OSS .ithmb decoders had systematic hardware confirmation.

**Synthetic test vectors (CC0)** — No public F-prefix test data existed before this project. Reuhno contributed generated CC0 vectors covering 3 slot geometries (56×55 slot with varying content rectangles, 128×128, 320×320) with 30 reference PNGs. These are the first public test vectors for raw .ithmb decoding.

**C#→Rust cross-verification** — First time pixel output from an independent Rust implementation was verified byte-for-byte against C# reference decoders across all 7 raw pixel formats, both encode and decode directions. Verified via binary oracle bridge.

**Exhaustive roundtrip tests** — All 7 format encoders pass stable roundtrip (encode→decode→encode produces bit-identical output). 65,536-value exhaustive tests for RGB565, 32,768 for RGB555, 15,625 nibble combinations for CL. All passing.

- **Miri verification** — All SIMD code paths (21 tests) verified for memory safety — zero undefined behavior.

## Negative Knowledge

**iOS Photos ≠ iPod Classic** — Downloaded and analyzed two iOS firmware images (9.3.5 and iOS 18) confirming their .ithmb files use a completely different, proprietary format not decodable by this codec. Documents a common misconception about .ithmb cross-platform compatibility.
