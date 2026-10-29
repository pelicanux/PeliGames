# GDK do AppImage

O AppImage usa uma variante da GDK 3.24.41 do Ubuntu 24.04, pacote fonte
`3.24.41-4ubuntu1.3`, incluindo os patches da distribuição. A alteração adicional
em `frame-clock.patch` solicita o callback de apresentação Wayland após pintura
OpenGL, permitindo que o relógio de quadros acompanhe a tela durante animações.
Não define uma taxa fixa de FPS e mantém os backends X11 e Wayland.

A preparação faz parte de `scripts/tauri.sh` quando o pacote selecionado inclui
AppImage. O comando normal `./compilar.sh` executa essa etapa no contêiner.
A biblioteca é copiada para `usr/lib/libgdk-3.so.0` dentro da imagem; o GTK do
sistema não é instalado nem modificado. O build-id da biblioteca no AppImage
final é conferido depois do linuxdeploy, que pode remover símbolos do ELF.

## Reconstrução da biblioteca

Use Ubuntu 24.04 amd64 com a versão correspondente de `libgtk-3-dev`, dependências
de desenvolvimento GTK/X11/Wayland/GL, um compilador C, Bun, `curl`, `dpkg-dev`,
`patch`, `xz-utils`, `binutils` e `patchelf`. Se Meson ou Ninja não estiverem
instalados, pacotes Ubuntu com SHA-256 fixado são extraídos apenas no temporário.
A compilação usa até 12 tarefas, prioridade baixa e uma pasta em tmpfs:

```sh
bash build.sh /tmp/peligames-gdk
```

No repositório, execute `bash scripts/gtk3-wayland/build.sh /tmp/peligames-gdk`.
A mesma pasta de saída pode ser usada novamente; os downloads são conferidos
antes do reúso. Não há instalação com `meson install`.

## Licença e fonte correspondente

A GDK mantém sua licença LGPL e os avisos originais de GTK e Ubuntu, fornecidos
em `COPYING` e `copyright` junto à biblioteca. Os manifestos e arquivos fonte
são obtidos do [arquivo oficial Ubuntu](https://archive.ubuntu.com/ubuntu/pool/main/g/gtk+3.0/)
com SHA-256 fixado no script.

Cada compilação do AppImage gera também
`Release/PeliGames_<versão>_GTK-source.tar.xz`, com os fontes upstream, os patches
Ubuntu, a alteração Wayland, as instruções, o script e os avisos originais.
Esse arquivo deve acompanhar o AppImage na distribuição; o script de publicação
local exige e inclui o arquivo na release. Ele fica separado para não aumentar
a imagem instalada. Preserve esses arquivos ao redistribuir a biblioteca.

Para reconstruir a partir desse arquivo, extraia-o e execute `build.sh` na pasta
`PeliGames-GTK-source`. Para evitar baixar os fontes novamente, copie o `.dsc` e
os dois arquivos `.tar.xz` fornecidos para `/tmp/peligames-gdk/downloads/` antes
de executar o comando. A saída será `/tmp/peligames-gdk/libgdk-3.so.0`.
