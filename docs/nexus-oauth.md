# Nexus Mods: OAuth desktop com PKCE

A base OAuth usa o navegador padrão, PKCE S256 e retorno local. Não usa Client Secret, serviço intermediário, chave compartilhada ou login simulado. A chave pessoal continua disponível para testes locais; não substitui o registro exigido para distribuição pública.

## Registro e ativação

Solicitar ao Nexus Mods o registro de um **cliente público desktop**:

- Aplicativo: PeliGames, launcher Linux open source.
- Repositório: https://github.com/pelicanux/PeliGames
- Método: Authorization Code + PKCE S256.
- Redirect URI exato: `http://127.0.0.1:17849/callback`.
- Confirmar com o suporte os escopos necessários para metadados, perfil e downloads. Por enquanto o parâmetro `scope` é omitido, conforme a base do exemplo oficial; não foi inventado um escopo.

Após receber o identificador, definir `PELIGAMES_NEXUS_CLIENT_ID` **na compilação** e recompilar o launcher. O identificador é público. Nunca adicionar Client Secret ou tokens ao código, arquivos `.env` versionados ou argumentos do launcher. Sem esse identificador o botão de login fica desabilitado e explica a pendência.

Endpoints oficiais de produção:

- `https://users.nexusmods.com/oauth/authorize`
- `https://users.nexusmods.com/oauth/token`
- `https://users.nexusmods.com/oauth/userinfo`

## Comportamento

- Listener restrito a `127.0.0.1:17849`, aberto somente durante o login, por até 180 segundos. Porta ocupada gera orientação; não encerra outro processo.
- Verifier e state independentes gerados pelo gerador aleatório do sistema. State e caminho são conferidos; parâmetros duplicados, retorno estranho e código ausente são recusados.
- Troca do código feita no Rust; somente o resumo da conta chega à interface. Cancelar ou expirar fecha o listener. Cancelar não revoga uma conta previamente conectada.
- Access e refresh tokens ficam em um único item do chaveiro do sistema (`nexusmods-oauth`), nunca em localStorage ou configuração. Limite de tamanho e erros sanitizados; nada é registrado em logs.
- Se o endpoint de token recusar a renovação (400/401: autorização revogada no site, refresh token expirado ou reutilizado), a sessão OAuth é apagada do chaveiro e a conta aparece desconectada, em vez de reenviar um token morto a cada consulta. Falhas de rede, 429 e erros do servidor mantêm a sessão.
- Refresh automático 60 segundos antes da expiração; consultas de perfil e requests de API recebendo 401 renovam e tentam novamente uma única vez. Uma segunda recusa encerra a tentativa, sem repetição infinita ou troca silenciosa para outra conta/chave.
- A operação de conta e seu bloqueio pertencem a uma tarefa que continua até concluir a persistência, mesmo quando o download que a iniciou é cancelado. Desconectar aguarda essa tarefa antes de apagar credenciais, impedindo uma gravação atrasada após a desconexão.
- A remoção de uma credencial já ausente é bem-sucedida. Falhas reais do chaveiro continuam sendo reportadas, e a desconexão tenta remover as duas credenciais mesmo se uma remoção falhar.
- Cancelar durante a gravação de um novo login restaura a sessão anterior (ou remove a sessão recém-criada), sob o mesmo bloqueio da conta.
- OAuth tem prioridade sobre uma chave de teste existente. Conectar uma nova chave pessoal remove a sessão OAuth; desconectar remove ambos os itens locais. A remoção local não afirma revogar autorização no servidor Nexus.
- Catálogo, requisitos e resolução de downloads usam o mesmo caminho autenticado: Bearer para OAuth, `apikey` para chave pessoal.
- Conta gratuita continua autorizando cada arquivo pelo site; o launcher exige o link autorizado para download. Premium permite resolução direta. O login OAuth não concede Premium.

## Política e limites revisados

As consultas agora identificam `Application-Name`, `Application-Version` e `User-Agent`, inclusive validação de chave e manifesto público das extensões. Consultas são espaçadas em pelo menos 200 ms; respostas 429 e cotas zeradas ativam bloqueio local de novas consultas. `Retry-After` numérico é respeitado; na ausência dele usa-se uma pausa conservadora de 60 segundos. Não há repetição automática de 429. Nenhum segredo do usuário é enviado a servidor do PeliGames.

Isso é uma revisão dos pontos acima, não uma aprovação formal do Nexus nem uma auditoria completa de toda funcionalidade. O login real, rotação real, escopos e aceitação dos endpoints/redirect para este aplicativo dependem do client_id e da validação junto ao Nexus. Confirmar também quais funcionalidades de coleções e downloads o registro autoriza.

## Validação e resposta ao suporte

Validação local realizada: TypeScript sem erros, build Vite concluído, `cargo check --release --lib --features custom-protocol` aprovado e onze testes Rust aprovados. Além dos cinco testes iniciais de PKCE, callback e leitura de tokens, os testes de regressão cobrem credenciais ausentes versus falhas reais do chaveiro, cancelamento durante renovação/persistência seguido de logout, perfil HTTP 401 com novo token, segunda recusa 401 e erros sem nova tentativa. Os testes usam credenciais fictícias, armazenamento simulado e servidor HTTP local; não acessam contas reais. A preview também foi conferida com o login desabilitado por ausência do client_id. Os avisos Rust restantes já existiam fora desta integração. Depois que o Nexus fornecer o registro, testar consentimento aceito/recusado, cancelamento, expiração, reinício do launcher, refresh e downloads com contas gratuita e Premium.

## Situação do registro

O suporte do Nexus (Dominic) respondeu ao contato e o caminho escolhido foi Authorization Code + PKCE. Esta base existe para demonstrar o fluxo ao time do Nexus; o client_id só é entregue depois da aprovação do registro. Até lá `PELIGAMES_NEXUS_CLIENT_ID` fica sem valor e o botão de login permanece desabilitado, explicando a pendência. Ao receber o identificador: compilar com a variável definida e fazer os testes reais listados acima.

Referências primárias:

- https://help.nexusmods.com/article/114-api-acceptable-use-policy
- https://modding.wiki/en/api/oauth2-guide
- https://github.com/Nexus-Mods/oauth2-demo-app
- https://github.com/Nexus-Mods/node-nexus-api
