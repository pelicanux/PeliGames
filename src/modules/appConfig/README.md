# Configurações compartilhadas

`AppConfig` fica em `src/types/appConfig.ts`. O backend mantém a configuração persistida. `appConfigStore` coordena a sua representação e as gravações da janela; os componentes usam `useAppConfig()`.

- `config`: última resposta confirmada pelo backend. `null` significa ausência de configuração; os padrões do editor não equivalem a setup concluído.
- `draft`: configuração confirmada com as alterações locais ainda não confirmadas. O editor usa essa representação.
- `update(patch)`: coloca uma alteração na fila compartilhada. Mapas de atalhos e destinos são mesclados por chave. Cada operação lê a configuração atual, grava e verifica a resposta persistida.
- `refresh()`: atualiza a leitura sem apagar intenções de edição pendentes. A janela também atualiza ao receber foco.
- `retry()`: repete alterações que falharam ou tenta novamente a leitura inicial. Uma falha não impede as operações seguintes.

O adaptador Tauri fica em `appConfig.ts`; o estado e a fila independentes de React ficam em `store.ts`. Novos consumidores devem usar essa API, evitando gravar cópias completas de configurações mantidas por componentes. A fila cobre a janela atual; mudanças externas continuam sob autoridade do backend e são reconciliadas por leitura.

Verificação: `bun test tests/appConfig`.
