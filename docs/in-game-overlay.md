# Overlay interativo dentro do jogo

Pesquisa iniciada em 18/10/2026 e integração revisada em 19/10/2026. A camada Rust de [`tools/game-overlay`](../tools/game-overlay/README.md) tem integração de empacotamento e opt-in por jogo. O painel externo de tela inteira, o polling de processos e o registro global de atalho foram removidos. **O teste mais recente exibiu o painel no Palworld; o usuário identificou problemas de captura, cursor e arraste. A revisão de entrada passou nos testes isolados e foi empacotada; aguarda novo teste gráfico. O histórico e os limites estão abaixo; continua experimental.**

## Inicialização e ciclo de vida

A biblioteca de uma camada Vulkan precisa estar disponível antes de `vkCreateInstance`. Não é possível acrescentá-la a um jogo já aberto apenas exibindo o painel ou escrevendo seu perfil. O primeiro uso e alterações na biblioteca/cadeia exigem reiniciar o **jogo**. Alterações de controles compatíveis com recarga do MAKO não exigem recarregar a biblioteca.

Fluxo pretendido:

1. Salvar o perfil por jogo gera um comando de inicialização com as dependências necessárias.
2. Na Steam, a linha passa pelo fluxo existente de aplicação manual/verificação ou aplicação automática com o cliente fechado. Uma edição manual já salva não exige reiniciar a Steam para ser verificada.
3. Fora da Steam, o adaptador existente encadeia os mesmos componentes ao comando real, sem `%command%` e sem ativação global por sessão.
4. O carregador Vulkan carrega a biblioteca no processo gráfico. O painel começa oculto.
5. O atalho configurado alterna visibilidade e captura de entrada da sessão correta. Esc não fecha o painel.
6. Ocultar pelo mesmo atalho devolve os comandos ao jogo. Fechar o jogo descarrega a camada; ela não mantém um processo do launcher vivo. Não há conexão obrigatória com o launcher. O watchdog libera captura se a apresentação ou o desenho pararem.

Não colocar variáveis em `/etc/environment`, no ambiente global do desktop ou no cliente Steam inteiro. Não mudar opções da Steam real durante pesquisa/testes de contrato.

## Duas integrações distintas

### Desenho

A camada desenha nos quadros da aplicação e acompanha a recriação da swapchain, mudanças de tamanho, minimizar/restaurar e encerramento. Não é Layer Shell e não precisa de uma janela sempre no topo. A independência da ordem de janelas não implica compatibilidade universal com jogos ou compositores.

Rust pode usar `ash` para o Vulkan e `egui` com um renderer compatível para controles. O renderer não resolve sozinho descoberta de camadas, despacho do loader, sincronização, HDR, propriedade das imagens, múltiplas filas ou entrada. Não carregar GTK/WebKit/Tauri dentro do processo do jogo.

### Entrada

Só desenhar o painel não libera o mouse. O caminho deve impedir que as interações do painel também controlem o jogo.

- **X11/XWayland:** verificar o tipo real da superfície do jogo, não apenas `XDG_SESSION_TYPE`. O adaptador é restrito ao loop Xlib do Wine/Proton: `XCheckIfEvent` não bloqueante para teclado/botões e `XGetEventData` para XInput2. A revisão de entrada acompanha também a janela auxiliar de captura do Wine, substitui temporariamente sua restrição pela superfície inteira do jogo e usa o cursor nativo na conexão existente. Suspende mudanças de captura/cursor/posição do jogo enquanto aberto e restaura a última solicitação ao fechar com foco. Eventos raw não geram um segundo clique na UI; botões consultados por polling também são filtrados. Alt+Tab, watchdog e destruição liberam a captura sem a restaurar sobre outras janelas. Loops Xlib/XCB nativos diferentes não foram implementados por suposição.
- **Wayland nativo:** observar os objetos da conexão/superfície do jogo, capturar entrada para o painel e controlar a entrega dos eventos à aplicação. Quando o ponteiro estiver travado, um cursor desenhado pelo overlay pode acompanhar movimento relativo, sem tentar criar uma segunda restrição na mesma superfície.
- `wl_pointer_add_listener` é inline; o fork estudado intercepta `wl_proxy_add_listener`. Clientes podem usar escopos locais do carregador. O projeto usa também um audit shim para o caminho Wine. **Uma Vulkan layer isolada ou apenas `LD_PRELOAD` não prova que a interceptação alcançou os listeners reais do jogo.**
- O encaminhamento de símbolos locais precisa alcançar também as funções originais: redirecionar um import e depois procurar apenas com `RTLD_NEXT` pode falhar. O shim publica os endereços do DSO Wayland/Xlib em uma tabela atômica no namespace base e os invalida no descarregamento. A prova em Python conferiu 14 endereços públicos com `dlsym`/`RTLD_LOCAL`, sem compilar o shim ou conectar ao servidor gráfico.
- Preservar os eventos necessários para manter o estado de foco, keymap e modificadores consistente. Transições precisam evitar teclas/botões presos, comandos duplicados e libertação indevida de objetos emprestados.
- Não chamar callbacks do jogo enquanto se mantém um mutex do overlay. Não capturar eventos de outros aplicativos ou registrar texto digitado.
- Se não houver prova de desenho e bloqueio de entrada na sessão, não anunciar um menu interativo funcional nem manter captura parcial.

## Integração com MAKO

O pacote local `render-v4.5.0`, em `/tmp/peligames-framegen-upstreams/mako/bin/mako-launch`, foi inspecionado sem executar o jogo ou modificar a instalação:

- Define `VK_IMPLICIT_LAYER_PATH` para seus manifests privados e remove `VK_ADD_IMPLICIT_LAYER_PATH`.
- Tem caminhos específicos para vkBasalt e overlay da Steam, organizados também por `VK_INSTANCE_LAYERS`.
- Preserva determinadas camadas explícitas solicitadas pelo chamador. Isso permite estudar integração, mas **não é evidência de compatibilidade** de outra camada.

Portanto, instalar um manifest global ou exportar somente um caminho adicional de camada implícita antes de chamar `mako-launch` não garante seu carregamento.

A posição do overlay em relação ao MAKO é uma decisão de renderização/sincronização: desenhar antes da geração pode incluir o menu nos quadros usados pela geração; desenhar depois exige tratar corretamente todos os presents e imagens que o MAKO produz. Não selecionar a ordem apenas pelo nome dos manifests ou por uma captura onde o menu apareceu.

A documentação atual do upstream declara camadas Vulkan arbitrárias fora do conjunto validado. Não remover a política de isolamento do MAKO, não liberar camadas concorrentes de Frame Generation e não presumir que o procedimento do branch `main` é idêntico ao pacote estável instalado.

Flatpak é outro limite: as bibliotecas precisam estar acessíveis e ser compatíveis com o runtime dentro do sandbox, nas arquiteturas necessárias. O host `mako-launch` e uma `.so` do host não substituem a preparação da extensão.

## Bloqueio de movimento na câmera — revisão de 19/10/2026

Soltar a restrição do cursor e consumir eventos Wayland/X11 não cobre sozinho jogos que consultam Raw Input ou a posição do cursor na camada Windows. O ReShade trata leitura normal e em lote; o OptiScaler também sanitiza os deltas do mouse separadamente da gestão do cursor. Esta revisão aplica o princípio em Rust no limite Unix do Wine, sem incorporar código C/C++ ou criar outro cursor.

`wine_input.rs` intercepta o registro de serviços `KeAddSystemServiceTable` do Wine e entrega uma cópia da tabela win32u com três entradas substituídas: `NtUserGetRawInputBuffer`, `NtUserGetRawInputData` e `NtUserGetCursorPos`. A descoberta usa os endereços ELF publicados pelo audit shim e valida unicidade e tamanho dos argumentos; não depende de números de syscall de uma versão do Proton. A tabela original permanece intacta. Isso é registro de serviços **do Wine no processo do jogo**, sem alterações no kernel ou no desktop.

Com captura ativa, as leituras de Raw Input continuam consumindo a fila normalmente, mas os deltas entregues ao jogo são zerados e o modo absoluto é neutralizado. Tamanhos, retornos de erro, consultas de tamanho, teclado, HID e transições de botões são preservados. A posição consultada pelo jogo fica congelada; o painel continua recebendo o movimento pelo adaptador nativo. Fechar o painel, perder foco ou parar de apresentar quadros libera esse filtro. Tabelas/ABIs não reconhecidas são encaminhadas sem alterações e geram diagnóstico; esse caminho ainda exige validação gráfica com cada Proton.

Validação desta revisão: 55 testes de biblioteca e cinco do runner passaram; um teste Vulkan opcional ficou ignorado. Uma bancada com fixtures ELF escritas em Rust passou pelo audit shim release, carregamento `RTLD_LOCAL` e registro de tabela, executando as funções reais do novo módulo. Conferiu captura ligada/desligada repetidamente, leitura individual/em lote, congelamento/restauração de posição e preservação de erros e consultas de tamanho. Não abriu jogo, sessão Wine ou compositor e não certifica o comportamento gráfico real. Nenhum pacote foi gerado nesta revisão.

Referências consultadas:

- [ReShade — entrada e leitura de Raw Input](https://github.com/crosire/reshade/blob/main/source/input_windows.cpp).
- [OptiScaler — sanitização de entrada bruta](https://github.com/optiscaler/OptiScaler/blob/master/OptiScaler/menu/input/input_system_raw.cpp).
- [Wine — registro da tabela win32u](https://github.com/ValveSoftware/wine/blob/proton_11.0/dlls/win32u/syscall.c) e [leitura de Raw Input](https://github.com/ValveSoftware/wine/blob/proton_11.0/dlls/win32u/rawinput.c).

## Integração e validação pendente

`overlay_enabled` é uma opção por jogo, com default `false` na UI e na desserialização Rust. O salvamento confirma a arquitetura do executável e permite PE32 para testes com o novo WoW64 do Proton. Isso não confirma que o Proton escolhido usa um processo gráfico Linux de 64 bits: essa condição precisa ser verificada no teste real. ELF32 e Flatpak continuam bloqueados, e os payloads ELF64 e o manifest Vulkan `library_arch=64` permanecem inalterados. Não há suporte nativo i686 nesta alteração. Isso não muda o caminho de perfil já usado pelo PeliRun. A opção desativada não encadeia a camada; controles e runner também conferem o opt-in antes de ativar.

O build do launcher incorpora `peligames-overlay-run`, `libpeligames_game_overlay.so` e `libpeligames_overlay_audit.so`. Os crates usam alvos Cargo separados, lockfiles e dependências offline. O instalador materializa somente o conjunto incorporado, em diretório privado com hash de conteúdo; versões antigas permanecem para os wrappers já salvos. O runner gera um manifest explícito privado junto ao conjunto e respeita o limite de descoberta do MAKO. Reconhece a camada MAKO também quando o adaptador MangoHUD já removeu `ENABLE_MAKO`.

O atalho é local ao jogo. Nenhum grab global/portal é registrado; o bootstrap apenas remove a antiga ação KDE do próprio PeliGames caso ela ainda exista. O mesmo parser físico é compartilhado por configurações, runner e camada. Esc não é aceito como tecla de alternância. O menu tem posição inicial à direita, pode ser arrastado e fica limitado à imagem do jogo. A cor translúcida é desenhada sobre o quadro existente, não sobre uma janela WebKit.

São pendências antes de declarar suporte:

- Negociação e encaminhamento de funções do loader Vulkan, inclusive funções desconhecidas e objetos criados pela própria camada.
- Falha de inicialização sem alteração da linha original ou captura parcial de entrada.
- F8 configurável, abertura oculta, ausência de fechamento por Esc e uma única rota de atalho, sem janela externa abrindo simultaneamente.
- Entrada e cursor com jogo capturando o mouse; retorno de foco e ausência de comandos presos após fechar, Alt+Tab, perda de conexão e encerramento.
- XWayland e Wayland nativo em KDE e GNOME, com jogos reais. Identificar cada caminho testado separadamente.
- MAKO ativo: alteração de multiplicador, fixo/adaptativo, pausa, geração efetiva, legibilidade do menu, frame pacing e recuperação de swapchain.
- Coexistência com MangoHUD e overlay Steam; SDR primeiro. Não anunciar HDR ou suporte universal a anti-cheat.
- 64/32 bits e Flatpak verificados separadamente. Um teste de `vkcube` não prova compatibilidade com Proton, MAKO ou jogos reais.

A compilação Rust e a verificação do carregador foram executadas; o teste gráfico em jogo continua pendente. A ordem proposta agora é MAKO → post-processadores gerenciados → overlay → driver, com base no código estável `81d398508592d339da9f4287ea844e695f427630`; a camada recebe cada apresentação real/gerada abaixo do MAKO. A sincronização e a interceptação de entrada dentro dos jogos continuam sendo pendências de validação, não recursos certificados.

O adaptador inicial cobre listeners com ABI C implementados em Rust no Wayland nativo e desativa a captura se encontrar um dispatcher de entrada ou ABI desconhecido. O código para o loop Xlib do Wine/XWayland inclui movimento relativo XI2; ainda não equivale a suporte validado em um Proton real. Flatpak, 32 bits, HiDPI e clientes nativos XCB não foram declarados resolvidos. Nenhuma janela externa é aberta para mascarar a ausência de uma camada funcional.

Verificações executadas após a autorização de compilação em 19/10/2026:

- 27 testes Rust do renderer/runner, incluindo tecla gravada, F8, Esc, foco, posse de entrada, negociação Vulkan e preservação do perfil.
- Bancada com as funções/módulos reais do encadeamento e da detecção de executável: 16 testes aprovados, incluindo sete de MAKO/MangoHUD/overlay e o bloqueio de 32 bits/Flatpak; um teste opcional dependente de instalação real ficou ignorado.
- Runner compilado: opt-in legado/ausente e desativado não pré-carregam a camada; ativado carrega; ausência das bibliotecas preserva o comando original. Argumentos com espaços, aspas e texto de shell continuam literais.
- Bibliotecas release e audit shim carregados em processo separado: os 14 símbolos Wayland/Xlib conferem com o redirecionamento `RTLD_LOCAL`; `dlclose` invalida os originais.
- Instância/dispositivo Vulkan, consulta encaminhada e fila gráfica criados e destruídos com lavapipe, sem conexão ao compositor. Isso não testa desenho, captura real do mouse, frame pacing ou geração de quadros MAKO.
- Biblioteca do renderer construída no Ubuntu 24.04 com requisito máximo GLIBC_2.35. O payload exato foi conferido por conteúdo e hash no binário final e no executável extraído do AppImage. Ambos passaram em `--verify-ui` com 71 assets incorporados. O relatório está em `Release/build-validation.json`.

**Não houve execução de jogo ou teste gráfico.** A certificação de KDE/GNOME, Wine/Proton e coexistência com MAKO permanece separada dos resultados acima.

### Falha observada no teste pela Steam em 19/10/2026

Após o usuário salvar o opt-in e executar Palworld, o journal registrou que `pressure-vessel-wrap` rejeitou `VkLayer_PELIGAMES_overlay.json` por ausência de um campo obrigatório. O campo estava presente, mas `implementation_version` era um número JSON; o leitor do runtime exige uma string. O loader do host havia aceitado esse formato nos testes isolados, que portanto não cobriam esse limite da Steam.

O runner foi corrigido para gerar `implementation_version: "1"` e `file_format_version: "1.2.1"`, versão que introduziu `library_arch`. A comparação com o `steam-runtime-system-info` da instalação local, usando `--no-graphics-tests --no-libraries` e manifests temporários, reproduziu `cannot-load` no formato anterior e reconheceu a camada no formato corrigido. Esse teste só valida a leitura do manifest, sem abrir o jogo ou carregar uma interface gráfica.

Foi acrescentado um teste Rust para os tipos dos campos obrigatórios, incluindo a rejeição da versão numérica. Após nova autorização de compilação, os 28 testes do renderer/runner passaram. O manifest gerado pelo runner release compilado foi reconhecido pelo leitor da Steam, e o pré-carregamento em um processo isolado terminou sem erro.

O binário e o AppImage em `Release/` foram recompilados com essa correção. Os três payloads incorporados foram conferidos por conteúdo em ambos; as duas interfaces passaram em `--verify-ui`. O novo relatório está em `Release/build-validation.json`. O teste gráfico em jogo continua pendente. Ao abrir o novo launcher, salve novamente o perfil com o overlay ativado para atualizar o wrapper para o novo conjunto de bibliotecas; depois reabra o jogo.

### Palworld abre, mas o painel não aparece

O usuário repetiu o teste com o AppImage corrigido, salvou o perfil e deixou o jogo aberto. Foi confirmado o hash do executável em uso, `overlay_enabled=true`, a preferência F8, o encadeamento no wrapper e os dois DSOs no processo gráfico. O erro do manifest não reapareceu. O journal chegou à camada Vulkan, portanto esse segundo problema não era a falta de salvamento do perfil.

A leitura dos indicadores da própria biblioteca no processo confirmou a tabela de 14 funções originais preenchida, entrada Xlib observada, superfície Xlib registrada e uma swapchain de cinco imagens BGRA/SDR em 2560×1440. Porém, essa swapchain estava marcada como inelegível; o renderer e a thread dos controles não tinham iniciado. A versão compilada não preservava todos os parâmetros de criação nem registrava o motivo dessa recusa, portanto não foi possível atribuir um valor exato de flag à execução apenas pelos logs antigos.

Na revisão do filtro, `flags.is_empty()` rejeitava inclusive `MUTABLE_FORMAT`, usado pelo DXVK para views UNORM/sRGB e compatível com a view no formato original usada pelo overlay. O código agora aceita esse bit e os bits de metadados de apresentação ID2/Wait2/timing, preservando os parâmetros originais. Proteção, device group, alocação adiada, renderização multiamostrada e bits desconhecidos continuam exigindo tratamento separado. A cadeia de apresentação aceita também a troca de modo do maintenance1 e os metadados ID2/timing sem reescrever os nós.

Cada criação passa a registrar os parâmetros e o motivo específico de recusa; a preparação do renderer e a disponibilidade da entrada geram mensagens nas transições, sem registrar teclas digitadas. Foram acrescentados três testes de regressão para opções normais do DXVK, preservação dos metadados e recusa das imagens ainda não cobertas.

Após a nova autorização, essa segunda correção foi compilada no Ubuntu 24.04: os 31 testes do renderer/runner passaram, incluindo os três novos. O pré-carregamento em processo isolado e a leitura do manifest pelo runtime da Steam passaram novamente. Os três payloads foram conferidos por conteúdo no binário e no executável extraído do AppImage, e ambos passaram em `--verify-ui`. O relatório em `Release/build-validation.json` identifica os hashes dos pacotes, das bibliotecas e das fontes utilizadas. Os diretórios temporários em RAM foram removidos. **O desenho e a entrada no Palworld ainda precisam ser testados com esses novos pacotes.** É necessário salvar novamente o perfil no launcher novo para o wrapper apontar para a camada atualizada antes de reabrir o jogo.

### Painel aparece, mas cliques chegam ao jogo e há dois cursores

No teste seguinte, o usuário confirmou que o painel apareceu e reconheceu hover, mas relatou cliques sem efeito, ações simultâneas do personagem, dois cursores e ausência de arraste. O journal desse teste registra uma superfície Xlib de 2560×1440; portanto a revisão se concentra no caminho Wine/XWayland, inclusive quando o desktop usa Wayland.

O filtro anterior aceitava botões apenas da janela Vulkan e seus ancestrais. O Wine pode direcionar o ponteiro a uma janela auxiliar de clipping, separada dessa árvore. O renderer também desenhava incondicionalmente uma seta em coordenadas relativas, sem sincronizá-la com o cursor nativo. Por fim, o texto selecionável do título consumia o gesto de arraste.

A revisão adiciona `x11_pointer.rs`: registra os pedidos de captura do jogo, substitui a restrição pela superfície inteira com um cursor do servidor X e usa coordenadas nativas escaladas para a imagem Vulkan. Recentralizações, troca de cursor e novos pedidos de captura do jogo ficam adiados enquanto o painel está aberto. O fechamento pelo atalho restaura o pedido mais recente, com um timestamp válido; perda de foco/watchdog/destruição apenas liberam os recursos. Falha ao obter a captura cancela a abertura, sem remover a captura de outro cliente. A biblioteca não desenha uma segunda seta no caminho Xlib.

Cliques raw e nativos possuem estados separados; apenas os nativos geram eventos de clique do painel. Isso evita duplicação e respeita o mapeamento lógico dos botões. Liberações de botões pressionados antes da abertura continuam para o jogo, e liberações pertencentes ao painel não vazam depois de fechá-lo. O polling de botões e de posição do jogo é filtrado durante a captura. O texto do cabeçalho deixa de ser selecionável. A UI é compartilhada em `panel.rs` entre o renderer e os testes; uma mudança de foco/visibilidade reinicia o estado de ponteiro/arraste do egui, mantendo a posição do painel e sem sintetizar cliques de soltura.

Passaram **44 testes isolados** (39 de renderer/UI/entrada e cinco do runner), incluindo cliques nos multiplicadores e modelo leve, arraste limitado à imagem, perda/retorno de foco, múltiplas conexões Wine, restauração e falha de captura. O audit shim passou na verificação de tipos com os **26 símbolos** compartilhados. Esses resultados usam um backend Xlib simulado e egui sem GPU; não comprovam ainda a entrega de eventos no jogo. A compilação seguinte repetiu `loader_probe` com os 26 símbolos, `RTLD_LOCAL`, invalidação por `dlclose` e criação de instância/dispositivo Vulkan com lavapipe. No Palworld, ainda é necessário testar F8 durante movimentação, menus com cursor, troca de multiplicador, arraste, Alt+Tab e retorno dos controles.

Após autorização do usuário, foram gerados novos binário e AppImage no Ubuntu 24.04 com 12 tarefas. Os 44 testes passaram novamente; os três payloads foram conferidos por conteúdo em ambos os executáveis, e as interfaces passaram em `--verify-ui`. Os hashes e as fontes estão registrados em `Release/build-validation.json`; os diretórios temporários em RAM foram removidos. Salve novamente o perfil no launcher atualizado antes de reabrir o jogo para atualizar o wrapper. **O teste gráfico dessa revisão continua pendente.** O adaptador Wayland nativo continua com seu cursor relativo experimental e exige revisão/validação própria; a correção do cursor nativo aqui é específica de Xlib/XWayland.

### Troca de Proton: Wineland e SDR de 10 bits

Os logs de 19/10/2026, 14:07–14:25, confirmaram que o perfil não foi perdido: GE-Proton 11-7 registrou superfície Xlib BGRA8, renderer e captura do mouse; Wineland 11.0-20260913 carregou a biblioteca, mas deixou a superfície indisponível; CachyOS Wayland reconheceu a superfície e recusou `A2B10G10R10_UNORM_PACK32` com `SRGB_NONLINEAR`. Dez bits por canal não implicam HDR.

A revisão aceita os dois formatos packed RGB10A2 UNORM exclusivamente no contrato SDR já existente, sem modificar as imagens ou a preferência do jogo. Consulta as capacidades de attachment/blend antes de criar o renderer. HDR/PQ, formatos inteiros e formatos ainda não cobertos continuam em passthrough com motivo explícito. Não força oito bits, desativa HDR nem altera variáveis do usuário.

O código oficial do Wineland instalado esclareceu o caminho proxy: Wine registra a superfície Wayland por uma função pública v1 e chama `vkCreateXlibSurfaceKHR` com uma janela X11 auxiliar; a camada do Wineland traduz isso novamente em Wayland. O adaptador Rust novo observa apenas o resultado bem-sucedido dessa função, preserva seus argumentos/retorno e identifica a superfície pelo trio exato instância/display/janela. Falhas não apagam registros aceitos; clear e destruição encerram o registro. A tabela compartilhada com o audit shim agora inclui o símbolo v1, resolvido somente no DSO Wineland x86_64 conhecido. Isso evita registrar/capturar o proxy como uma janela de jogo Xlib, sem editar o Proton ou reordenar suas camadas. Não implementa uma nova captura de cursor Wayland; esse adaptador segue experimental.

**Validação:** 50 testes de regressão e um teste Vulkan executado explicitamente passaram no Ubuntu 24.04/lavapipe. O teste Vulkan criou os recursos e pipelines reais dos seis formatos SDR aceitos. O probe carregou a biblioteca Wineland instalada em `RTLD_LOCAL`, verificou o wrapper e o retorno de erro original com instância nula, descarregou o DSO e conferiu a invalidação dos endereços; 27 símbolos foram verificados. Não foi aberto jogo nem alterado perfil real. Os diretórios temporários em RAM foram removidos. **Após autorização, foram gerados novos binário e AppImage no Ubuntu 24.04 com 12 tarefas.** Os 51 testes e o probe release de 27 símbolos passaram novamente; os três payloads foram conferidos por conteúdo em ambos os executáveis, e as interfaces passaram em `--verify-ui`. Hashes e fontes estão em `Release/build-validation.json`. Os diretórios em RAM foram removidos. Salve novamente o perfil no launcher novo para atualizar o wrapper antes de reabrir o jogo. Falta o teste no Palworld com GE/XWayland, Wineland e CachyOS/Wayland, incluindo foco, F8, cursor, cliques e retorno ao jogo.

Referências do contrato consultado (implementação Rust própria, sem incorporar código C):

- [Proxy Wine na revisão usada pelo Wineland instalado](https://github.com/nanomatters/wine-cachyos/blob/9d3370ac8b985f3b10a46725f22ac238f6dd94fe/dlls/winewayland.drv/vulkan_x11_proxy.c).
- [ABI v1 e tradução da superfície na versão instalada](https://github.com/nanomatters/proton-salkim/blob/wineland-11.0-20260913/vklayers/wineland_vk_translate.c).

### Wayland nativo: foco do teclado na janela pai

O teste seguinte isolou o comportamento usando o mesmo CachyOS Proton: em XWayland o usuário abriu o painel; em Wayland nativo F8 não abriu. O journal de 19/10/2026 às 14:48:21 confirma que o pacote novo aceitou RGB10A2 SDR, preparou o renderer e reconheceu listeners Wayland. Às 14:48:54 a execução Xlib com o mesmo Proton reconheceu entrada e ativou a captura às 14:48:58. Portanto a recusa anterior de dez bits foi corrigida; os logs disponíveis ainda não comprovam qual superfície recebeu o foco nessa execução Wayland.

A inspeção encontrou um erro independente no contrato de foco: o adaptador exigia igualdade entre `wl_keyboard.enter` e a superfície Vulkan, embora Wine possa desenhar numa subsuperfície e manter o teclado na janela pai. A revisão acompanha somente os relacionamentos reais de `wl_subcompositor.get_subsurface`, pelo ABI fixo público `wl_proxy_marshal_array_flags`. A libwayland instalada e o Wine CachyOS consultado usam esse caminho para os requests gerados; não foi introduzido shim variádico, assembly ou código C. Os seis argumentos e o retorno originais são preservados. A leitura dos objetos limita-se aos slots conhecidos do request; requests desconhecidos são encaminhados sem interpretação.

Agora uma superfície Vulkan aceita foco em si mesma ou em ancestral explicitamente observado, nunca em janelas irmãs, outras janelas do processo ou qualquer cliente do desktop. Destruição do papel/subsuperfície remove a associação; destruição de um objeto focado limpa o foco correspondente. `wl_keyboard.leave` compara a superfície realmente focada, inclusive o pai, e não apaga um foco mais recente se receber um leave atrasado. Alt+Tab suspende a captura. Ciclos e árvores excessivas são limitados. Registros de transição indicam reconhecimento/perda do foco, sem registrar teclas digitadas. O helper legado que expunha apenas a superfície Vulkan para essa comparação foi removido.

**Validação:** 55 testes de regressão e um teste Vulkan explícito passaram (56 no total); o probe com a biblioteca Wineland instalada conferiu 28 símbolos, `RTLD_LOCAL`, invalidação no descarregamento e criação de instância/dispositivo Vulkan. Há regressões para janela pai, ancestrais, recusa de irmãos/outras janelas, destruição/reutilização de endereços, preservação do encaminhamento do request, F8 e perda/retorno de foco. São testes isolados, sem uma sessão gráfica de Wine. Os temporários em RAM foram removidos. **Após autorização, a revisão de foco foi empacotada em 19/10/2026 no binário e no AppImage.** Os 56 testes e o probe release com 28 símbolos passaram novamente; os três payloads foram conferidos por conteúdo nos dois executáveis e ambos passaram em `--verify-ui`. Hashes e fontes estão em `Release/build-validation.json`. Os temporários em RAM foram removidos. Falta validar esta revisão no Palworld. Salve novamente o perfil no launcher atualizado antes de reabrir o jogo para atualizar o wrapper. A entrada e o cursor Wayland seguem experimentais.

Referências: [protocolo de subsuperfícies](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_subcompositor), [API cliente Wayland](https://wayland.freedesktop.org/docs/html/apb.html).

## Fontes primárias

- [Khronos: interface do loader com as camadas](https://github.com/KhronosGroup/Vulkan-Loader/blob/main/docs/LoaderLayerInterface.md)
- [Khronos: flags de criação da swapchain](https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainCreateFlagBitsKHR.html) e [troca de modo por apresentação](https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainPresentModeInfoKHR.html).
- [DXVK: criação e apresentação da swapchain em 2.7.1](https://github.com/doitsujin/dxvk/blob/v2.7.1/src/dxvk/dxvk_presenter.cpp) e [implementação atual](https://github.com/doitsujin/dxvk/blob/master/src/dxvk/dxvk_presenter.cpp): mutable format, maintenance1 e metadados de apresentação. Foram usados como referência de contrato, sem copiar código C++.
- [MAKO: isolamento e apresentação](https://github.com/eugeniosegala/MAKO/blob/main/engine/docs/WSI-ISOLATION.md)
- [MAKO: integrações opcionais](https://github.com/eugeniosegala/MAKO/blob/main/engine/docs/LAYER-CHAINING.md)
- [ReShade: desenho e bloqueio de entrada](https://github.com/crosire/reshade/blob/main/source/runtime_gui.cpp)
- [OptiScaler: instalação, atalho e limitações de mouse](https://github.com/optiscaler/OptiScaler/wiki/Manual-Installation)
- [vkDaedalus: arquitetura de entrada](https://github.com/Daaboulex/vkDaedalus#input-architecture). Revisão de código estudada: `f3425ddf383c7b840f7c63c8a00e6ac300f45d19`; arquivos `src/wayland_interpose.cpp`, `src/mouse_input_wayland.cpp`, `src/keyboard_input_wayland.cpp` e `src/input_blocker.cpp`. São referências de um fork experimental; nenhuma fonte C/C++ foi copiada para o aplicativo.
- [Renderer egui/ash](https://github.com/adrien-ben/egui-ash-renderer): desenha os comandos fornecidos, mas não implementa a camada do loader nem a captura da entrada do jogo.
- [Port ReShade Linux, beta 2](https://github.com/TheForgotten69/reshade/releases/tag/v6.8.0-beta.2): registra entrada experimental e situações de entrega simultânea ao jogo.
- [Bridge Steam Wayland](https://github.com/nanomatters/ge-steam-overlay-wayland): distingue desenho da limitação de consumo de entrada no Wine.
- [Wine: fila e cookies Xlib](https://github.com/wine-mirror/wine/blob/master/dlls/winex11.drv/event.c) e [XI2/movimento relativo](https://github.com/wine-mirror/wine/blob/master/dlls/winex11.drv/mouse.c): contrato observado para o adaptador X11 inicial; cookies sem dados são tratados pelo dispatcher. São referências de comportamento, sem copiar o driver C.
- [X.Org: contrato da fila Xlib](https://xorg.freedesktop.org/archive/current/doc/libX11/libX11/libX11.html) e [libXi: ABI XInput2](https://github.com/mirror/libXi/blob/master/include/X11/extensions/XInput2.h). As estruturas Xlib usam o ABI LP64; não têm o mesmo layout dos eventos wire/XCB.

## Diagnóstico real Tomb Raider — ABI do cursor em GE-Proton 11-7

O console da Steam em 19/10/2026, nas execuções das 19:50 e 19:53 (America/Fortaleza), registra o overlay renderizado e a captura nativa ativada, mas também `Wine: tabela de entrada não reconhecida; filtro de câmera não instalado.` O teste anterior com fixtures somente para a variante de dois argumentos não reproduzia a tabela deste Proton.

A inspeção estática do `x86_64-unix/win32u.so` instalado de Proton-GE Latest identificou os três endereços exportados na tabela de serviços real; os tamanhos de argumentos são Buffer=24, Data=40 e Cursor=8 bytes. O código anterior exigia Cursor=16 bytes e recusava a tabela inteira.

Correção: selecionar uma função de encaminhamento de um argumento para Cursor=8 e outra de dois argumentos para Cursor=16, mantendo validação de endereço, unicidade e tamanho dos demais serviços. Não se relaxou o reconhecimento para uma ABI arbitrária. Diagnóstico de falha agora informa os três tamanhos encontrados, sem expor endereços.

Passaram os cinco testes do módulo real e a bancada audit/RTLD_LOCAL em dois processos separados, um para cada variante. Ambos validaram registro, movimento bruto normal/em lote, congelamento de posição, restauração ao fechar e preservação de erros. Nenhum pacote foi gerado após esta correção e o comportamento no jogo ainda precisa de novo teste gráfico.
