# Ícones do aplicativo

A fonte dos ícones é [`public/peligames.svg`](../../public/peligames.svg), composta por caminhos vetoriais. A interface e os atalhos criados pelo launcher utilizam esse SVG.

`bun run icons` gera as variantes nativas exigidas pelo Tauri em `generated/`. Esses arquivos derivados ficam fora do Git; os comandos de desenvolvimento, compilação e empacotamento fazem a geração automaticamente. PNG, ICO e ICNS são formatos de saída para as plataformas, não fontes mantidas do design.

Ícones extraídos de executáveis de jogos são dados externos e preservam seu formato raster original.
