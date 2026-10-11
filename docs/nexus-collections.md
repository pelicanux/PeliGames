# Coleções do Nexus

Atualizado em 10 de outubro de 2026.

O botão **Coleção** (aba Mods de um jogo Nexus) e os links `nxm://<jogo>/collections/<slug>[/revisions/<n>]` abrem a janela da coleção. Ela aceita o link da página (`nexusmods.com/games/<jogo>/collections/<slug>`, também `next.nexusmods.com`) ou o slug.

## Fluxo

1. **Revisão:** `collectionRevision` na API GraphQL v2 (sem revisão no link, usa a última publicada). A lista traz `modId`, `fileId`, nome do arquivo, tamanho, versão e se o mod é opcional. Coleções de outro jogo são recusadas, e arquivos já presentes na biblioteca (mesmo `modId` e `fileId`) são mostrados e não baixados de novo.
2. **Escolha:** os obrigatórios entram sempre; os opcionais, só se marcados. Ao instalar, o backend relê a revisão escolhida; a interface só informa os opcionais.
3. **Instruções:** o pacote da revisão (`downloadLink`, autenticado com a chave do usuário) é baixado e convertido, e o `collection.json` é lido. De cada mod de origem Nexus vêm as escolhas do instalador (`choices`, tipo `fomod`), o tipo de mod do Vortex (`details.type`) e a fase (`phase`). Sem esse pacote, os mods ainda entram na fila, e as opções FOMOD ficam para o usuário confirmar.
4. **Fila:** os arquivos entram na fila de downloads como um lote, em ordem de fase. Conta Premium baixa direto; conta gratuita autoriza um arquivo por vez no navegador ("Mod Manager Download" ou "Slow download" monitorado na pasta Downloads).
5. **Instalação de cada arquivo:**
   - **Plano normal** do módulo do jogo.
   - **Tipo de mod do Vortex:** se o pacote não for reconhecido e o módulo mapear o tipo (`vortex_mod_types`) para um destino dentro dos seus destinos permitidos, é aplicada a pasta raiz manual com esse destino.
   - **FOMOD:** as escolhas da coleção são convertidas pelos nomes de passo, grupo e opção (o índice só desempata nomes repetidos). Se algum nome não corresponder, nada é adivinhado: o mod fica importado, com aviso, para o usuário escolher.
   - **Pendências:** requisitos ausentes ou problemas do plano também deixam o mod importado, com a mensagem.
   - **Ordem:** depois de instalar, os mods da coleção são reordenados na biblioteca na ordem do curador. Os demais mods mantêm suas posições.

## Limites

- Quando dois mods têm o mesmo arquivo, vence o que vem depois na lista (ver "Prioridade de sobrescrita" em `NEXUS_GAME_SUPPORT.md`). A coleção reordena seus mods na ordem do curador.
- Ainda não são aplicados: regras de ordem (`modRules`), patches binários (`patches`), `fileOverrides`, mods de fontes externas (`browse`, `manual`, `direct`) e a ordem de plugins Bethesda da coleção.
- O mapa `vortex_mod_types` vem dos módulos gerados pelo `tools/vortex-import`. Os módulos escritos à mão ainda não o declaram.

## Referências e validação

- Formato do `collection.json`: `Nexus-Mods/extension-collections`, `src/types/ICollection.ts` (commit `109ae729e82a9b27ade67969f771ba7f6542346b`). As escolhas FOMOD vêm de `installerChoices` (`src/util/transformCollection.ts`).
- O esquema GraphQL foi conferido por introspecção da API pública. Uma revisão real (coleção `htknoa`, Stardew Valley) serve de exemplo nos testes.
- Testes cobrem: interpretação de links, revisão (duplicados, outro jogo, `downloadLink` adulterado), `collection.json` (só fontes Nexus do jogo) e a conversão de escolhas FOMOD (nomes, índice para nomes repetidos e recusa sem correspondência).
- Nenhuma coleção foi instalada de verdade nesta validação; a janela foi conferida na prévia com dados de demonstração.

Implementação: `src-tauri/src/commands/nexus_collections.rs`, `nexus_downloads.rs` (`queue_collection`, `install_curated`) e `src/modules/nexus/NexusCollectionDialog.tsx`.
