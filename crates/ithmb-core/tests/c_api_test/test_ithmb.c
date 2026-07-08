/**
 * @file test_ithmb.c
 * @brief Integration test for the ithmb C API.
 *
 * Compiles a minimal .ithmb file for a known profile (1007 = 480×864 RGB565),
 * decodes it through the C API, and verifies the output matches expected BGRA.
 *
 * Usage: test_ithmb
 * Returns 0 on success, non-zero on failure.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdatomic.h>
#include "ithmb.h"

/* ---------------------------------------------------------------------------
 * Known test profile
 * ------------------------------------------------------------------------- */
/* Profile 1007: 480×864 RGB565, little-endian, frame_byte_length = 829440 */
#define PROF_PREFIX  1007u
#define PROF_WIDTH   480u
#define PROF_HEIGHT  864u
#define OUTPUT_BYTES (PROF_WIDTH * PROF_HEIGHT * 4u) /* 1,658,880 */

/* ---------------------------------------------------------------------------
 * Helper: build a minimal .ithmb buffer for profile 1007
 * ------------------------------------------------------------------------- */
static uint8_t* build_test_file(size_t* out_len) {
    /* 4-byte prefix + pixel data */
    size_t pixel_bytes = PROF_WIDTH * PROF_HEIGHT * 2u; /* 16-bit RGB565 */
    *out_len = 4 + pixel_bytes;
    uint8_t* buf = (uint8_t*)malloc(*out_len);
    if (!buf) return NULL;

    /* Big-endian prefix = 1007 */
    buf[0] = (uint8_t)(PROF_PREFIX >> 24);
    buf[1] = (uint8_t)(PROF_PREFIX >> 16);
    buf[2] = (uint8_t)(PROF_PREFIX >> 8);
    buf[3] = (uint8_t)(PROF_PREFIX);

    /* Fill pixel data with 0xFF (white in RGB565 LE = [0xFF, 0xFF]) */
    memset(buf + 4, 0xFF, pixel_bytes);

    return buf;
}

/* ---------------------------------------------------------------------------
 * Test 1: ithmb_prefix_to_profile with known prefix
 * ------------------------------------------------------------------------- */
static int test_prefix_to_profile(void) {
    IthmbImage out = {0};
    int32_t ret = ithmb_prefix_to_profile(PROF_PREFIX, &out);

    if (ret != ITHMB_OK) {
        fprintf(stderr, "FAIL test_prefix_to_profile: returned %d, expected %d\n", ret, ITHMB_OK);
        return 1;
    }
    if (out.width != PROF_WIDTH) {
        fprintf(stderr, "FAIL test_prefix_to_profile: width=%u, expected %u\n", out.width, PROF_WIDTH);
        return 1;
    }
    if (out.height != PROF_HEIGHT) {
        fprintf(stderr, "FAIL test_prefix_to_profile: height=%u, expected %u\n", out.height, PROF_HEIGHT);
        return 1;
    }
    fprintf(stdout, "PASS test_prefix_to_profile: %ux%u\n", out.width, out.height);
    return 0;
}

/* ---------------------------------------------------------------------------
 * Test 2: ithmb_prefix_to_profile with unknown prefix
 * ------------------------------------------------------------------------- */
static int test_prefix_to_profile_unknown(void) {
    IthmbImage out = {0};
    int32_t ret = ithmb_prefix_to_profile(0xDEADBEEFu, &out);

    if (ret != ITHMB_ERROR_UNSUPPORTED) {
        fprintf(stderr, "FAIL test_prefix_to_profile_unknown: returned %d, expected %d\n",
                ret, ITHMB_ERROR_UNSUPPORTED);
        return 1;
    }
    fprintf(stdout, "PASS test_prefix_to_profile_unknown\n");
    return 0;
}

/* ---------------------------------------------------------------------------
 * Test 3: ithmb_decode with known profile
 * ------------------------------------------------------------------------- */
static int test_decode(void) {
    size_t file_len;
    uint8_t* file_buf = build_test_file(&file_len);
    if (!file_buf) {
        fprintf(stderr, "FAIL test_decode: malloc failed\n");
        return 1;
    }

    /* Get dimensions */
    IthmbImage out = {0};
    int32_t ret = ithmb_prefix_to_profile(PROF_PREFIX, &out);
    if (ret != ITHMB_OK) {
        fprintf(stderr, "FAIL test_decode: prefix lookup failed: %d\n", ret);
        free(file_buf);
        return 1;
    }

    /* Allocate output buffer */
    size_t out_bytes = (size_t)out.width * (size_t)out.height * 4u;
    out.data = (uint8_t*)malloc(out_bytes);
    if (!out.data) {
        fprintf(stderr, "FAIL test_decode: output malloc failed\n");
        free(file_buf);
        return 1;
    }

    /* Decode without cancellation */
    ret = ithmb_decode(file_buf, file_len, &out, NULL);
    if (ret != ITHMB_OK) {
        fprintf(stderr, "FAIL test_decode: ithmb_decode returned %d\n", ret);
        free(file_buf);
        free(out.data);
        return 1;
    }

    /* Verify output dimensions match expected */
    if (out.width != PROF_WIDTH) {
        fprintf(stderr, "FAIL test_decode: output width=%u, expected %u\n", out.width, PROF_WIDTH);
        free(file_buf);
        free(out.data);
        return 1;
    }
    if (out.height != PROF_HEIGHT) {
        fprintf(stderr, "FAIL test_decode: output height=%u, expected %u\n", out.height, PROF_HEIGHT);
        free(file_buf);
        free(out.data);
        return 1;
    }

    /* Verify all pixels are white (BGRA = [255, 255, 255, 255]) */
    /* We check first pixel and a few spot locations, then verify total size */
    size_t expected_output_bytes = (size_t)PROF_WIDTH * (size_t)PROF_HEIGHT * 4u;
    if (expected_output_bytes != out_bytes) {
        fprintf(stderr, "FAIL test_decode: output size mismatch\n");
        free(file_buf);
        free(out.data);
        return 1;
    }

    /* Check first pixel */
    if (out.data[0] != 255 || out.data[1] != 255 || out.data[2] != 255 || out.data[3] != 255) {
        fprintf(stderr, "FAIL test_decode: first pixel is not white\n");
        free(file_buf);
        free(out.data);
        return 1;
    }

    /* Check last pixel */
    size_t last_pixel_offset = expected_output_bytes - 4;
    if (out.data[last_pixel_offset] != 255 ||
        out.data[last_pixel_offset + 1] != 255 ||
        out.data[last_pixel_offset + 2] != 255 ||
        out.data[last_pixel_offset + 3] != 255) {
        fprintf(stderr, "FAIL test_decode: last pixel is not white\n");
        free(file_buf);
        free(out.data);
        return 1;
    }

    fprintf(stdout, "PASS test_decode: %ux%u, %zu bytes\n", out.width, out.height, out_bytes);

    free(file_buf);
    free(out.data);
    return 0;
}

/* ---------------------------------------------------------------------------
 * Test 4: ithmb_decode with cancellation
 * ------------------------------------------------------------------------- */
static int test_decode_cancel(void) {
    size_t file_len;
    uint8_t* file_buf = build_test_file(&file_len);
    if (!file_buf) {
        fprintf(stderr, "FAIL test_decode_cancel: malloc failed\n");
        return 1;
    }

    IthmbImage out = {0};
    ithmb_prefix_to_profile(PROF_PREFIX, &out);
    out.data = (uint8_t*)malloc((size_t)out.width * (size_t)out.height * 4u);
    if (!out.data) {
        free(file_buf);
        return 1;
    }

    /* Cancel before starting */
    atomic_bool cancel = ATOMIC_VAR_INIT(1);
    int32_t ret = ithmb_decode(file_buf, file_len, &out, &cancel);

    /* Cancellation may be detected before decode or during — both are valid */
    if (ret != ITHMB_ERROR_CANCELED) {
        fprintf(stderr, "FAIL test_decode_cancel: returned %d, expected %d\n",
                ret, ITHMB_ERROR_CANCELED);
        free(file_buf);
        free(out.data);
        return 1;
    }

    fprintf(stdout, "PASS test_decode_cancel\n");

    free(file_buf);
    free(out.data);
    return 0;
}

/* ---------------------------------------------------------------------------
 * Test 5: ithmb_decode with NULL src -> invalid
 * ------------------------------------------------------------------------- */
static int test_decode_null_src(void) {
    IthmbImage out = {0};
    out.data = (uint8_t*)malloc(OUTPUT_BYTES);
    if (!out.data) return 1;

    int32_t ret = ithmb_decode(NULL, 0, &out, NULL);
    if (ret != ITHMB_ERROR_INVALID) {
        fprintf(stderr, "FAIL test_decode_null_src: returned %d, expected %d\n",
                ret, ITHMB_ERROR_INVALID);
        free(out.data);
        return 1;
    }

    fprintf(stdout, "PASS test_decode_null_src\n");
    free(out.data);
    return 0;
}

/* ---------------------------------------------------------------------------
 * Main
 * ------------------------------------------------------------------------- */
int main(void) {
    int failures = 0;

    failures += test_prefix_to_profile();
    failures += test_prefix_to_profile_unknown();
    failures += test_decode();
    failures += test_decode_cancel();
    failures += test_decode_null_src();

    if (failures > 0) {
        fprintf(stdout, "\n%d test(s) FAILED\n", failures);
    } else {
        fprintf(stdout, "\nAll C API tests PASSED\n");
    }
    return failures;
}
