# Overlay PeliGames dentro do jogo

Camada Vulkan em Rust com painel MAKO compacto, translúcido e arrastável. Começa oculto e alterna pela mesma tecla (F8 por padrão), sem Esc, janela de desktop ou barra de título. A interação é interceptada no processo gráfico; não usa GTK, WebKit, Tauri ou atalhos globais.

**Experimental: a revisão de entrada foi empacotada em 19/10/2026 e os logs seguintes confirmaram renderer, atalho e captura nativa em GE-Proton/XWayland.** A troca para Wineland/CachyOS Wayland expôs duas limitações adicionais: proxy de superfície não reconhecido e rejeição de SDR de 10 bits. A correção dessas duas limitações passou em 50 testes de regressão, um teste Vulkan explícito de criação dos pipelines e um probe de 27 símbolos com a biblioteca Wineland instalada; foi incorporada ao binário e ao AppImage em 19/10/2026; falta o teste no jogo. A interface do launcher tem opt-in por jogo; perfis antigos permanecem desativados. Antes de anunciar suporte, executar a matriz gráfica em [in-game-overlay.md](../../docs/in-game-overlay.md).

## Carregamento

O build do launcher incorpora este crate, seu runner e o crate `audit`, em alvos Cargo separados. `PELIGAMES_OVERLAY_BUNDLE` pode apontar a um conjunto previamente compilado da mesma arquitetura. Não são baixados binários de terceiros para esta camada.

Ao salvar a opção por jogo, o conjunto é materializado em `FrameGeneration/overlay/<hash>/`. O wrapper já usado pelo PeliRun encadeia MAKO → adaptador MangoHUD → `peligames-overlay-run -- COMANDO ARGUMENTOS`. O runner preserva os argumentos, o limite privado de descoberta do MAKO, as camadas Steam e os DSOs preexistentes. Acrescenta a camada depois dos post-processadores gerenciados. Não modifica a configuração da Steam nem o ambiente global do desktop.

O runner e os controles exigem `overlay_enabled=true` no JSON associado ao `MAKO_CONFIG`. Erros na preparação do runner fazem o comando original continuar sem os novos `LD_*`/Vulkan. Uma falha posterior em uma biblioteca nativa não pode ser isolada dessa forma: requer validação gráfica e opt-in explícito.

## Desenho e entrada

- `ash`/`egui` e `egui-ash-renderer`: quadros SDR RGBA/BGRA de oito bits e A2B10G10R10/A2R10G10B10 UNORM de dez bits, com suporte do driver a attachment e blend; preserva o quadro existente e sincroniza slots/semaforos.
- Área móvel limitada à imagem do jogo e fundo com alfa. O título não seleciona texto, permitindo arrastar o painel; mudanças de visibilidade/foco cancelam gestos anteriores sem gerar cliques artificiais.
- Wayland nativo: listeners com ABI C implementados em Rust, teclado/ponteiro e movimento relativo; despachantes/ABIs desconhecidos deixam a captura desativada. O cursor relativo desse adaptador continua experimental e não recebe a correção de cursor nativo do XWayland.
- X11/XWayland com Wine: captura temporária na conexão do próprio jogo e cursor nativo livre na superfície inteira, substituindo a restrição da janela auxiliar do Wine. As coordenadas reais alimentam o painel; não se desenha uma segunda seta. Grabs, mudanças de cursor e recentralizações solicitados pelo jogo ficam suspensos até fechar o painel. Os eventos raw são bloqueados separadamente, sem duplicar cliques da interface.
- Wineland: observa o registro público versionado `__wine_wayland_vulkan_register_surface_v1` da camada `libVkLayer_WINELAND_translate_x86_64.so`, inclusive `RTLD_LOCAL`. Uma chamada aceita associa instância/display/janela proxy à superfície Wayland real; falhas preservam o mapeamento anterior e clear/destruição o removem. O proxy nunca recebe uma captura Xlib do overlay. A ordem de camadas e os arquivos do Proton não são alterados. Esta integração ainda usa o adaptador de entrada Wayland experimental.
- `rtld-audit` em Rust `no_std` publica os símbolos originais de bibliotecas `RTLD_LOCAL`; sem alocação, locks ou chamadas gráficas nos callbacks do carregador.
- Foco e releases conhecidos são preservados. Fechar pelo atalho restaura a última captura solicitada pelo jogo; Alt+Tab e watchdog liberam o mouse sem recapturar o desktop. A destruição da janela/conexão também libera os recursos.
- Não exporta entry points Vulkan globais que possam substituir o loader antes de a cadeia existir. A negociação usa o símbolo privado indicado no manifest.
- Atualizações escrevem TOML/sidecar em thread separada com debounce; não mudam a DLL ou o runtime durante a partida.

## Limites e verificação

Linux/glibc x86_64 e Vulkan SDR são o alvo inicial. O launcher permite opt-in em executáveis Windows de 32 bits para testar o novo WoW64 do Proton com processo gráfico Linux de 64 bits; isso ainda não é suporte validado. Executáveis Linux de 32 bits e Flatpak continuam bloqueados. O payload permanece exclusivamente x86_64, com manifest Vulkan `library_arch=64`. OpenGL, HDR, entrada nativa XCB, escalas fracionárias e combinações com anti-cheat não têm suporte declarado. Os caminhos KDE/GNOME Wayland e Wine/Proton precisam de teste real; uma análise de código não os certifica.

Dependências fixadas e textos de licença preparados. Na revisão de entrada passaram 39 testes do renderer/UI/entrada e cinco do runner. O audit shim passou em `cargo check --release --locked --offline`; a tabela compartilhada agora contém 26 símbolos. Os testes cobrem restauração da captura, falha sem remover a captura de outro cliente, conexões Wine distintas, polling de botões, ordem dos eventos raw/nativos, cursor escalado, clique nos controles, arraste e perda de foco. Os testes usam um backend Xlib simulado e o painel egui real sem GPU; **não certificam a entrega real de eventos no Palworld, KDE ou GNOME**.

As validações anteriores de distribuição cobriram 16 testes da bancada de encadeamento/compatibilidade (um teste opcional de instalação real ignorado), os modos do runner, 14 símbolos `RTLD_LOCAL` e Vulkan com lavapipe. A compilação de 19/10/2026 repetiu os 44 testes e o probe com os 26 símbolos, `RTLD_LOCAL`, invalidação por `dlclose` e criação de instância/dispositivo Vulkan com lavapipe. Os três payloads foram conferidos por conteúdo no binário e no executável extraído do AppImage; ambos passaram em `--verify-ui`. O relatório está em `Release/build-validation.json`. A matriz gráfica continua pendente. Salve novamente o perfil no launcher atualizado antes de reabrir o jogo para atualizar o wrapper.

Para repetir os testes: `cargo test --locked --offline --manifest-path tools/game-overlay/Cargo.toml`. O exemplo `loader_probe` verifica o carregador sem abrir uma janela, executado pelo runner com perfil temporário e lavapipe. O próximo passo de validação é o teste gráfico com MAKO/Proton. Validar recriação da swapchain, foco/mouse, F8/Esc e recuperação no KDE e GNOME separadamente.

Referências e atribuições: [NOTICES.md](NOTICES.md), também incluídas no diretório de licenças dos pacotes.

### Validação da revisão Wineland/SDR de 10 bits

Em 19/10/2026 passaram 50 testes isolados (45 do renderer/entrada e cinco do runner) e o teste Vulkan opcional executado explicitamente com lavapipe: criação real de render pass, views, framebuffers e pipeline de blend nos seis formatos SDR. O `loader_probe`, com `PELI_PROBE_WINELAND_LIBRARY` apontando à biblioteca de tradução instalada, validou 27 símbolos, redirecionamento `RTLD_LOCAL`, preservação da falha da função Wineland real, invalidação por `dlclose` e criação de instância/dispositivo Vulkan. Não abriu janela nem jogo. Os alvos temporários em RAM foram removidos. **Após autorização, os novos binário e AppImage foram gerados no Ubuntu 24.04 em 19/10/2026.** Os 51 testes e o probe release de 27 símbolos passaram novamente. Os três payloads foram conferidos por conteúdo nos dois executáveis, que passaram em `--verify-ui`; o relatório está em `Release/build-validation.json`. Os alvos em RAM foram removidos. Salve novamente o perfil no launcher novo para atualizar o wrapper antes de reabrir o jogo. A captura/cursor Wayland, foco e troca entre Protons ainda precisam do teste em jogo.

### Revisão de foco Wayland nativo

O teste no mesmo CachyOS Proton confirmou overlay em XWayland e falha de abertura em Wayland; os logs novos já mostram renderer preparado com SDR de dez bits. O código exigia foco na superfície Vulkan exata, ignorando a janela pai de uma subsuperfície. A nova revisão observa o protocolo público de criação/destruição de subsuperfícies, aceita foco apenas na própria superfície ou em ancestrais comprovados e trata leave/Alt+Tab, destruição e reutilização de objetos. Não presume foco por pertencer ao mesmo processo. Não cria conexão ou captura global, não modifica o Proton e não usa C ou assembly.

Passaram 55 testes de regressão, um teste Vulkan explícito (56 no total) e o probe de carregamento com 28 símbolos. Os testes de encaminhamento e foco são isolados; ainda falta teste real no Palworld. **Esta revisão de foco foi incorporada ao binário e ao AppImage em `Release/` após autorização em 19/10/2026.** Os 56 testes e o probe release com 28 símbolos passaram novamente; os payloads foram conferidos nos dois executáveis e as interfaces passaram em `--verify-ui`. O relatório está em `Release/build-validation.json`; os temporários em RAM foram removidos. Salve novamente o perfil no launcher atualizado antes de reabrir o jogo para atualizar o wrapper. Os limites do cursor Wayland experimental permanecem. Detalhes e evidências em `docs/in-game-overlay.md`.

### Filtro de movimento Wine/Proton

A revisão `wine_input.rs` acrescenta filtragem de Raw Input (individual e em lote) e congelamento da posição consultada pelo jogo durante a captura, além das rotas nativas Wayland/XWayland. O registro da tabela win32u é reconhecido por endereços e ABI, sem números fixos de syscall; tabelas desconhecidas são preservadas. O diagnóstico registra se o filtro foi instalado. Ainda requer teste gráfico real.

Bancada isolada do carregador e do filtro, com fixtures inteiramente em Rust e sem abrir Wine ou jogos:

```sh
bash tools/game-overlay/tests/wine_input/run.sh
```

Detalhes e fontes: `docs/in-game-overlay.md`.
