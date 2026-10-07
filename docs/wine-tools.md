# WineCFG e Winetricks

WineCFG e Winetricks ficam lado a lado abaixo de Config Avançada / Voltar às informações nas configurações do jogo instalado.

Implementação própria em Rust (`core/wine_tools.rs`), consultando o [Heroic](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/src/backend/tools/index.ts), o [Winetricks](https://github.com/Winetricks/winetricks) e o [UMU](https://github.com/Open-Wine-Components/umu-launcher).

WineCFG chama `umu-run winecfg`. Winetricks instala a seleção com `umu-run winetricks <componente>…`. O backend resolve a entrada registrada, o prefixo real e o Proton selecionado e define `WINEPREFIX`, `PROTONPATH`, `GAMEID=umu-default`, `STORE=none` e `PROTON_VERB=waitforexitandrun`. Não usa Wine do sistema, não troca runner, não aplica MangoHud/Gamescope/ajustes avançados de jogo e não monta comandos de shell. Requer um prefixo inicializado. Usa o UMU do sistema quando disponível; caso contrário, prepara automaticamente uma cópia própria em `$XDG_CONFIG_HOME/peligames/runners/umu`.

A janela Winetricks usa a superfície de vidro e a barra de título fixa do launcher. Mostra logs de preparação/execução, pesquisa DLLs/fontes pelo nome/descrição/categoria, seleção múltipla e instalação. Itens instalados são lidos de `winetricks.log`. A listagem é atualizada após a execução. A ferramenta limita a seleção a 40 componentes e aceita somente identificadores presentes no catálogo. Não instala dependências de sistema automaticamente; falhas e dependências externas faltantes aparecem no log da ferramenta.

O catálogo é lido preferencialmente do Winetricks incluído no Proton (`protonfixes/winetricks` ou `files/protonfixes/winetricks`). Caso não exista, o texto oficial é consultado via HTTPS e armazenado em `$XDG_CONFIG_HOME/PeliGames/tools/winetricks-catalog.txt`. Esse texto é interpretado como metadados, nunca executado. O UMU executa seu Winetricks com o runner escolhido; componentes recentes do catálogo oficial podem exigir atualização do runner/UMU. O script externo não é incorporado ao código do launcher.

Durante uma execução a janela bloqueia o fechamento e as seleções para evitar perder o acompanhamento. O fechamento fica disponível ao terminar ou falhar. Encerre jogos/programas no prefixo antes de iniciar WineCFG ou instalar componentes. A camada nativa compartilha o bloqueio de operações com instalação, edição e remoção; a prévia usa o mesmo backend Rust por um transporte NDJSON com verificação de origem. Logs são gravados em `logs` ao lado do prefixo.

Validação: build da interface, 27 testes Rust, cargo check no Ubuntu, comandos CLI em prefixo temporário com UMU simulado e teste visual da busca/seleção/atualização de itens instalados. Não foram instaladas dependências em jogos reais nem executado Wine real nesses testes.

## UMU gerenciado pelo PeliGames

`core/umu_manager.rs` consulta o release latest oficial de Open-Wine-Components/umu-launcher e baixa o pacote zipapp portátil. Confere tamanho e SHA-256 fornecidos pelo GitHub, extrai em uma pasta temporária, preserva o zipapp original com suas licenças e publica a versão somente após validar o executável. A cópia fica em `~/.config/peligames/runners/umu/<versão>/umu/umu-run`, com `current.json` registrando origem, versão e hash do executável. Há bloqueio de instalação entre processos; arquivos incompletos são removidos.

O resolvedor compartilhado prefere `umu-run` do PATH e `~/.local/bin`, depois reutiliza a cópia própria validada, baixando somente quando ela falta ou está danificada. Não consulta a rede em cada execução e não instala pacotes do sistema. O zipapp inclui bibliotecas Python, mas requer Python 3.10 ou superior do sistema. O próprio UMU pode baixar o Steam Runtime necessário na primeira execução.

A prévia utiliza o mesmo módulo Rust. O comando de desenvolvimento `peligames-proton-service install-umu` prepara explicitamente a cópia própria, mesmo quando há UMU no sistema. Download sem conexão, ausência de Python, checksum inválido e pacote inválido retornam erros; não iniciam o instalador/jogo nesses casos.
