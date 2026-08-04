#ifndef GLMAXX_EXL3_STAGED_V2_H
#define GLMAXX_EXL3_STAGED_V2_H

#include "glmaxx_kernel.h"

#define GLMAXX_EXL3_STAGED_KERNEL_ABI \
  "glmaxx.sm120.exl3.warp_staged_projection.v2"

#ifdef __cplusplus
extern "C" {
#endif

// Decode-only performance successor to the retained scalar EXL3 control.
// It consumes the unchanged v1 descriptor and workspace, accepts rows 1..8,
// and never reconstructs a persistent dense weight matrix.
int32_t glmaxx_exl3_staged_projection_launch(
    const glmaxx_exl3_descriptor* descriptor, void* cuda_stream,
    int32_t* asynchronous_error);

const char* glmaxx_exl3_staged_kernel_abi(void);

#ifdef __cplusplus
}
#endif

#endif  // GLMAXX_EXL3_STAGED_V2_H
