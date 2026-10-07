# PeliGames

Launcher de jogos Windows para Linux com Proton, biblioteca de jogos, Pelinstall, ferramentas Wine e gerenciador de mods DLSSNR-AMD.

## Instalação

Baixe AppImage, RPM ou DEB em [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM e DEB instalam PeliGames e Pelinstall em `/opt/PeliGames`; os atalhos ficam nos diretórios de aplicativos do sistema.
- AppImage reúne os dois módulos. Dê permissão de execução e abra o arquivo. A primeira abertura registra o launcher e “Abrir com Pelinstall” para o usuário. Mantenha o AppImage no mesmo local para preservar os atalhos.
- Para abrir um executável Windows no instalador: `./PeliGames.AppImage --install /caminho/programa.exe`.

Proton, UMU e Winetricks são baixados conforme necessário. Configurações e ferramentas ficam em `~/.config/PeliGames`. O backend e os arquivos do mod ficam em `~/.config/PeliGames/Mod/DLSSNR`. As pastas antigas são migradas automaticamente, preservando arquivos existentes. Prefixos e jogos permanecem nos diretórios escolhidos pelo usuário. Capas do SteamGridDB exigem uma chave pessoal opcional, configurada em Preferências.

## Desenvolvimento e compilação

Requer Linux, Rust, Bun e as dependências de compilação do [Tauri 2](https://v2.tauri.app/start/prerequisites/), incluindo WebKitGTK 4.1. Para RPM, instale também `rpm`, `rpmbuild` e `cpio`.

```sh
bun install --frozen-lockfile
bun run tauri dev
# Binários locais:
bash scripts/build-binaries.sh
# Binários + AppImage, RPM e DEB:
bash scripts/build-release.sh
```

Os scripts usam todos os threads disponíveis e um diretório temporário em `/tmp`. Para compilar em RAM, `/tmp` deve ser tmpfs. As saídas ficam em `Release/`. Os binários incluem a interface e não dependem de localhost.

## Créditos e licença

PeliGames é desenvolvido por Pelicano e distribuído sob a licença [MIT](LICENSE).

O gerenciador de mods integra os projetos [DLSSNR-AMD, de mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD), e [DLSSNR-RDNA3, de mauri870](https://github.com/mauri870/DLSSNR-RDNA3). As licenças e avisos desses componentes estão em [licenses/](licenses/README.md) e na janela Sobre do aplicativo.

O launcher utiliza Proton, UMU e Winetricks, mantidos por seus respectivos projetos. Esses componentes têm suas próprias licenças e são obtidos separadamente pelo aplicativo.
