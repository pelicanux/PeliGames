# Serviço de Proton para a prévia

Este executável de desenvolvimento compartilha `src-tauri/src/core/proton_manager.rs` e `game_installation.rs` com o aplicativo Tauri. O protocolo usa JSON por linha em stdout. O servidor Vite transporta as respostas e eventos, sem reimplementar a lógica de instalação em JavaScript.

Compile no mesmo ambiente Linux onde o servidor Vite será executado:

```bash
cargo build --manifest-path tools/proton-service/Cargo.toml
bun run dev:ui
```

Em `preview.html`, abra **Preferências → Gerenciar Proton**, selecione GE-Proton ou Proton CachyOS e clique em **Atualizar Proton**. Downloads nessa tela são reais e gravam runners em `$XDG_CONFIG_HOME/peligames/runners/proton` (normalmente `~/.config/peligames/runners/proton`). Na aba **Instalar jogo**, confirmar o nome, escolher destino, Proton e executável e clicar em Instalar também é uma operação real: o serviço executa o instalador usando UMU e o runner escolhido. Requer `umu-run` no PATH ou em `~/.local/bin`.

O aplicativo desktop usa comandos Tauri diretamente e não depende deste executável. Não distribua o servidor Vite como parte do launcher.

## Consulta e instalação isoladas

```bash
tools/proton-service/target/debug/peligames-proton-service check ge-proton /tmp/peligames-runner-test
tools/proton-service/target/debug/peligames-proton-service install cachyos-proton /tmp/peligames-runner-test
```

O terceiro argumento é um destino opcional, útil para validar sem alterar os runners pessoais. Sem ele, usa a pasta de configuração do PeliGames. Durante `install`, uma linha recebida por stdin solicita cancelamento. O download em disco é descartado ao cancelar; uma versão já publicada permanece intacta.

```bash
cargo test --manifest-path tools/proton-service/Cargo.toml
```

Os testes cobrem arquitetura, formatos gzip/xz, permissão executável, cancelamento, digest e manifesto inválido. Para validar Tauri, use `cargo check --manifest-path src-tauri/Cargo.toml` em um ambiente com GTK/WebKit de desenvolvimento. Não compartilhe a pasta de artefatos Cargo entre host e container com versões distintas de glibc; use `CARGO_TARGET_DIR` separado.

O comando `run-game` recebe como segundo argumento um objeto JSON com `name`, `directory`, `executable` e `proton` (caminhos absolutos). Aguarda o término do instalador, registra stdout/stderr em `<destino>/logs` e retorna os caminhos do prefixo e do log. O fechamento da prévia não termina um instalador já aberto. O runtime Steam necessário pode ser baixado pelo UMU no primeiro uso. Os testes adicionais cobrem validação antes de criar arquivos, runner escolhido, caminhos com espaços/símbolos, preservação do prefixo e código de falha com log.


`monitor-game <executável registrado>` inicia uma sessão monitorada com o mesmo núcleo usado no aplicativo. Emite linhas JSON `execution` com identificador e estados starting/running/stopping/exited/cancelled/failed. Enviar `cancel` seguido de newline no stdin encerra o grupo e os processos descendentes da execução. O encerramento não usa kill global de Wine/Steam. A prévia mantém o processo de supervisão vivo entre requisições e abas, expondo início, consulta e cancelamento pela rota local `/__preview/game-execution`.


`update-game <JSON>` recebe `path` (identidade da entrada), `name`, `proton` e `executable`; valida os caminhos e salva as alterações por entrada no manifesto, preservando prefixo, capa e identidade durante varreduras. `monitor-game <entrada registrada> [executável auxiliar]` permite executar outro arquivo Windows com o prefixo/runner salvos da entrada, sem modificar o executável principal nem registrar uma nova instalação.


Adição de executáveis existentes: `add-game <JSON>` recebe os mesmos campos de `run-game`, registra somente o executável escolhido e prepara uma pasta de prefixo sem iniciar o programa. `uninstall-game <identidade registrada>` remove todo o prefixo associado e o registro da biblioteca, preservando os arquivos externos; a interface deve pedir confirmação antes de chamar este comando.
