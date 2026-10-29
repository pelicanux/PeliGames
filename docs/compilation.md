# Compilar no container

Com o container Ubuntu de desenvolvimento já preparado e o projeto acessível no mesmo caminho dentro dele, abra um terminal na raiz do projeto:

```sh
./compilar.sh
```

O comando usa `ubuntu-24-dev`, iniciando-o se estiver parado. Gera os executáveis **PeliGames**, **PeliInstall** e o **AppImage** na pasta `Release/`. Não publica nada no GitHub. O container precisa ter as dependências de compilação, Clang 21 ou superior, Bun e Rust; os crates necessários devem estar disponíveis no cache, pois o build Rust usa modo offline. `/tmp` precisa estar em tmpfs no container.

O AppImage também prepara sua biblioteca GDK com a correção de sincronização Wayland. Essa etapa usa até 12 tarefas e baixa fontes Ubuntu e, se necessário, Meson/Ninja, conferindo seus SHA-256. O contêiner precisa de `dpkg-dev`, `patch`, `xz-utils`, `binutils`, `patchelf` e das dependências de desenvolvimento GTK/X11/Wayland/GL. O arquivo `Release/PeliGames_<versão>_GTK-source.tar.xz` contém a fonte correspondente e deve acompanhar o AppImage na distribuição. Veja [a preparação da GDK](../scripts/gtk3-wayland/README.md). `--bin` não executa essa etapa.

Outras opções:

```sh
./compilar.sh --bin                 # somente os executáveis
./compilar.sh --all                 # executáveis, AppImage, DEB e RPM
./compilar.sh --jobs 4              # reduzir tarefas paralelas
./compilar.sh --container NOME      # outro container já preparado
./compilar.sh --help
```

São usadas seis tarefas por padrão. Para `--all`, o host também precisa de Bun e `bsdtar` para gerar os hashes dos pacotes.

## Resultado e erros

O terminal mostra o início e a conclusão. A saída detalhada fica em `Release/compilacao.log`; a execução anterior fica em `Release/compilacao-anterior.log`. Se ocorrer um erro, o terminal mostra as últimas linhas e retorna um código diferente de zero. Uma segunda compilação simultânea no mesmo projeto é recusada.

Os executáveis ficam em `Release/peligames` e `Release/pelinstall`; o pacote fica em `Release/PeliGames-x86_64.AppImage`. Cada compilação substitui os arquivos correspondentes ao modo escolhido. Pacotes antigos de outros formatos permanecem na pasta: `--bin` não atualiza o AppImage, DEB ou RPM.

`Release/SHA256SUMS-local.txt` contém os hashes dos executáveis e do AppImage gerados pelo modo escolhido. Para acompanhar o log por vontade própria, em outro terminal:

```sh
tail -f Release/compilacao.log
```
