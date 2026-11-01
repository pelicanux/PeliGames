/* SPDX-License-Identifier: GPL-3.0-or-later */
#pragma once
#define __HIP_PLATFORM_AMD__
#include <hip/hip_runtime_api.h>
#include <stdint.h>
#include <stddef.h>
/* PE and Unix halves share only fixed x86-64 HIP ABI structures and pointers. */
_Static_assert(sizeof(void *) == 8, "The bridge requires x86-64");
_Static_assert(sizeof(hipDeviceProp_t) == 1472, "HIP R0600 ABI changed");
_Static_assert(offsetof(hipDeviceProp_t, luid) == 272, "HIP LUID ABI changed");
_Static_assert(offsetof(hipDeviceProp_t, gcnArchName) == 1160, "HIP architecture ABI changed");
_Static_assert(sizeof(hipExternalMemoryHandleDesc) == 104, "HIP memory ABI changed");
_Static_assert(sizeof(hipExternalSemaphoreSignalParams) == 144, "HIP fence ABI changed");
_Static_assert(sizeof(hipExternalSemaphoreWaitParams) == 144, "HIP fence ABI changed");
_Static_assert(sizeof(dim3) == 12, "HIP launch ABI changed");

/* hipcc's module-registration ABI (ROCm clr hip_platform.cpp). Compilers emit
 * these calls; public headers do not declare them. Pointer arguments are opaque
 * keys or names, identical for Windows- and Linux-compiled modules. */
void **__hipRegisterFatBinary(const void *wrapper);
void __hipRegisterFunction(void **modules, const void *host_function, char *device_function,
                           const char *device_name, unsigned thread_limit, void *tid, void *bid,
                           dim3 *block, dim3 *grid, int *warp_size);
void __hipRegisterVar(void **modules, void *variable, char *host_name, char *device_name,
                      int external, size_t size, int constant, int global);
void __hipUnregisterFatBinary(void **modules);

/* HIP 7.2 x86-64 texture descriptors cross the PE/Unix boundary unchanged. */
_Static_assert(sizeof(hipTextureObject_t) == 8, "HIP texture handle ABI changed");
_Static_assert(sizeof(hipResourceDesc) == 64, "HIP resource descriptor ABI changed");
_Static_assert(offsetof(hipResourceDesc, res.pitch2D.width) == 40, "HIP pitch width ABI changed");
_Static_assert(offsetof(hipResourceDesc, res.pitch2D.pitchInBytes) == 56, "HIP pitch stride ABI changed");
_Static_assert(sizeof(hipTextureDesc) == 64, "HIP texture descriptor ABI changed");
_Static_assert(offsetof(hipTextureDesc, normalizedCoords) == 40, "HIP texture flags ABI changed");
_Static_assert(sizeof(struct hipResourceViewDesc) == 48, "HIP resource view ABI changed");
_Static_assert(sizeof(unsigned short) == 2, "HIP D16 value ABI changed");
