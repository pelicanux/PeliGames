# Atribuições do overlay

Partes do overlay adaptam para Rust código destes projetos. As versões alteradas são do PeliGames e não são versões oficiais dos originais.

- **vkBasalt / vkDaedalus** (Georg Lehmann, Boux e Daaboulex): interceptação de entrada Wayland e separação entre desenho e bloqueio de entrada. Licença zlib, aviso abaixo.
- **MangoHUD** (flightlessmango): contagem de FPS por intervalo de apresentações em `present_metrics.rs`. Licença MIT, aviso abaixo.
- **Khronos Vulkan Loader / Vulkan Headers** (The Khronos Group Inc., Valve Corporation, LunarG, Inc.; Apache-2.0 OR MIT): definições ABI da interface de camadas, reescritas em Rust.

As bibliotecas Rust do overlay, como ash e egui, estão nos avisos das dependências Rust.

## Aviso zlib da referência vkBasalt/vkDaedalus

Copyright (c) 2019 - 2022 Georg Lehmann

This software is provided 'as-is', without any express or implied
warranty. In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required.

2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.

3. This notice may not be removed or altered from any source distribution.

## Aviso MIT do MangoHUD

MIT License

Copyright (c) 2020 flightlessmango

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
