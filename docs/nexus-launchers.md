# Jogos de outros launchers na biblioteca Nexus

Atualizado em 10 de outubro de 2026.

A varredura da biblioteca Nexus lê, sem alterar nada, os jogos instalados pela Steam e por quatro launchers de Linux, nas versões nativa e Flatpak. Cada jogo entra com o prefixo Wine/Proton do próprio launcher e, quando possível, com o mesmo Proton. A identificação do jogo continua sendo feita pelos módulos (arquivos e cabeçalho do executável), nunca pelo nome.

**Só entram jogos com suporte.** A varredura automática adiciona apenas os jogos que um módulo reconhece e cujo suporte no Vortex está confirmado (`vortex_supported`, que segue o `vortex_status` da auditoria; um teste confere os dois). Os demais jogos instalados são ignorados. O Mass Effect Legendary Edition, com suporte Vortex não confirmado, fica de fora da detecção automática, mas ainda pode ser adicionado manualmente.

**Limpeza ao escanear.** Cada clique em escanear também remove da biblioteca Nexus os jogos detectados automaticamente (Steam, Heroic, Lutris, Faugus, Bottles e instalações do PeliGames) que não têm mais suporte, inclusive aqueles cuja pasta sumiu. As regras de exceção são:

- **Adicionados pelo menu do Nexus** (`launcher: "Manual"`): continuam na biblioteca e são removidos com o botão direito na capa.
- **Com mods instalados ou em execução:** continuam na biblioteca, porque removê-los deixaria arquivos órfãos no jogo. O motivo fica registrado no log do jogo.
- **Arquivos baixados:** os pacotes de mods de um jogo removido não são apagados do disco.

| Launcher | Arquivos lidos | Prefixo | Proton |
|---|---|---|---|
| Heroic (Epic, GOG, Amazon, sideload) | `~/.config/heroic` e `~/.var/app/com.heroicgameslauncher.hgl/config/heroic`: `installed.json` de cada loja, biblioteca em cache e `GamesConfig/<app>.json` | `winePrefix` | `wineVersion.bin` quando `type` é `proton` |
| Lutris | `games/*.yml` em `~/.local/share/lutris`, `~/.config/lutris` e nas pastas do Flatpak `net.lutris.Lutris` | `game.prefix` | `wine.version`, procurado em `runners/wine`, `runners/proton` e `compatibilitytools.d` |
| Faugus Launcher | `faugus-launcher/games.json` em `~/.local/share` e no Flatpak `io.github.Faugus.faugus-launcher` | `prefix` (exceto `Linux-Native` e `Steam`) | `runner`, procurado em `compatibilitytools.d` |
| Bottles | `bottles/*/bottle.yml` em `~/.local/share/bottles` e no Flatpak `com.usebottles.bottles` | a própria garrafa (`Custom_Path` quando definido) | `Runner`, procurado em `runners` |

- **Pasta do jogo:** é a pasta do executável registrado. A descoberta sobe até seis níveis para achar a raiz que o módulo reconhece. No Bottles, caminhos Windows (`C:\...`) são convertidos para `drive_c` da garrafa, e caminhos com `..` são recusados.
- **Proton:** só é registrado um runner com o script `proton`, que é o que o lançamento com mods usa. Wine comum (por exemplo `wine-ge` do Lutris) deixa o campo vazio, e o usuário escolhe o Proton nas configurações do jogo.
- **Nome:** o Lutris não guarda o título no YAML do jogo. O nome vem do arquivo (`fallout-4-1712345678.yml` → "Fallout 4") e é trocado pelo nome do módulo quando o jogo é reconhecido. No GOG via Heroic, o título vem da biblioteca em cache, porque o `installed.json` só tem o ID.
- **Duplicados:** a mesma pasta aparece uma vez só. A Steam vem primeiro e, depois, Heroic, Lutris, Faugus e Bottles.

## Pastas por loja (GOG e Epic)

As edições GOG e Epic de alguns jogos Bethesda usam outras pastas em `AppData/Local` (lista de plugins) e em `Documentos/My Games` (INI e logs). O campo `store_variants` dos módulos troca essas pastas quando um arquivo que só aquela loja instala está na pasta do jogo. A regra é a mesma da libloadorder, usada pelo LOOT:

| Jogo | Arquivo | Pasta |
|---|---|---|
| Skyrim SE | `Galaxy64.dll` | `Skyrim Special Edition GOG` |
| Skyrim SE | `EOSSDK-Win64-Shipping.dll` | `Skyrim Special Edition EPIC` |
| Enderal SE | `Galaxy64.dll` | `Enderal Special Edition GOG` |
| Fallout 4 | `EOSSDK-Win64-Shipping.dll` | `Fallout4 EPIC` |
| Fallout New Vegas | `EOSSDK-Win32-Shipping.dll` | `FalloutNV_Epic` |

- **Independe do launcher:** a detecção vale para um jogo GOG ou Epic instalado por qualquer launcher.
- **Prioridade:** o GOG vence o Epic quando os dois arquivos existem, como na libloadorder.
- **Fora daqui:** as edições da Microsoft Store não são tratadas.

## Lançamento

- **Prefixo no formato da Steam** (com `pfx/`, como os do Heroic com Proton): é iniciado pelo Proton quando existe uma pasta da Steam (nativa, `~/.steam/steam` ou Flatpak).
- **Sem Steam instalada:** o `umu-run` executa a mesma pasta. Antes, faltar a Steam nativa impedia o lançamento.

## Limites

- **Testes:** os leitores foram testados com arquivos sintéticos de cada launcher e com o Heroic real desta máquina (jogos Epic e sideload). Lutris, Faugus e Bottles não tinham jogos instalados aqui.
- **Prefixo padrão:** um jogo sem prefixo no launcher (por exemplo, o prefixo padrão do Lutris) entra sem prefixo, e o usuário escolhe nas configurações.
- **Outros launchers:** itch, Minigalaxy e outros ainda não são lidos.
