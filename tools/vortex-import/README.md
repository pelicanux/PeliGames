# vortex-import

Extrai, por análise estática, os fatos declarativos das extensões de jogos do
Vortex e compara com os módulos Nexus do PeliGames
(`src-tauri/resources/nexus-games`). **Nenhum código das extensões é
executado** e nenhum módulo do launcher é alterado.

```sh
tools/vortex-import/run.sh            # baixa as revisões fixadas e gera o relatório
cargo test --manifest-path tools/vortex-import/Cargo.toml
```

- `sources.txt`: repositórios e commits lidos (os mesmos de `support-route-cases.json`).
  `fetch-sources.sh` os guarda em `~/.cache/peligames/vortex-sources`; do Vortex,
  só `extensions/games` é extraído.
- `generated/facts/<fonte>/<extensão>.json`: um arquivo por jogo, com lojas,
  executável, `requiredFiles`, pasta padrão, `stopPatterns`, mod types (destino e
  prioridade), instaladores (prioridade, extensões/nomes testados, mod type),
  ferramentas, links de dependências e recursos (ordem de carregamento, merge,
  hooks, diálogos de setup).
- `generated/report.md`: resumo por fonte e família, comparação com os módulos
  existentes e lista dos jogos do Vortex ainda sem módulo.

## Como a leitura funciona

O parser `oxc` lê JS/TS de todos os arquivos da extensão. Um avaliador limitado
resolve literais, constantes, objetos, spreads, `path.join`, templates,
`pathPattern`/`template`, retornos simples de funções, classes registradas com
`new` e laços `Object.keys(X).forEach` (extensões com vários jogos). Expressões
que dependem de execução ficam marcadas como `{?…}` e entram em `unresolved`.

Classificação:

- 🟢 dados declarativos resolvidos;
- 🟡 algum valor depende de execução, ou a família usa ordem de carregamento;
- 🔴 merge, ferramenta externa (Fluffy, Frosty, Anvil, Reloaded…) ou registro
  não resolvido.

`game_data_complete` indica que ID, loja, executável e algum destino foram
resolvidos. Instaladores, ordem de carregamento e hooks pertencem à família do
jogo e devem ser implementados uma vez por família.

## Limites

- Sem análise de escopo: nomes repetidos usam a primeira declaração com valor útil.
- Os sinais de instaladores são os textos citados no teste; a lógica exata do
  teste (combinações, negações) precisa ser conferida ao portar a família.
- Fatos estruturais não comprovam carregamento no jogo via Proton.
- Extensões comunitárias publicadas apenas no Nexus (`site/mods/…`) não estão
  nas fontes atuais e aparecem como "não encontrada" no relatório.

## Geração de módulos por família

```sh
tools/vortex-import/run.sh --emit
```

Regrava, para cada família (`unreal`, `unity`, `re_engine`),
`src-tauri/resources/nexus-games/<família>-vortex.json` (módulos com `family`) e
`<família>-vortex-route-cases.json` (proveniência e casos sintéticos de rota). A unicidade de identidade é verificada entre todas as famílias e os
módulos existentes. Confira os perfis de família em `src-tauri/src/core/nexus_modules.rs`. Baixa as listas públicas de jogos do
Nexus e de extensões Vortex (sem chave de API) para validar domínio e extensão.
Jogos com módulo próprio nunca são substituídos. Depois de gerar, rode os testes
do `src-tauri`: a matriz de rotas e a unicidade de identidade precisam passar.

## Ajustes manuais e versão dos módulos

- `overrides.json`: o que a análise estática não deduz, por domínio (`merge` mescla campos, `append` acrescenta itens que faltarem nas listas; `reason` é obrigatório). O `run.sh --emit` aplica os ajustes a cada geração, então nada feito à mão se perde. Um ajuste cujo jogo deixou de ser gerado interrompe a geração para revisão.
- **Versão:** cada módulo mantém a versão anterior quando o conteúdo gerado não mudou (as revisões nas URLs são ignoradas) e sobe a de correção (`1.0.0` → `1.0.1`) quando mudou. Jogos novos começam em `1.0.0`. Os casos de rota (`module_version`) acompanham.

## Downloads do GitHub

`github_downloads` (em `extract.rs`) reconhece o download do UE4SS do template
ChemBoy1 (`UE4SS_URL_API`, `prereleaseTag: "experimental-latest"`,
`UE4SS_DLFILE_STRING = "ue4ss_v"`) e o valor de `autoDownloadUe4ss`. Os módulos
Unreal com rotas UE4SS recebem o campo `downloads`, usado pelas verificações do
launcher para baixar e importar o pacote.

Nos módulos Unity, o importador lê também `BEP_VER`, `BEP_BE_VER`,
`BEP_BE_COMMIT`, `BEPINEX_PAGE_NO`/`MELON_PAGE_NO` e `allowBepinexNexus`/
`allowMelonNexus`, e emite os downloads do BepInEx (GitHub, Bleeding Edge ou
página do Nexus) e do MelonLoader, agrupados como alternativas.

## Verificação de atualizações

`check-updates.ts` compara a revisão fixada de cada extensão com a atual e não altera nada:

```
bun tools/vortex-import/check-updates.ts --output /tmp/peligames-updates.md [--git]
```

- **Fontes:** o `sources.txt` mais todas as URLs `github.com/<dono>/<repo>/blob/<commit>/…` citadas em `sources` e `vortex_evidence` dos recursos (`src-tauri/resources/nexus-games/*.json`). Um repositório renomeado é seguido pela própria comparação.
- **Relatório, por repositório:**
  - commits novos e link do diff;
  - por extensão alterada, os módulos que a usam e os arquivos mudados;
  - extensões novas, sem módulo.
- **Classificação:** "cosmética" quando só mudam documentação (`.md`, `.bbcode`, `.txt`), pacote montado (`.zip`), imagens, comentários ou números de versão; "regra" no restante.
- **Limite da API:** sem autenticação são 60 consultas por hora; com `GITHUB_TOKEN` no ambiente, 5000. Com `--git`, ou quando o limite é atingido, a comparação segue pelos repositórios públicos com `git` (clones parciais em `~/.cache/peligames/vortex-git`), sem limite.
- **Depois do relatório:**
  - **famílias geradas** (`*-vortex.json`): rode `run.sh --emit` numa pasta temporária, compare e copie o que estiver certo;
  - **módulos feitos à mão:** revise o diff, ajuste o módulo, suba a versão dele e atualize `support-route-cases.json`;
  - em ambos os casos, suba a revisão em `sources.txt` e nas URLs só depois da revisão.
