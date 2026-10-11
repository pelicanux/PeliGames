# Família RE Engine nos módulos Nexus

Atualizado em 10 de outubro de 2026.

`"family": {"kind": "re_engine"}` aplica as regras do REFramework compartilhadas pelos jogos RE Engine. Elas são as mesmas que os módulos revisados já usavam, agora num único perfil em `nexus_modules.rs`:

- **Rotas:** `reframework/autorun`, `reframework/plugins` e `reframework/data` (também dentro de invólucros); scripts `.lua` soltos na raiz do pacote vão para `reframework/autorun`; `dinput8.dll` na raiz do pacote vai para a raiz do jogo.
- **Requisito e Proton:** requisito externo REFramework (`dinput8.dll`) e override `dinput8=n,b` no Proton quando o carregador está presente.
- **Recusados:** `natives/` e `pak_mods/`. Mods Fluffy, substituições natives e invalidação de paks exigem processamento especializado e não são aplicados.

## Módulos

- **Migrados (versão 1.1.0):** Monster Hunter Rise, Monster Hunter Wilds, Resident Evil 2, 3, 4 e Village, Devil May Cry 5, Dragon's Dogma 2, Street Fighter 6 e Resident Evil Requiem. As regras dos dez eram idênticas e continuam iguais; a matriz de rotas da auditoria passa sem alteração.
- **Gerados (`re_engine-vortex.json`):** Dead Rising Deluxe Remaster, Monster Hunter Stories 3, Onimusha: Way of the Sword, Pragmata e Resident Evil 7.
  - O gerador só aceita extensões do template "Fluffy + REFramework" que instalam o REFramework.
  - Usa a constante `EXEC` do jogo completo e recusa executáveis de demo (`trial`/`demo`) ou do Xbox. No RE7, o primeiro retorno da extensão era a demo `re7trial.exe`.
  - Uncharted (`.psarc`) aparece na mesma família do Vortex, mas não é RE Engine e fica fora.

Nenhum desses jogos foi executado; `runtime_verified` permanece `false`.

## Template Basic do Vortex

O template "Basic Game" (43 jogos) não foi convertido em família. Muitas extensões têm constantes não preenchidas (`XXX`) ou lógica escrita jogo a jogo, e os instaladores permissivos copiam executáveis e DLLs para a raiz. Gerar módulos a partir disso adivinharia destinos; esses jogos exigem revisão individual.
