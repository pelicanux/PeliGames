# Daniel 0.8 HIP bridge (experimental)

Linux/Wine HIP bridge based on AMDNR-Linux linux-v0.3.6, GPL-3.0-or-later.
Upstream: https://github.com/hassanm0301/AMDNR-Linux/tree/linux-v0.3.6/linux/bridge

Adds the seven HIP texture/pitched-memory exports required by Daniel 0.8 and a
NeuralHub-specific loader-query compatibility layer for Wine. It preserves native
HIP error codes and keeps the PE and Unix dispatch tables generated together.
No proprietary runtime or NVIDIA DLL is included here.

Build with ROCm 7.2 headers, Wine development headers/tools, MinGW and Vulkan headers:

```sh
ROCM_PATH=/opt/rocm WINE_INCLUDE_DIR=/usr/include/wine bash tools/daniel-bridge/build.sh
```

`AMDNR_BUILD_DIR` selects the output directory. Always use the PE DLL and Unix .so
from the same build. Rebuilt artifacts need validation and updated integrity pins
before PeliGames accepts them as a prepared experimental runtime package.

For Proton, the runtime package selects HIP and `HipPredicatedWait=0` in its
NeuralHub settings. This does not disable neural processing or replace its output
with a pass-through. Compatibility remains dependent on game, GPU and Proton.
