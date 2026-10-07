# Cores de destaque

Preferências → Cor de destaque oferece vermelho, laranja, amarelo, verde, azul (padrão e cor oficial), anil e violeta. A escolha é aplicada imediatamente e salva em `peligames.accentColor` no armazenamento local da interface, independente do formulário de backend. Fechar ou cancelar esse formulário não desfaz a cor escolhida. Sem preferência salva, ou com valor inválido, o destaque usa azul (`#169bff`). Escolhas válidas já salvas são preservadas.

`src/theme/accentTheme.ts` define as paletas e suas variáveis CSS. `ThemeProvider.tsx` aplica essas variáveis à raiz, incluindo os diálogos renderizados em portais, e sincroniza abas da prévia. `AccentColorPreference.tsx` apresenta as sete opções com nomes e indicação da seleção.

Os antigos tons de vermelho decorativos usam variáveis compartilhadas: bordas, brilho de capas, ícones de configuração, botões de instalação/reparo, seleções, menus e diálogos. Cores de erro, ações destrutivas, sucesso, identificação de plataformas mantêm seu significado. A marca PeliGames e seu ícone acompanham a cor de destaque.

A imagem `src/assets/bg_main.jpg` permanece única. Ela é renderizada na camada `body::before`, com `hue-rotate` determinado pela paleta. O filtro não afeta textos, ícones ou capas; o blur dos painéis e o fundo dinâmico das capas permanecem. O modo Desempenho continua ocultando essa camada.

Verificação: compilação TypeScript/Vite; seleção das sete opções no navegador; persistência da escolha após recarregar; contraste mínimo de 4,5:1 entre texto e extremos dos gradientes dos botões primários em todas as paletas; revisão visual dos dois modos.

Captura: `images/accent-color-preferences.png`.

## Tema claro e rolagem

Preferências também oferece Escuro e Claro (branco). A aparência é independente da cor de destaque, aplicada imediatamente e salva em `peligames.appearance`. Somente escolhas explícitas escrevem no armazenamento; sincronizações entre abas atualizam a interface sem regravar preferências antigas.

`src/theme/appearance.css` define as superfícies em branco gelo e as cores neutras do tema claro. Os componentes mantêm valores de fallback para o tema escuro. A imagem de fundo permanece visível através das superfícies translúcidas, com luminosidade ajustada para a aparência clara. Painéis, janelas, menus e balões compartilham vidro com blur e bordas neon na cor de destaque. As camadas atrás dos diálogos escurecem menos a interface. Campos e textos usam tons mais escuros para manter a leitura; as capas preservam suas cores e seu reflexo branco. O modo Desempenho continua desativando blur, brilho e imagem de fundo.

As barras de rolagem usam o componente compartilhado `FloatingScrollbars`: indicadores sobrepostos ao conteúdo, sem trilho permanente nem espaço reservado na borda. Aparecem ao rolar ou ao aproximar o ponteiro a 24 pixels da borda direita. Permanecem visíveis enquanto o ponteiro está nessa região ou durante o arraste; fora dela, desaparecem suavemente após um segundo sem atividade. A cor acompanha o destaque nos dois temas. A rolagem nativa por roda, teclado e toque permanece no próprio contêiner.

Verificação: `bun run build`; créditos, licenças, preferências, painéis de mods e instalação, menu Proton e rolagem em 1250 × 900 e 800 × 850; retorno ao tema escuro; seleção das sete cores; persistência de Claro e Azul após recarregar. O contraste calculado dos tons de texto de destaque sobre a superfície clara e dos extremos dos botões primários excede 4,5:1 nas sete paletas.

Capturas: `images/light-theme-credits-final.png`, `images/light-theme-preferences.png`, `images/light-theme-mods.png`, `images/light-theme-install.png` e `images/dark-theme-rounded-scrollbar.png`.

Revisão do branco gelo: compilação TypeScript/Vite e conferência visual do fundo, painéis de instalação, menu Proton e janela de créditos. Capturas atualizadas: `images/ice-glass-install.png` e `images/ice-glass-credits.png`.

`src/theme/glassSurfaces.css` padroniza o acabamento dos painéis, diálogos, menus e balões: neon no canto superior esquerdo e inferior direito, bordas intermediárias neutras e iluminação radial interna nesses dois cantos. O menu da engrenagem usa opacidade de 98–96% na base para manter a leitura sobre o conteúdo. Conferido nos temas escuro e claro, incluindo créditos; captura: `images/corner-glass-light.png`.

Barra flutuante: compilação TypeScript/Vite e verificação nos créditos de repouso sem indicador, ativação por rolagem e proximidade do mouse, arraste até o topo e ocultação ao afastar o ponteiro. Captura: `images/floating-scroll-idle.png`.

Os diálogos usam `ModalSurface`: a superfície externa conserva moldura, blur e iluminação dos cantos, enquanto apenas `.modal-scroll-body` rola. Isso impede que o pseudo-elemento da borda acompanhe o conteúdo. A barra flutuante reconhece a nova área interna automaticamente. Verificado rolando preferências e créditos até o fim e abrindo a licença; compilação TypeScript/Vite aprovada. Captura: `images/fixed-frame-settings.png`.


A cor de **Games**, seu brilho e o ícone do pelicano acompanham a paleta selecionada, incluindo titlebar, créditos e placeholders genéricos. O ícone azul original é preservado; `--brand-hue` aplica rotação de matiz somente à imagem da marca, mantendo transparência e detalhes neutros. A moldura e o brilho do ícone nos créditos também usam os tokens de destaque. A escolha azul conserva a imagem original, sem rotação.
