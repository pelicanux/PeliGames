# Overlay PeliGames

O overlay reúne ferramentas dentro do jogo em uma única camada. É experimental e deve ser ativado individualmente para cada instalação. A chave **Overlay PeliGames** controla essa camada; MAKO e limite de FPS sem foco são funções independentes que podem ser usadas juntas.

## Ativar e usar

1. Nos detalhes do jogo, abra **Desempenho** e ative **Overlay PeliGames**.
2. Para controles de geração de quadros, instale e configure o MAKO e sua cópia de `Lossless.dll`, depois habilite-o em **Geração de quadros**. Para FPS sem foco, ative a opção em **Desempenho**. O overlay identifica as funções habilitadas; nenhuma delas exige ativar a outra.
3. Escolha o atalho, **F8** por padrão, e aguarde o salvamento automático.
4. Se o jogo pertence a outro launcher, siga as instruções do PeliRun exibidas na configuração.
5. Inicie ou reabra o jogo. Use o mesmo atalho para abrir e fechar o painel; arraste pelo cabeçalho para reposicioná-lo. Esc permanece reservado ao jogo.

A camada precisa ser carregada no início do processo. Ativá-la com o jogo aberto só terá efeito na próxima execução. O mesmo painel e atalho atendem às funções habilitadas. Os controles do MAKO aparecem quando ele está ativo; o limite sem foco trabalha mesmo com o painel fechado. Desligar a chave geral desativa o overlay e seu limite sem foco, mas não desliga a geração de quadros do MAKO. Trocar bibliotecas ou runtime exige reabrir o jogo.

## Jogos online

O overlay foi feito para jogos offline. Ele carrega bibliotecas no processo do jogo e intercepta teclado e mouse para o painel interativo. Em jogos online com anti-cheat ou proteção própria, isso pode ser interpretado como trapaça e levar a bloqueio ou banimento da conta. O uso em jogos online é por conta e risco de quem joga.

## Compatibilidade e problemas

O alvo inicial é Linux x86_64 com Vulkan SDR. A compatibilidade depende do jogo, do Proton e do caminho Wayland/XWayland. HDR, OpenGL e processos gráficos Linux de 32 bits não têm suporte declarado. Executáveis Windows de 32 bits podem ser testados com o novo WoW64 e processo gráfico Linux de 64 bits.

Jogos nativos Linux recebem teclado e mouse no painel apenas com Wayland nativo. Em X11 ou XWayland, o atalho funciona nos jogos executados pelo Wine ou Proton.

Na Steam Flatpak, o PeliGames copia o overlay para a pasta do PeliRun dentro do sandbox da Steam; abra a Steam Flatpak uma vez antes de ativá-lo. Outros launchers Flatpak e o MAKO no Flatpak ainda não carregam o overlay.

Enquanto o painel estiver aberto, a interação deve ficar exclusiva dele. Se o jogo continuar recebendo movimentos ou cliques, feche o painel pelo atalho e desative o overlay antes de reabrir o jogo. Depois de cada execução, **Desempenho** mostra o resultado da última sessão do overlay: se ele carregou, se o jogo criou uma tela compatível e se o teclado e o mouse foram reconhecidos. Abra **Etapas registradas** e informe esse conteúdo, o jogo, o Proton, o compositor e o modo Wayland/XWayland ao reportar um problema.

Veja o [guia MAKO e overlay](overlay.md) para requisitos e os [avisos de terceiros](../tools/game-overlay/NOTICES.md) para licenças e créditos.
