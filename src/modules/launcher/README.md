# Composição do launcher

Este módulo reúne apresentação e integrações da janela principal. O `App.tsx` coordena a biblioteca e compõe os recursos, preservando a seleção do jogo e as transições da capa.

- `SelectedGameOverview` apresenta os dados detectados; `GamePathTile` oferece ações de abrir e copiar caminhos. Ambos preservam as classes visuais compartilhadas.
- `gameInfoPresentation` trata valores desconhecidos e agrupa tecnologias de upscaling para apresentação e recomendações.
- `useGameReleaseDate` consulta a data de lançamento e ignora respostas de seleções anteriores. Os add-ons consultam seus próprios estados de instalação.
- `useLauncherAppearance` e `useLauncherWelcome` cuidam das preferências visuais, boas-vindas e oferta de integração do AppImage.
- `useLauncherOpenRequest` recebe pedidos externos de abertura sem reter callbacks antigos.
- `usePreferencesRequests` conecta eventos às rotas de preferências, respeitando os bloqueios fornecidos pelo coordenador.
- `useLibraryEntryDialogs` e `LibraryEntryDialogs` coordenam escolhas de executáveis e jogos descobertos após encerrar outro launcher. Usam o bloqueio de mutação da biblioteca fornecido pelo aplicativo.

As configurações compartilhadas pertencem a `modules/appConfig`; estes componentes não devem criar outra fonte persistente para elas. Ajustes e atalhos de mods são lidos e salvos pelo serviço do respectivo add-on.

Verificação das regras e da apresentação: `bun test tests/launcher tests/appConfig`.
