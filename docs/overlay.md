# MAKO e overlay PeliGames — experimental

Disponíveis para testes na versão 0.4.5. A camada do overlay é nova e ainda pode apresentar falhas de entrada, foco, cursor, atalhos ou renderização, conforme o jogo, Proton e compositor. O funcionamento em KDE não certifica GNOME, XFCE, Cinnamon ou window managers; as rotas Wayland e Wine/Proton XWayland precisam de validação real em cada ambiente.

## Preparar um jogo

1. Em **Configurações → Mods → MAKO**, instale o componente. O pacote vem do projeto oficial MAKO; ele não inclui a DLL do Lossless Scaling.
2. Instale sua cópia do **Lossless Scaling** pela Steam, na ramificação pública padrão. O PeliGames procura `Lossless.dll` nas bibliotecas Steam, inclusive em discos montados, e permite selecioná-la manualmente.
3. Abra **Biblioteca → detalhes do jogo → Geração de quadros** e ligue **Ativar neste jogo**. Ajuste o multiplicador e as demais opções. As alterações são salvas automaticamente; aguarde a confirmação de salvamento ou confira a mensagem de erro.
4. Ligue **Overlay PeliGames** se quiser os controles dentro do jogo. Escolha o atalho (padrão **F8**) antes de iniciar a partida. O carregamento da camada, a troca de atalho/DLL/runtime e a ativação exigem reabrir o jogo.
5. Na primeira configuração de um launcher externo, siga o comando fornecido pelo PeliRun. Na Steam, cole a linha nas opções de inicialização e use a verificação, ou escolha aplicação automática, que fecha e reabre a Steam. No Heroic use os campos de Wrapper; no Lutris, Prefixo de comando. Use o executável e os argumentos fornecidos pela interface; `%command%` pertence ao formato Steam, não aos campos de wrapper do Heroic/Lutris. Outros launchers exigem campos que aceitem encadear o comando real.
6. Abra o jogo. Com ele em foco, F8 abre e fecha o painel. Arraste pelo cabeçalho para mudar sua posição. Esc continua reservado ao jogo. O painel só fica disponível se a camada tiver sido carregada nesse processo.

A configuração é por instalação. Vincular o PeliRun uma vez permite modificar o perfil e reabrir o jogo sem editar repetidamente a linha da Steam ou reiniciar o cliente. O PeliGames aplica seus próprios perfis ao abrir os jogos que gerencia; o uso direto por outro launcher exige o vínculo nesse launcher. Proton da Steam continua sendo escolhido na aba Compatibilidade da própria Steam.

## Ajustes durante a partida

Com o jogo iniciado com MAKO, multiplicador, modo adaptativo, FPS alvo, escala de movimento e modelo leve podem ser alterados no launcher, sem botão Salvar. Os controles disponíveis no overlay também atualizam o perfil do MAKO. O carregamento de DLLs/camadas e outras opções de inicialização continuam dependendo de uma nova execução do jogo.

O painel deve consumir a interação do mouse enquanto aberto e devolvê-la ao jogo ao fechar. A revisão desta versão corrige o reconhecimento da consulta de cursor do GE-Proton, mas ainda precisa de validação gráfica nas diferentes combinações. Se a câmera ou ações do jogo continuarem respondendo enquanto o painel estiver aberto, feche-o pelo atalho, desative **Overlay PeliGames**, reabra o jogo e reporte o jogo, Proton, modo Wayland/XWayland e logs.

## Limites e compatibilidade

- Overlay inicial para Linux x86_64, Vulkan SDR (8 bits ou formatos UNORM de 10 bits), com entrada Wayland ou Wine/Proton XWayland. HDR, OpenGL, caminhos nativos XCB e sandbox Flatpak não têm suporte declarado pelo overlay.
- Jogos Windows de 32 bits podem habilitar o teste com o **novo WoW64**, desde que o processo gráfico Linux seja de 64 bits. Isso não confirma suporte de todo Proton. Processos Linux gráficos de 32 bits continuam bloqueados.
- MAKO e overlay são opções distintas: desativar o overlay não obriga desativar a geração de quadros. MAKO em Flatpak tem preparação de extensão/permissões específica; isso não habilita o overlay no sandbox.
- Não empilhe geradores de quadros. Para MAKO, desative o FG interno e o FG do OptiScaler. As verificações de conflitos conhecidos permanecem; não há mais uma caixa de confirmação obrigatória para salvar.
- Se o desktop reservar a combinação, ela pode não chegar ao jogo. Escolha outro atalho. O PeliGames não registra uma tecla global nem altera atalhos de outros programas.
- A camada é carregada dentro do processo do jogo. Não há garantia de aceitação por anti-cheat; evite testar em jogos protegidos.
- Os testes automatizados e de carregamento não substituem testes gráficos reais. Esta publicação é uma **prerelease experimental**, não uma certificação de compatibilidade universal.

## Runtimes e diagnóstico

Runtimes, versões anteriores e perfis ficam em `~/.config/PeliGames/FrameGeneration`. O PeliRun mantém um executável compartilhado e um JSON por perfil em `~/.local/share/pelirun`; cada perfil tem identificador curto. Atualizações não devem substituir runtimes ainda vinculados a outros jogos. Os detalhes técnicos estão em [PeliRun](steam-launch-runtime.md) e na [camada experimental](in-game-overlay.md).

Ao reportar problemas, inclua versão do PeliGames, nome do jogo, Proton, compositor e se usa Wayland nativo ou XWayland. Consulte os logs do jogo e, para execuções diretas pela Steam, o console da Steam; procure mensagens `[PeliGames overlay]`. Nunca publique credenciais ou dados de conta junto dos logs.
