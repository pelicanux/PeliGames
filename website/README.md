# Página pública do PeliGames

Site estático publicado em https://pelicanux.github.io/PeliGames/. Usa as capturas de demonstração do tour de boas-vindas, a marca e as fontes locais do launcher. Não depende da execução do aplicativo, de frameworks ou de serviços de análise.

## Prévia local

Na raiz do repositório, execute:

```sh
node website/build.mjs --output /tmp/peligames-site
bunx vite /tmp/peligames-site --host 127.0.0.1 --port 8080
```

Abra http://localhost:8080. Escolha outro diretório de saída se ele já existir. O montador não sobrescreve diretórios existentes.

## Publicação

O workflow `GitHub Pages` publica alterações de `website/`, de seus recursos públicos e do próprio workflow na branch `main`. Também atualiza os downloads quando uma release é publicada ou editada, e pode ser executado manualmente pelo GitHub Actions.

Os links usam a release pública de publicação mais recente, incluindo versões experimentais. Rascunhos são ignorados. Os três pacotes Linux precisam estar disponíveis antes de publicar a release. Os metadados são gerados durante a publicação; o navegador não precisa consultar a API do GitHub. Os mesmos links e a versão atual também são gravados no HTML para funcionar com JavaScript desativado.

Somente os arquivos estáticos e os recursos listados explicitamente em `build.mjs` entram no site. Fontes Jersey 10 e Oxanium são distribuídas com suas licenças OFL. As capturas são demonstrações da interface; marcas e imagens dos jogos pertencem a seus respectivos titulares.
