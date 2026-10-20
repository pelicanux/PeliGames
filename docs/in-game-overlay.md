# Overlay PeliGames

O overlay reúne ferramentas dentro do jogo em uma única camada. É experimental e deve ser ativado individualmente para cada instalação. A chave **Overlay PeliGames** controla essa camada; MAKO e limite de FPS sem foco são funções independentes que podem ser usadas juntas.

## Ativar e usar

1. Nos detalhes do jogo, abra **Desempenho** e ative **Overlay PeliGames**.
2. Para controles de geração de quadros, instale e configure o MAKO e sua cópia de `Lossless.dll`, depois habilite-o em **Geração de quadros**. Para FPS sem foco, ative a opção em **Desempenho**. O overlay identifica as funções habilitadas; nenhuma delas exige ativar a outra.
3. Escolha o atalho, **F8** por padrão, e aguarde o salvamento automático.
4. Se o jogo pertence a outro launcher, siga as instruções do PeliRun exibidas na configuração.
5. Inicie ou reabra o jogo. Use o mesmo atalho para abrir e fechar o painel; arraste pelo cabeçalho para reposicioná-lo. Esc permanece reservado ao jogo.

A camada precisa ser carregada no início do processo. Ativá-la com o jogo aberto só terá efeito na próxima execução. O mesmo painel e atalho atendem às funções habilitadas. Os controles do MAKO aparecem quando ele está ativo; o limite sem foco trabalha mesmo com o painel fechado. Desligar a chave geral desativa o overlay e seu limite sem foco, mas não desliga a geração de quadros do MAKO. Trocar bibliotecas ou runtime exige reabrir o jogo.

## Compatibilidade e problemas

O alvo inicial é Linux x86_64 com Vulkan SDR. A compatibilidade depende do jogo, do Proton e do caminho Wayland/XWayland. HDR, OpenGL, processos gráficos Linux de 32 bits e sandbox Flatpak não têm suporte declarado. Executáveis Windows de 32 bits podem ser testados com o novo WoW64 e processo gráfico Linux de 64 bits.

Enquanto o painel estiver aberto, a interação deve ficar exclusiva dele. Se o jogo continuar recebendo movimentos ou cliques, feche o painel pelo atalho e desative o overlay antes de reabrir o jogo. Consulte os logs e informe jogo, Proton, compositor e modo Wayland/XWayland ao reportar um problema.

Veja o [guia MAKO e overlay](overlay.md) para requisitos e os [avisos de terceiros](../tools/game-overlay/NOTICES.md) para licenças e créditos.
