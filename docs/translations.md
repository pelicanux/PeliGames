# Tradução da interface

O PeliGames usa i18next e react-i18next com catálogos JSON locais. O idioma inicial é **Automático (sistema)**; uma escolha manual nas configurações, no instalador ou nas boas-vindas fica salva. No Linux, a detecção considera o idioma de mensagens (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES` e `LANG`), com fallback para a preferência do navegador integrado. Preferências antigas `pt` continuam válidas como `pt-BR`.

Os catálogos ficam em `src/i18n/locales/<idioma>/<área>.json`. Inglês (`en`) é a referência; português brasileiro usa `pt-BR`. Somente o catálogo inglês e o idioma selecionado precisam ser carregados. O carregamento é reutilizado nas próximas trocas. Se um catálogo falhar, a escolha anterior é preservada; na inicialização, a interface pode usar inglês.

## Acrescentar um idioma

Use uma identificação BCP 47, como `es`, `fr` ou `zh-Hans`. Exporte um kit para uma pasta fora dos catálogos:

```sh
bun run i18n:export es /tmp/peligames-es-kit
```

Cada mensagem do kit contém `source`, para contexto, e `translation`, para preencher. Preserve as variáveis como `{{name}}`, os nomes de teclas e os termos técnicos. Não altere `source`; se ele ficou desatualizado, exporte outro kit. Novos textos devem usar chaves estáveis e variáveis nomeadas, evitando montar frases traduzidas por concatenação.

No arquivo `language.json`, configure o nome nativo, a direção e os aliases:

```json
{
  "nativeName": "Español",
  "direction": "ltr",
  "aliases": ["es-ES"],
  "published": false
}
```

`published: false` permite guardar uma tradução em andamento sem oferecê-la no seletor. Mude para `true` depois da revisão; a validação exigirá todas as mensagens e as formas plurais necessárias àquele idioma. Para idiomas escritos da direita para a esquerda, use `direction: "rtl"` e confira também o layout.

Importe e valide:

```sh
bun run i18n:import es /tmp/peligames-es-kit
bun run i18n:check
bun run i18n:test
bun run i18n:audit
```

A importação verifica chaves, variáveis e metadados antes de gravar os catálogos. O registro dos idiomas, os tipos das chaves e os carregadores são gerados por `bun run i18n:generate`, também executado em `dev` e `build`. Não edite os arquivos `*.generated.ts` manualmente. Um idioma publicado entra nos seletores pelos metadados; não é necessário adicionar opções em componentes React.

## Usar mensagens no código

```tsx
const {t, language} = useI18n();
const label = t("settings", "language");
const amount = new Intl.NumberFormat(language).format(value);
```

Para novas mensagens, use `t("área", "chave", {name})` e `{{name}}` no JSON. Plurais seguem as regras do idioma: por exemplo, `item_one` e `item_other` são chamados por `t("área", "item", {count})`. Outros idiomas podem exigir `zero`, `two`, `few` e `many`; o kit inclui as formas necessárias. Serviços sem React podem receber o idioma e usar `translator(language)`.

Mensagens ausentes usam o catálogo inglês. Dados fornecidos por jogos, nomes de mods, conversas e conteúdo de serviços externos mantêm seu idioma original. Ainda existem textos fixos de telas antigas e mensagens livres do backend a migrar; acrescentar um catálogo não traduz automaticamente esses conteúdos. `i18n:audit` lista possíveis textos visíveis ainda fixos para orientar essa migração e rejeita condicionais binárias de português/inglês. Nomes técnicos também podem aparecer nesse relatório e exigem revisão humana. Erros técnicos devem preservar seus detalhes e, nas novas APIs, oferecer códigos e parâmetros para a interface localizar a mensagem.
