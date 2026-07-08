#ifndef ITHMB_H
#define ITHMB_H

/**
 * @file ithmb.h
 * @brief C API for decoding Apple .ithmb thumbnail files.
 *
 * This is the public C interface for libithmb_core. Link against
 * libithmb_core.{so,dylib,dll} when built with the `c` feature.
 *
 * ## Typical workflow
 *
 * @code
 * // 1. Read prefix from file header
 * uint32_t prefix = (buf[0] << 24) | (buf[1] << 16) | (buf[2] << 8) | buf[3];
 *
 * // 2. Get output dimensions
 * IthmbImage out = {0};
 * ithmb_prefix_to_profile(prefix, &out);
 *
 * // 3. Allocate output buffer
 * out.data = (uint8_t*)malloc(out.width * out.height * 4);
 *
 * // 4. Decode
 * int32_t ret = ithmb_decode(buf, file_len, &out, NULL);
 * // ret == 0 on success; out.data now holds BGRA pixels
 *
 * // 5. Clean up
 * free(out.data);
 * @endcode
 */

#include <stddef.h>   /* size_t */
#include <stdint.h>   /* int32_t, uint8_t, uint32_t */

#ifdef __cplusplus
extern "C" {
#endif

/* ---------------------------------------------------------------------------
 * Types
 * ------------------------------------------------------------------------- */

/** A decoded image descriptor. */
typedef struct {
    /** Pointer to BGRA pixel data (8-bit per channel, 4 bytes per pixel).
     *  Must be allocated by the caller before calling ithmb_decode(). */
    uint8_t* data;
    /** Image width in pixels. */
    uint32_t width;
    /** Image height in pixels. */
    uint32_t height;
} IthmbImage;

/* ---------------------------------------------------------------------------
 * Error codes
 * ------------------------------------------------------------------------- */

/** Operation completed successfully. */
#define ITHMB_OK               0
/** The input data is invalid or corrupt. */
#define ITHMB_ERROR_INVALID   (-1)
/** The format is recognised but not supported by this decoder. */
#define ITHMB_ERROR_UNSUPPORTED (-2)
/** The operation was cancelled by the caller. */
#define ITHMB_ERROR_CANCELED   (-3)

/* ---------------------------------------------------------------------------
 * Functions
 * ------------------------------------------------------------------------- */

/**
 * Look up the output dimensions for a given format prefix.
 *
 * Given a 4-byte big-endian prefix (the first 4 bytes of an .ithmb file),
 * this function fills @p out with the pixel dimensions of the matching
 * profile.  The caller can then allocate @c out->data = malloc(out->width *
 * out->height * 4) and pass the same struct to ithmb_decode().
 *
 * @param prefix  4-byte big-endian format prefix cast to uint32_t.
 * @param out     Pointer to an IthmbImage whose width/height will be set.
 *
 * @retval ITHMB_OK              Success.
 * @retval ITHMB_ERROR_INVALID   @p out is NULL.
 * @retval ITHMB_ERROR_UNSUPPORTED  No profile matches @p prefix.
 */
int32_t ithmb_prefix_to_profile(uint32_t prefix, IthmbImage* out);

/**
 * Decode an .ithmb file from a raw byte buffer.
 *
 * The caller must provide a pre-allocated output buffer in @p out->data.
 * Before calling this function, use ithmb_prefix_to_profile() to determine
 * the required buffer size (width * height * 4 bytes).
 *
 * @param src         Pointer to the raw file bytes.
 * @param len         Length of the input buffer in bytes.
 * @param out         Output image descriptor.  Must have out->data pointing
 *                    to a buffer of at least out->width * out->height * 4
 *                    bytes.  On success the width/height fields are updated
 *                    to the decoded image dimensions.
 * @param cancel_flag Optional cancellation flag.  When the pointed-to
 *                    _Atomic bool becomes non-zero, decoding is cancelled
 *                    at the next macroblock boundary.  Pass NULL if
 *                    cancellation is not needed.
 *
 * @retval ITHMB_OK              Success.
 * @retval ITHMB_ERROR_INVALID   The input is corrupt, too short, or too
 *                               large.
 * @retval ITHMB_ERROR_UNSUPPORTED  The format could not be identified.
 * @retval ITHMB_ERROR_CANCELED  The operation was cancelled via
 *                               @p cancel_flag.
 */
int32_t ithmb_decode(const uint8_t* src,
                     size_t len,
                     IthmbImage* out,
                     const _Atomic _Bool* cancel_flag);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* ITHMB_H */
