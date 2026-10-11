# Família Unity nos módulos Nexus

Atualizado em 10 de outubro de 2026.

Os módulos podem declarar `"family": {"kind": "unity", "data": "<Jogo>_Data", "loaders": ["bepinex", "melonloader"]}`. A família reproduz o template híbrido BepInEx/MelonLoader do Vortex (ChemBoy1); `loaders` lista os carregadores que a extensão oferece para o jogo.

## Rotas

Expandidas por `nexus_modules.rs` ao carregar o módulo:

- **BepInEx:** `BepInEx/`, `winhttp.dll`, `doorstop_config.ini` e `.doorstop_version` na raiz; pastas `plugins/`, `patchers/` e `config/` vão para `BepInEx/`. O módulo recebe o requisito externo BepInEx e o override `winhttp=n,b` quando o loader está presente.
- **MelonLoader:** `MelonLoader/`, `version.dll` e `dobby.dll` na raiz; pastas `Mods/`, `Plugins/`, `UserLibs/` e `UserData/`. O módulo recebe o requisito externo MelonLoader e o override `version=n,b`.
- **DLL solta:** somente na raiz do pacote, depois de remover um invólucro comum. Recebe uma rota canônica e é ajustada pela etapa abaixo. DLLs em pastas irmãs (variantes) continuam recusadas.
- **Recusados:** `<Jogo>_Data/` (dados, `Managed/` e Assembly), arquivos `.assets`, `.resource` e `.ress`, e a pasta do carregador que o jogo não usa.

## Etapa por pacote (`nexus_unity.rs`)

Como no Vortex, cada DLL em pasta de mods é lida e classificada pelo carregador que referencia nos metadados .NET:

| Referência | Destino |
|---|---|
| `BepInEx.Preloader.Core.Patching` | `BepInEx/patchers/<id do mod>/` |
| `BepInEx` | `BepInEx/plugins/<id do mod>/` (ou a pasta do pacote) |
| `MelonPlugin` | `Plugins/` |
| `MelonLoader` | `Mods/` |

O pacote é recusado, sem aplicar arquivos, quando:
- uma DLL não referencia nenhum carregador;
- o pacote mistura mods dos dois carregadores, ou traz BepInEx e MelonLoader juntos;
- o mod é de um carregador que o jogo não usa no módulo;
- o mod é de um carregador diferente do instalado no jogo (ou do que o próprio pacote instala);
- os dois carregadores já estão instalados no jogo.

Configurações reescritas pelo jogo (`BepInEx/config/*.cfg`, `UserData/`) são preservadas como configuração, e o log `MelonLoader/Latest.log` entra na coleta de logs de carregadores.

**Diferenças em relação ao Vortex:** as substituições de `<Jogo>_Data`, Assembly DLL e `.assets`, os carregadores próprios de cada jogo, o instalador de saves e o fallback que copia qualquer arquivo para a raiz não foram adotados.

## Módulos gerados

`tools/vortex-import/run.sh --emit` grava `src-tauri/resources/nexus-games/unity-vortex.json` e `unity-vortex-audit.json`. São **23 jogos**: 8 com BepInEx e MelonLoader, 14 só com BepInEx e 1 só com MelonLoader.

Para cada jogo, o gerador exige:
- extensão do template híbrido (UMM, MSCLoader e loaders integrados ficam fora);
- pasta `_Data`, executável real (não `gamelaunchhelper.exe`) e arquitetura x64;
- carregadores derivados de `loaderChoice` e `recommendedLoader`;
- jogo que não seja XNA;
- domínio e extensão confirmados nas listas públicas do Nexus e do Vortex;
- identidade exclusiva (executável + `UnityPlayer.dll`).

Recusados nesta geração: animeshopsimulator (sem extensão Vortex publicada no manifesto público); romestead (jogo XNA: sem pasta de dados Unity).

Validação: testes de classificação das DLLs, carregador errado ou misto, detecção do carregador instalado, rotas de todos os membros da família, matriz de rotas da auditoria e unicidade de identidade. Nenhum jogo foi executado; `runtime_verified` permanece `false`.

## Módulos revisados migrados

Subnautica, Subnautica: Below Zero, TCG Card Shop Simulator, Supermarket Simulator e R.E.P.O. usam a família com `loaders: ["bepinex"]` (versão 1.1.0). As rotas duplicadas foram removidas; permanecem apenas as regras próprias:
- requisitos BepInEx específicos de cada jogo (a família não acrescenta um segundo);
- `QMods/` recusado nos jogos Subnautica e no TCG/Supermarket;
- no R.E.P.O., REPOLib, bundles `.repobundle`, pacotes com `manifest.json` do Thunderstore e a recusa das pastas MelonLoader.

Mudança de comportamento: DLLs passam a ser classificadas pela referência ao carregador. Um mod MelonLoader é recusado com mensagem explícita, em vez de ser copiado para `BepInEx/plugins`. Bibliotecas auxiliares sem referência acompanham os plugins do pacote. O Valheim continua com o perfil nativo Linux próprio (`valheim_profile`).
