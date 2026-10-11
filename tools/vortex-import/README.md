# vortex-import

Extrai, por análise estática, os fatos declarativos das extensões de jogos do
Vortex e compara com os módulos Nexus do PeliGames
(`src-tauri/resources/nexus-games`). **Nenhum código das extensões é
executado** e nenhum módulo do launcher é alterado.

```sh
tools/vortex-import/run.sh            # baixa as revisões fixadas e gera o relatório
cargo test --manifest-path tools/vortex-import/Cargo.toml
```

- `sources.txt`: repositórios e commits lidos (os mesmos de `support-audit.json`).
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
`<família>-vortex-audit.json` (evidência, casos de rota e jogos recusados com o
motivo). A unicidade de identidade é verificada entre todas as famílias e os
módulos existentes. Ver `docs/nexus-unreal-family.md`, `docs/nexus-unity-family.md` e `docs/nexus-re-engine-family.md`. Baixa as listas públicas de jogos do
Nexus e de extensões Vortex (sem chave de API) para validar domínio e extensão.
Jogos com módulo próprio nunca são substituídos. Depois de gerar, rode os testes
do `src-tauri`: a matriz de rotas e a unicidade de identidade precisam passar.
