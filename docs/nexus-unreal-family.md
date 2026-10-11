# Família Unreal nos módulos Nexus

Atualizado em 10 de outubro de 2026.

Os módulos podem declarar `"family": {"kind": "unreal", "project": "<Projeto>"}`. Ao carregar, `nexus_modules.rs` expande a família seguindo as prioridades dos instaladores do template Unreal do Vortex:

1. Caminhos explícitos do projeto (`<Projeto>/Content/Paks/~mods`, `LogicMods`, `Binaries/Win64/ue4ss`, `dwmapi.dll`, `Binaries/Win64/Mods`).
2. Pastas reconhecidas em qualquer nível do pacote (`LogicMods/`, `ue4ss/`, `dwmapi.dll`, `~mods/`).
3. Rotas próprias do módulo.
4. Rotas de fallback: `.pak/.utoc/.ucas` para `~mods` e scripts UE4SS identificados por `Scripts/main.lua` ou `dlls/main.dll`.

Com UE4SS habilitado (padrão), o módulo recebe o requisito externo UE4SS (`dwmapi.dll` com `UE4SS.dll` no layout aninhado ou plano), o override `dwmapi=n,b` no Proton quando o loader está presente, a lista `mods.txt` gerenciada e a detecção de mods manuais. Antes, tudo isso existia apenas para o Palworld. Com `"ue4ss": false`, `ue4ss/` e `LogicMods/` são recusados. Builds Xbox/GDK (`Binaries/WinGDK`) são recusados.

O que **não** foi adotado da extensão Vortex: configurações e saves em AppData, filmes, substituição de paks originais e o fallback que copia qualquer arquivo para `Binaries/Win64`. Arquivos sem rota continuam recusando o pacote.

Módulos revisados migrados para a família: Palworld, Stellar Blade, Stalker 2, FF7 Rebirth e Clair Obscur (com UE4SS); Hogwarts Legacy, FF7 Remake, Code Vein, Crisis Core e Marvel Rivals (somente paks). Ready or Not (destino `Paks/`) e Oblivion Remastered (layout próprio) permanecem fora da família.

## Módulos gerados

`tools/vortex-import/run.sh --emit` grava `src-tauri/resources/nexus-games/unreal-vortex.json` e `unreal-vortex-audit.json`. São **72 jogos** (72 com UE4SS e 0 somente com paks). Para cada jogo, o gerador exige:

- extensão Vortex com revisão fixada e família Unreal (UE2/3/TFC excluídos);
- pasta do projeto igual a `EPIC_CODE_NAME` e executável Shipping dentro de `<Projeto>/Binaries/`;
- identidade forte: executável da raiz e Shipping, ambos com cabeçalho MZ;
- domínio presente na lista pública de jogos do Nexus e extensão publicada no manifesto público do Vortex (`nexus_game_id` registrado);
- identidade não contida na de outro módulo; nos casos ambíguos, os dois jogos são recusados.

Recusados nesta geração (16): sem ID: (sem ID); pasta ~mods do projeto não resolvida: borderlands3, crisoltheaterofidols, dynastywarriors3completeeditionremastered, myheroacademiaallsjustice, norse, reanimal, romeoisadeadman, routine, stateofdecay2, styxbladesofgreed; executável Shipping fora do projeto: keeper; projeto divergente: untildawn; domínio fora do padrão: windrose-server; identidade ambígua com runefactoryguardiansofazuma: daemonxmachinatitanicscion; identidade ambígua com daemonxmachinatitanicscion: runefactoryguardiansofazuma.

Validação: testes do perfil para todos os membros da família (paks, LogicMods, loader, scripts, recusa de executáveis, WinGDK e arquivos soltos), matriz de rotas da auditoria e unicidade de identidade entre todos os módulos. Nenhum desses jogos foi executado; `runtime_verified` permanece `false`.

## Ferramenta e arquivos

- `tools/vortex-import`: extração estática dos fatos das extensões Vortex, relatório comparativo e geração dos módulos (ver `tools/vortex-import/README.md`).
- `src-tauri/resources/nexus-games/unreal-vortex.json` e `unreal-vortex-audit.json`: módulos gerados e evidências, regravados pela ferramenta.
- `src-tauri/src/commands/nexus_modules.rs`: esquema `family`, expansão das rotas e testes da família.
