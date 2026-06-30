// Licensed under MIT. See LICENSE file.
using System;

// The codec uses Native AOT with unsafe pointer operations, SIMD intrinsics,
// and platform-specific memory allocation — none of which are CLS-compliant.
[assembly: CLSCompliant(false)]
