# Catálogo de instaladores

O catálogo oferece receitas para Ubisoft Connect, EA app e Battle.net no assistente **Instalar jogo**. Escolha o launcher, um destino novo e o Proton disponível. Conclua as telas do instalador e feche o aplicativo instalado para que o PeliGames confirme seu executável.

As receitas têm estado `research-only`: instalação, login e jogos ainda não têm compatibilidade certificada. São necessários `osslsigncode` e certificados CA para verificar os instaladores oficiais. Os componentes opcionais devem ser selecionados no assistente. Em falha ou cancelamento, prefixo e logs ficam disponíveis para diagnóstico.

## Atualizar o catálogo

- `index.json` lista as receitas, política e hashes.
- `recipe.schema.json` define a estrutura permitida.
- `recipes/*.json` define os instaladores e ajustes disponíveis.

Para validar alterações:

```sh
bun scripts/validate-launcher-recipes.ts
bun test ./scripts/test-launcher-recipes.ts
```

Ao alterar uma receita, atualize `recipe_version`, o SHA-256 no índice e `catalog_version`. O validador rejeita comandos livres e downloads fora dos hosts permitidos. Use somente fontes oficiais e preserve as licenças dos componentes distribuídos.
