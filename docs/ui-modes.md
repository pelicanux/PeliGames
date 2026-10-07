# Modos da interface na barra da biblioteca

Os botões **Gerenciador de mod** e **Instalar jogo** ficam sempre visíveis à esquerda de **Pasta**, com ícones e indicação do modo ativo. O modo de instalação abre também sem uma capa selecionada. Ele mostra uma capa genérica, nome confirmável por Enter ou pelo botão Confirmar, edição pelo lápis, diretório editável e botão Instalar, informações simplificadas (diretório, lançamento e plataforma) e o painel Configurações de instalação. Iniciar não aparece enquanto a instalação ainda é um rascunho. O botão Instalar já executa um instalador Windows no prefixo do destino usando o Proton selecionado. Ao término sem erro, os executáveis finais encontrados são registrados na categoria PeliGames com o nome e a capa do formulário. Selecionar uma dessas entradas habilita Iniciar com seu prefixo e Proton persistidos.

## Prévia interativa

Execute `bun run dev:ui` na raiz do projeto e acesse `http://127.0.0.1:1424/preview.html`. Clique diretamente em **Instalar jogo** para abrir o formulário, ou na capa de **Jogo com mod** para conferir o gerenciador de mods. A biblioteca inicial e operações legadas de mods usam dados demonstrativos. O gerenciamento de Proton e o botão Instalar jogo executam operações reais por meio do serviço Rust local. A busca de capas na prévia consulta catálogos reais da Steam, Epic e GOG pelo servidor local. Para testar, confirme um nome real, como **Portal 2**. A biblioteca inicial continua demonstrativa. No aplicativo desktop, a busca usa o comando nativo e pode recorrer também ao SteamGridDB configurado. O servidor padrão do aplicativo permanece na porta 1420.

## Componentes

- `src/components/GameModeSelector.tsx`: dois botões temáticos com ícones, descrições e estado ativo.
- `src/components/GameInfoPanel.tsx`: estrutura compartilhada das informações nos dois modos.
- `src/components/InstallGameView.tsx`: resumo, edição/confirmacão do título, balão com o nome completo e painéis de instalação.
- `src/hooks/useInstallationGameDraft.ts`: nome confirmado, rascunho de edição e estado da capa; respostas de buscas antigas são descartadas.
- `src/services/customCovers.ts`: busca por nome compartilhada com a restauração de capas.
- `src/components/GamePanelCarousel.tsx`: transição dos painéis, preservando o conteúdo de mods e respeitando o modo de desempenho.
- `src/components/GameGrid.tsx`: integração da barra e busca compacta que abre um campo ao clicar na lupa.
- `src/App.css`: adaptação da busca ao espaço disponível na barra e ações do jogo lado a lado na janela reduzida.

## Verificação

`bun run build` passou. Interface conferida no navegador com dados demonstrativos e chamadas nativas simuladas, sem executar instalações:

- 1250 × 900: busca completa e dois modos à esquerda de Pasta; recolher detalhes logo abaixo da capa.
- 800 × 850: busca reduzida a uma lupa e todos os botões na mesma linha.
- Busca pela lupa: foco automático, filtro por nome, limpeza e fechamento com Escape.
- Abertura de Instalar jogo sem selecionar uma capa, edição de nome e diretório e ausência de ações de iniciar/reparar/desinstalar no rascunho.
- Retorno ao conteúdo original de mods, com arquitetura, API gráfica, status do mod e upscalers preservados.

Capturas do formulário: `images/game-installation-desktop.png` e `images/game-installation-compact.png`.

## Confirmação do nome e capas

Balões dos modos e do nome aparecem apenas ao passar o mouse, sem permanecer abertos pelo foco do botão. A confirmação troca o campo pelo título destacado com lápis, inicia a busca de capa e preserva o rascunho durante a troca de modos. Sem resultado ou em caso de falha, o ícone genérico permanece com uma mensagem na interface.

Verificados no navegador: nome vazio bloqueado, confirmação por Enter e pelo botão, edição pelo lápis, balão com nome longo, atribuição de capa e fallback sem resultado. A busca do aplicativo usa o comando nativo existente. A prévia usa uma rota de desenvolvimento, sem credenciais, para consultar catálogos públicos.

Captura: `images/game-installation-confirmed-name.png`.

## Capas reais na prévia

`scripts/preview-covers.ts` fornece a rota de desenvolvimento `/__preview/cover` para contornar as restrições de origem do navegador. A rota consulta Steam, Epic e GOG, compara os títulos e devolve a URL da capa vertical. Ela não é incluída na compilação de produção e não usa chaves pessoais. Conferida a busca de **Portal 2** e a exibição de sua capa real no formulário.

Captura: `images/game-installation-real-cover.png`.

## Organização atual dos formulários

Configurações ficam à esquerda e informações do jogo à direita nos dois modos, preservando as larguras de cada painel. O formulário de instalação contém nome do jogo/programa, local de instalação, tipo de instalador (Proton) e executável. Nome confirmado e capa aparecem no resumo ao lado, com os mesmos efeitos de inclinação e brilho da capa de mods.

Recolher detalhes descarta nome, capa e os caminhos e Proton selecionados. Reabrir Instalar jogo começa um formulário novo com o destino base `~/Games/PeliGames`. Essa pasta é criada ao abrir o modo, tanto pelo comando Rust no aplicativo quanto pela rota local da prévia. Confirmar o título sugere uma subpasta com o nome do jogo/programa, sem criá-la ainda. A sugestão acompanha alterações do título confirmado; um caminho escolhido manualmente permanece intacto. Caracteres de caminho são normalizados apenas no nome da subpasta. Ao clicar em Instalar, o backend cria o prefixo em `<destino>/prefix` e executa o arquivo escolhido. Durante a execução, recolher detalhes fica bloqueado para preservar o formulário. Os botões de modos ficam neutros enquanto só a biblioteca está visível. Gerenciador de mod abre uma janela de seleção; escolher DLSSNR-AMD abre o wizard de configuração do mod somente quando ainda não há configuração salva. Concluir a configuração mostra o grid completo; clicar numa capa abre os painéis e ações do mod.

A varredura somente de leitura reconhece pastas com o arquivo `proton`, deduplica caminhos reais e inclui ferramentas de compatibilidade Steam, bibliotecas Steam externas, Heroic, Lutris e pastas comuns do sistema. O seletor de instalação lista apenas versões locais por caminho. O gerenciamento fica em **Preferências → Gerenciar Proton**, abaixo de Projeto Backend, com cartões GE-Proton e Proton CachyOS e botão Atualizar Proton no canto superior direito. A família escolhida abre no atualizador embutido em Preferências, com botão Voltar. Ele consulta os releases oficiais do GitHub, mostra as versões, baixa com progresso/MB/s, valida e extrai o pacote em `~/.config/peligames/runners/proton`. A conclusão refaz a varredura do seletor de instalação sem trocar a versão já selecionada no formulário. A prévia usa o mesmo núcleo Rust por `tools/proton-service`, que deve ser compilado com `cargo build --manifest-path tools/proton-service/Cargo.toml`.

Os seletores de caminho usam os diálogos Tauri no aplicativo. Na prévia Linux, o servidor local abre Zenity ou KDialog. A prévia verifica a origem da solicitação antes de abrir um diálogo. A execução de instaladores usa `run_game_installer` no aplicativo e `/__preview/game-installer` na prévia, compartilhando o núcleo Rust.

Verificação: compilação da interface, teste Rust de descoberta/deduplicação no Distrobox Ubuntu, detecção dos Protons reais no servidor de prévia, seleção no menu, capa real de Portal 2 com efeito de mouse, ordem dos painéis e limpeza do formulário ao recolher.

Captura: `images/game-installation-config-left.png`.

Conferido o destino padrão no navegador: confirmar Portal 2 sugere `~/Games/PeliGames/Portal 2`, um destino manual permanece após editar o nome e recolher/reabrir limpa o formulário, voltando à pasta base. Confirmada a criação de `~/Games/PeliGames` no disco. `bun run build` e `cargo check` no Distrobox Ubuntu passaram. Captura: `images/default-installation-directory.png`.

Os quatro botões da barra (Gerenciador de mod, Instalar jogo, Pasta e Escanear) compartilham `HoverTooltip`, com o visual original. O balão é renderizado fora da barra, escolhe acima ou abaixo do botão e limita sua posição à janela, evitando corte quando só o grid está aberto. Aparece somente com o mouse, desaparece ao sair ou clicar e não fica preso ao foco. A edição do nome usa “Cancelar”, com capitalização normal.

Verificação: `bun run build` e prévia a 800 × 650, com os quatro balões na tela inicial, edição/Cancelamento do nome e retorno ao título confirmado. Captura: `images/toolbar-hover-grid.png`.

## Categoria das instalações próprias

Após o instalador encerrar sem erro, o núcleo Rust examina o destino e seu prefixo, filtra instaladores/desinstaladores e arquivos Windows e registra os executáveis na categoria PeliGames. As entradas usam a capa do formulário e mostram o nome do arquivo sobre a capa para distinguir múltiplos executáveis. Manifestos no destino e um índice na configuração restauram a categoria após reabrir. Escanear atualiza somente os destinos registrados para essa categoria, além da varredura habitual de Steam/Heroic/pastas manuais. Iniciar usa o prefixo e runner persistidos; o diretório real é usado pelas funções de mods e Abrir pasta.

Conferidos sete testes Rust, compilação da interface, cargo check no Ubuntu e prévia isolada com executáveis Wine controlados: registro após execução, capa, recarga, iniciar, varredura de arquivo novo e exclusão de arquivo apagado. Captura: `images/peligames-installed-category.png`. Nenhum jogo de terceiros foi instalado na verificação.


O campo **Local de instalação** oferece os botões **Padrão** e **Escolher**, lado a lado. Padrão retorna a `~/Games/PeliGames/<nome confirmado>` e cria essa pasta; Escolher abre o seletor do sistema e só altera o destino quando a escolha é confirmada. Após selecionar uma opção com sucesso, os dois botões desaparecem e são substituídos pelo campo de caminho no mesmo local, mantendo edição manual, balão com o caminho completo e ícone de pasta para alterar o local. Cancelar a escolha inicial ou falhar na seleção mantém os botões visíveis. Cancelar o seletor preserva o destino anterior. Recolher detalhes também reinicia essa escolha.


A biblioteca abre no escopo **PeliGames**, mostrando apenas jogos e programas registrados pelo próprio launcher. **Gerenciador de mod** abre a seleção de mod; escolher **DLSSNR-AMD** abre o wizard apenas se ainda não houver configuração salva do mod; com configuração existente, abre diretamente a biblioteca completa (Steam, Heroic, outras fontes e PeliGames); **Instalar jogo** retorna ao escopo próprio. O escopo do grid é independente do painel superior: selecionar uma capa própria abre seus detalhes sem revelar as outras bibliotecas. Recolher detalhes mantém o escopo atual, enquanto reabrir o aplicativo começa no escopo PeliGames. A varredura inicial e Escanear no escopo próprio atualizam somente destinos registrados do PeliGames; a varredura completa continua disponível no gerenciador de mods. O cache conserva todas as fontes, sem duplicar ou apagar entradas ao alternar o filtro. A barra de controles permanece visível com biblioteca vazia.


O carrossel possui um terceiro modo, **installed**, exclusivo para entradas registradas pelo PeliGames. Selecionar uma dessas capas no escopo próprio abre esse modo. No grid completo acessado pela seleção de mod, as capas próprias também abrem o modo mods, com seus painéis e ações existentes. O painel esquerdo preserva moldura, vidro e iluminação padrão e reúne as Configurações da entrada instalada. O painel direito reutiliza Informações do Jogo no formato do instalador: diretório de instalação (pasta que contém o prefixo), lançamento e plataforma Windows (Proton / Wine), sem arquitetura, API gráfica, upscalers nem estado do mod. Capa, animações, título, Iniciar, Abrir pasta e Recolher detalhes são preservados. A seleção no escopo próprio não consulta análise de mods, instalação do mod ou configuração de inicialização neural e não exibe instalar/reparar/desinstalar mods. Jogos externos continuam no modo mods.

Verificação: build da interface aprovado e prévia conferida selecionando um programa já registrado no PeliGames, sem executá-lo. Confirmados painel esquerdo vazio, informações reduzidas à direita, ações próprias e seleção de um jogo Steam retornando aos controles de mods. Captura: `docs/images/installed-game-carousel.png`.


**Abrir pasta do jogo** funciona também na prévia do navegador: a rota local de desenvolvimento valida uma pasta absoluta existente e chama `xdg-open` com argumento separado, sem shell, removendo `LD_LIBRARY_PATH` herdado. O comando nativo Tauri permanece disponível no aplicativo desktop. O botão usa o diretório do executável registrado e apresenta erro em uma janela da interface quando a abertura falha. Conferidos clique em uma entrada PeliGames sem erro, rejeição de caminho inexistente e build da interface.


O botão **Iniciar** das entradas PeliGames usa o supervisor Rust `core/game_execution.rs`, compartilhado pelo aplicativo e pela prévia. Estados: `starting` (Iniciando…), `running` (Cancelar com X), `stopping` (Encerrando…), `exited`, `cancelled` e `failed`. A execução usa o prefixo e o Proton registrados. O estado running depende de detectar o processo do executável ativo por pelo menos 500 ms; essa detecção acompanha o processo e não representa um teste da resposta gráfica da janela. Durante a preparação, o botão já permite cancelar quando o supervisor retorna a sessão. O término normal ou o cancelamento devolve Iniciar; falhas apresentam mensagem e caminho do log.

O cancelamento aplica SIGKILL ao grupo criado para essa execução e aos seus descendentes identificados, incluindo processos separados que preservam o WINEPREFIX. A identificação inclui PID e instante de criação para evitar confundir PIDs reutilizados. A varredura e o encerramento continuam até não restar processo ativo dessa execução. Um prefixo que já esteja em uso é recusado antes de iniciar uma nova execução, evitando compartilhar e encerrar processos anteriores. Não há encerramento global de Wine ou Steam. Identificadores de sessão recusam cancelamentos antigos. O supervisor permanece ativo ao recolher detalhes ou trocar de seleção; a prévia recupera o estado após recarregar a página. Instalações e novas execuções simultâneas são bloqueadas.

Adaptadores: Tauri `start_peligames_game`, `get_peligames_execution` e `cancel_peligames_game`; prévia `/__preview/game-execution`, com Rust `monitor-game`, eventos JSON e cancelamento por stdin. A prévia preserva a execução ao fechar a aba. A inicialização de jogos externos pela Steam continua sendo delegada ao cliente Steam.

Verificação: onze testes Rust passaram, incluindo detecção do executável, término normal, código de falha, recusa de sessão antiga/duplicada, encerramento de filho separado por setsid e preservação de um processo sem relação com a execução. Build da interface e cargo check no Ubuntu aprovados. Na prévia isolada, Winecfg oficial do Proton Experimental já instalado foi executado em um prefixo de /tmp: confirmado Iniciando… → Cancelar → Encerrando… → Iniciar, com zero processos ativos restantes no prefixo após cancelar. Registros e prefixo temporários removidos. Captura: `docs/images/game-execution-cancel.png`.


O painel esquerdo do modo **installed** agora se chama **Configurações**. Oferece nome do jogo/programa, seletor das versões locais de Proton, caminho do executável com edição manual/seletor do sistema. Não há botão global de salvar: selecionar o Proton salva imediatamente; o nome é salvo com **Enter** ou **Confirmar**, abaixo do campo. A escolha de um executável pelo seletor também salva imediatamente; a edição manual do caminho é confirmada com **Enter** ou **Confirmar**. Cada confirmação altera apenas seu próprio campo e preserva rascunhos nos demais campos. Cancelar o seletor não altera a configuração. As alterações afetam apenas a entrada selecionada, mantendo prefixo, pasta de instalação e capas, inclusive capas personalizadas. O título exibido muda, mas a pasta existente não é renomeada. O backend valida arquivo Windows e runner antes de gravar; configurações por entrada ficam em `overrides` no manifesto e sobrevivem a novas varreduras. A identidade da entrada permanece estável mesmo quando o executável principal muda, evitando perder a associação da capa e permitindo restaurar a entrada caso o arquivo original seja removido.

**Executar programa** abre o seletor de .exe/.msi e, após confirmar um arquivo, inicia-o com o prefixo e o Proton salvos da entrada selecionada. Cancelar o seletor preserva o estado sem executar nada. Rascunhos ainda não confirmados não interferem na execução, que usa as configurações já salvas. A execução auxiliar usa o mesmo supervisor: Iniciando…, Cancelar e encerramento da árvore de processos; o manifesto e o executável principal permanecem intactos. Durante uma execução ou gravação, os campos ficam bloqueados. Adaptadores: `update_peligames_settings` no Tauri, CLI `update-game`, rota local `/__preview/peligames-settings` e executável opcional no comando/rota de monitoramento.

Verificação desta etapa: doze testes Rust aprovados, incluindo preservação das edições após rescan, prefixo/capa, configurações de outra entrada e rejeição de runner inválido sem alterar o manifesto. Build da interface e cargo check no Ubuntu aprovados. Na prévia isolada, confirmados edição de nome/executável/Proton, atualização do título, Escanear e recarga preservando os valores. Com resposta do seletor simulada somente no ambiente de teste, Executar programa abriu o Winecfg oficial no prefixo temporário existente e com o runner salvo; manifesto principal permaneceu idêntico. Cancelar encerrou os processos, e cancelar o seletor não criou uma nova execução. Prefixo, registros e simulação temporários foram removidos. Captura: `docs/images/installed-game-settings.png`.


Confirmação por campo no painel instalado: build da interface aprovado; prévia isolada verificou Proton salvo imediatamente sem confirmar/apagar o rascunho do nome, confirmação do nome por Enter e pelo botão Confirmar, e persistência de ambos após recarregar. Não há botão global Salvar alterações. Captura: `docs/images/installed-game-autosave.png`.


Seleção de mods: `ModManagerModal` reutiliza `ModalSurface`, com titlebar fixa em pílula, blur, cantos neon e fechamento pelo X, Escape ou clique no fundo. Exibe “Selecione o mod a ser usado.” e um cartão DLSSNR-AMD; catálogo e grade preparados para futuras integrações, sem cartão Nexus por enquanto. Cancelar preserva a seleção anterior; escolher o mod sem configuração salva abre o wizard existente, preservando GPU, backend, download, modelo de IA, idioma e ferramentas de emergência. Retornar reabre a seleção de mods; concluir a configuração recolhe os detalhes, limpa a seleção e mostra a biblioteca completa. A capa escolhida depois abre Configuração do Mod, Informações do Jogo e as ações existentes. Verificados na prévia seleção → grid → capa → painéis e fechamento preservando o jogo; build aprovado. Captura: `docs/images/mod-manager-selection.png`.


Rolagem da biblioteca: `.game-grid-container` usa `overscroll-behavior: auto`. A roda/trackpad rola o grid e, ao chegar a seus limites ou quando não há rolagem interna disponível, continua no contêiner principal. O isolamento `contain` anterior bloqueava essa continuação, causando a impressão de scroll inativo sobre as capas. Verificado no navegador: limite inferior do grid em 47 px permitiu rolar a janela de 0 a 55 px; rolagem para cima retornou ambos a 0. Build aprovado; mesma folha de estilo utilizada na prévia e no aplicativo.


Boas-vindas do launcher separadas da configuração de mods: na primeira entrada, `WelcomeModal` contém somente “Seja bem-vindo” e “Começar”. Começar registra `peligames_welcome_completed` no armazenamento local e abre o grid próprio, sem exigir configuração de GPU/mod nem criar uma configuração fictícia. Ausência de `load_app_config` não abre mais o wizard de mod automaticamente. Com biblioteca própria vazia, o aviso inclui um botão Instalar jogo. `SetupWizard` aparece pelo cartão DLSSNR-AMD apenas sem configuração concluída, e continua acessível pelos pontos de reconfiguração/recuperação existentes, com botão Retornar para a seleção de mods. Retornar e fechar ficam bloqueados durante download, extração ou gravação. Verificados na prévia boas-vindas → biblioteca própria, biblioteca vazia com botão de instalação e seleção → wizard completo → Retornar; build aprovado. Detecção de GPU e configuração nativa não foram executadas nesse teste de navegação. Capturas: `docs/images/launcher-welcome.png` e `docs/images/mod-setup-return.png`.


Wizard do mod somente antes do primeiro uso: o cartão DLSSNR-AMD aguarda carregar `load_app_config`. Uma configuração salva com backend AMDNR/OptiScaler e modelo .bin/.dll abre diretamente o grid completo de mods, incluindo depois de reiniciar. Ausência de configuração ou preferências padrão sem modelo abre o wizard. Concluir e salvar já atualiza esse estado na sessão atual; Retornar/cancelar não marca configuração concluída. Reconfiguração explícita e recuperação continuam disponíveis. Não é criada uma marca separada no navegador para substituir a configuração persistida do mod. Build aprovado e verificadas as decisões para configuração ausente, preferências sem modelo, configuração concluída e recarregada.


Instalar ou adicionar: o botão Instalar jogo abre `GameEntryModal`, com titlebar em pílula “Instalar jogo / adicionar jogo” e cartões Instalar jogo e Adicionar jogo. A primeira opção abre o formulário existente; a segunda abre o modo `add` no mesmo carrossel, reutilizando nome, capa, local de instalação, Proton, executável, vidro e animações. Adicionar registra explicitamente o executável sem executá-lo nem mover seus arquivos, cria/reutiliza `<destino>/prefix` e salva nome, runner e capa. O prefixo é inicializado pelo Proton/UMU no primeiro Iniciar. Destino já registrado e executável duplicado são recusados. A varredura das adições considera apenas o executável explicitamente registrado e suas edições. Após adicionar, a entrada aparece na biblioteca própria e abre os detalhes registrados com Iniciar; instalações que encontram executáveis também selecionam uma entrada ao terminar.

Desinstalar aparece abaixo da ação principal nos formulários e nos detalhes de entradas próprias, permanecendo desativado enquanto não há prefixo registrado ou existe operação em andamento. Sempre abre confirmação com nome e caminho, explicando que todo o prefixo será apagado permanentemente, incluindo jogos/programas/configurações/saves internos e todas as entradas que compartilham esse prefixo. Cancelar não altera nada. O backend recebe apenas uma identidade registrada, verifica se o prefixo é a pasta real `<destino>/prefix`, recusa redirecionamento por link e prefixo em uso, remove o prefixo inteiro, o manifesto e o registro da biblioteca. Logs e executáveis fora do prefixo permanecem. Adaptadores Rust: `add_peligames_entry`, `uninstall_peligames_entry`, CLI `add-game`/`uninstall-game`, prévia `/__preview/peligames-add` e `/__preview/peligames-uninstall`.

Verificação desta etapa: 14 testes Rust aprovados, incluindo persistência da adição após rescan, remoção somente do prefixo e rejeição de prefixo redirecionado/entrada desconhecida. Build da interface e cargo check no Ubuntu aprovados. Prévia isolada verificou janela de opções, formulário Adicionar, registro → Iniciar/Desinstalar e confirmação → Cancelar, com preservação do registro/prefixo. A varredura e remoção efetiva do registro temporário foram verificadas pelo serviço Rust; o executável externo permaneceu intacto. Nenhum executável Windows foi iniciado nesse teste. Capturas: `docs/images/install-or-add.png` e `docs/images/prefix-uninstall-confirm.png`.

### Proteção de configurações inacabadas

Nos formulários Instalar jogo e Adicionar jogo, recolher os detalhes, selecionar outra capa ou trocar de modo pede confirmação quando há nome (mesmo ainda não confirmado), destino escolhido, Proton ou executável preenchido. “Continuar configurando” e o X preservam o formulário; “Sair e descartar” limpa o rascunho e executa a ação solicitada. Formulários vazios e instalações concluídas não exibem o aviso. Verificados no navegador o cancelamento, o descarte ao recolher e a troca de modo; `bun run build` aprovado.

### Biblioteca vazia

Sem jogos próprios cadastrados e sem busca ativa, o grid exibe um botão circular de 104 px com o símbolo +, centralizado na área disponível. A mensagem de biblioteca vazia fica abaixo, com a orientação “Clique no botão acima para começar.”, sem ícone adicional. O botão segue a paleta, tem efeitos de hover e foco de teclado e abre o seletor Instalar jogo / adicionar jogo existente. Abertura e retorno conferidos no navegador; build aprovado.

O botão Pasta da barra do grid é exclusivo da biblioteca do Gerenciador de mod (`libraryScope=all`). Na biblioteca principal de jogos próprios ele não aparece; Escanear permanece disponível.

Clicar no título PeliGames da title bar retorna ao grid inicial da biblioteca própria, recolhe detalhes e limpa a busca. O título é acessível por teclado, mantém a identidade visual e pede confirmação se houver rascunho de instalação/adição. Durante instalação ou gravação da biblioteca, a navegação fica desabilitada. Verificados retorno, cancelamento do aviso e limpeza da busca na prévia; build aprovado.

A title bar usa somente o título PeliGames clicável para retornar à tela inicial, sem ícone de casinha adicional. O botão Início com casinha permanece no grid do Gerenciador de mod.

No grid do Gerenciador de mod, Início (ícone de casinha) substitui Instalar jogo e retorna à biblioteca própria usando a mesma navegação da title bar. Na biblioteca principal, Instalar jogo continua abrindo o seletor de instalação/adição.

O seletor de Proton no carrossel usa “Instalar com Proton” no modo instalar e “Executar com Proton” no modo adicionar jogo existente.

A biblioteca vazia agora oferece dois círculos lado a lado: + para instalar/adicionar e quebra-cabeça para o Gerenciador de mod. Cada opção tem título e descrição abaixo, com a mesma paleta, hover e acessibilidade. O quebra-cabeça abre o seletor de mods existente.

O botão Confirmar do nome fica oculto em campos vazios e nomes sem alterações. Aparece quando há um nome não vazio novo ou alterado, nos modos instalar/adicionar e nas configurações do jogo instalado; Enter continua confirmando.

As preferências de tema usam um único botão alternando Claro/Escuro (sol/lua), seguido das sete cores compactas na mesma linha. Nomes das cores ficam acessíveis por tooltip e leitores de tela; removidos os textos explicativos de cor e aplicação imediata. A persistência automática permanece.

Correção do aviso de configuração inacabada na conclusão: instalação e adição bem-sucedidas usam uma transição própria para o jogo registrado, sem passar pela confirmação de abandono. Essa transição limpa o rascunho e confirmações pendentes. Seleção manual de outra capa, recolher detalhes e navegação continuam protegidos; falhas mantêm o formulário.

### Configurações do jogo instalado e painel avançado

O executável foi movido para Informações do jogo, logo abaixo de Diretório. Diretório mostra o prefixo completo em um campo com seletor de pasta; seleção salva imediatamente e edição manual confirma por Enter ou Confirmar. O backend valida um prefixo Wine/Proton existente (drive_c e system.reg), mantém a escolha após varreduras e a usa tanto em Iniciar quanto em Executar. Trocar o prefixo não move nem apaga arquivos anteriores. Desinstalar considera o prefixo atualmente associado e os registros que o compartilham.

Em Configurações, “Executar programas nesse prefixo” precede Executar. Config Avançada troca somente o painel direito por “Configurações avançadas”, vazio por enquanto, com slide lateral e retorno às informações. Conferidos os painéis e a gravação do prefixo em prévia isolada; build, 17 testes Rust e cargo check no Ubuntu aprovados.

As opções avançadas por jogo e suas fontes estão documentadas em [advanced-settings.md](advanced-settings.md), com os seis blocos, incluindo Gamescope, opções desligadas por padrão, comandos, dependências e bloqueios de conflito.
