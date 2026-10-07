# Pacotes Linux

Execute `bash scripts/build-release.sh`. O script compila os dois binários com `custom-protocol`, verifica a interface embutida e gera os três formatos pelo wrapper `scripts/tauri.sh`. Usa todos os threads e um diretório temporário em `/tmp` (RAM quando `/tmp` é tmpfs). As saídas são copiadas para `Release/`.

## RPM e DEB

Os executáveis ficam em `/opt/PeliGames/PeliGames` e `/opt/PeliGames/Pelinstall`. As entradas de menu e “Abrir com Pelinstall” ficam em `/usr/share/applications`. Pequenos wrappers em `/usr/bin` preservam o acesso pelo terminal. O script de RPM substitui a cópia padrão do executável pelo wrapper antes de qualquer assinatura.

## AppImage

Um único arquivo inclui o launcher e o Pelinstall. Sem argumentos, abre o launcher; `--install /caminho/arquivo.exe` abre o instalador; `--launch-game IDENTIFICADOR` inicia um item da biblioteca em segundo plano. Um arquivo Windows passado diretamente também abre o Pelinstall.

A abertura registra as entradas do usuário em `~/.local/share/applications`, apontando para o AppImage externo. Os atalhos dos jogos também usam esse caminho permanente, verificado contra `APPDIR`, em vez do executável no ponto de montagem temporário. O AppImage deve permanecer nesse local. Mover o arquivo exige reabri-lo e atualizar os atalhos existentes.

Runners, prefixos e configurações continuam fora do AppImage, nos diretórios graváveis do usuário. O AppImage não exige instalação em `/opt`.

## Publicação

Atualize as versões em `package.json`, `src-tauri/Cargo.toml` e `src-tauri/tauri.conf.json` juntas. Depois de compilar e validar os pacotes, `bash scripts/publish-release.sh CAMINHO_DAS_NOTAS` publica somente AppImage, RPM e DEB no repositório `pelicanux/PeliGames`. Os binários independentes permanecem locais. Documentos privados, credenciais e `Release/` estão excluídos pelo `.gitignore`.
