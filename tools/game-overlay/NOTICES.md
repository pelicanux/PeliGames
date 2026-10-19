# Referências e atribuições

Esta é uma adaptação inicial em Rust do PeliGames. Não é uma versão oficial nem um port completo dos projetos abaixo. Nenhuma fonte C/C++ foi adicionada ao aplicativo.

- **vkBasalt / vkDaedalus**: referência para interceptar listeners Wayland, tratar movimento relativo, preservar metadados e separar desenho de bloqueio de entrada. Revisão estudada: `Daaboulex/vkDaedalus@f3425ddf383c7b840f7c63c8a00e6ac300f45d19`. O upstream reconhece Georg Lehmann, Boux e Daaboulex. A versão Rust alterada está claramente identificada e mantém abaixo o aviso zlib distribuído com a referência.
- **Khronos Vulkan Loader / Vulkan Headers**: contrato público de negociação, despacho e layouts de `vk_layer.h`, conforme [LoaderLayerInterface](https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderLayerInterface.md). Cabeçalho de referência: Copyright 2015–2023 The Khronos Group Inc.; Valve Corporation; LunarG, Inc.; SPDX Apache-2.0 OR MIT. A adaptação contém definições ABI em Rust, sem compilar/copiar o cabeçalho C.
- **MAKO Renderer 4.5.0**: código estável em `81d398508592d339da9f4287ea844e695f427630`, especialmente `engine/scripts/mako-launch` e `engine/mako-render/src/swapchain/present/{generated_frames,real_frames}.cpp`. Referência para respeitar o limite privado das camadas e distinguir quadros reais/gerados. O renderer MAKO não está incorporado neste crate.
- **ReShade / OptiScaler**: referências de comportamento do menu e da captura de entrada. Não são dependências nem foram incorporados como DLLs.
- **Wine / X.Org / libXi**: referências para o contrato de consumo da fila Xlib e layouts públicos XInput2 no adaptador Wine/XWayland. A implementação Rust é própria; não incorpora o código do driver Wine ou a implementação C de Xlib/libXi. Consultados `winex11.drv/event.c`, `winex11.drv/mouse.c`, o manual Xlib e `XInput2.h`; fontes na documentação da arquitetura.
- **Proton Wineland / Wine-Wayland (nanomatters)**: referência de interoperabilidade para a função pública versionada `__wine_wayland_vulkan_register_surface_v1` e o ciclo de registro do proxy Vulkan. Consultados `vklayers/wineland_vk_translate.c` do tag `wineland-11.0-20260913` e `winewayland.drv/vulkan_x11_proxy.c` da revisão `9d3370ac8b985f3b10a46725f22ac238f6dd94fe`. O adaptador Rust é próprio; nenhum código C desses projetos foi incorporado. Links em `docs/in-game-overlay.md`.
- **ash**, **egui** e **egui-ash-renderer**: dependências Rust obtidas pelo Cargo, com versões e checksums no lockfile. `ash`: MIT OR Apache-2.0; `egui`: MIT OR Apache-2.0; `egui-ash-renderer`: MIT. Seus avisos e os das dependências transitivas estão no inventário Rust do launcher; os grafos de dependências dos crates isolados também devem ser incluídos ao regenerá-lo. A camada só é carregada mediante ativação por jogo.

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
