# Comunidade Matrix — chat retrô

Acesso pela barra superior, ao lado do Discord, e por Preferências → Conexões.
Interface própria com fonte Jersey 10 (empacotada em `public/fonts`), fundo de terminal,
nomes coloridos e avatares. A comunidade é `!pNHMqrnxSAmcVFqrgf:matrix.org`; as salas são
as de primeiro nível dela, criadas pela administração (poucas salas gerais, não uma por jogo).

## Login e sessão

- Login SSO de matrix.org no navegador externo, com retorno numa porta aleatória de
  127.0.0.1 e caminho com nonce. O pedido expira após três minutos; conexões estranhas
  nessa porta são ignoradas sem cancelar o login.
- Troca do loginToken no backend, sem registrar URLs/tokens em logs.
- access_token guardado no chaveiro do sistema (libsecret/secret-tool) e lido uma vez
  por execução; o token nunca vai ao frontend. Logout revoga a sessão no Matrix.
- Se o servidor recusar o token (401: sessão encerrada no Element, revogada), a sessão
  local é apagada e o chat volta a oferecer o login. Desconectar com um token já
  recusado também funciona.

## Mensagens

- Um único `/sync` com long-poll (30 s) roda enquanto houver login, mesmo com o chat
  fechado, filtrado às salas da comunidade. Mensagens chegam na hora, sem polling.
- Menções (Matrix `m.mentions`, ou o nome do usuário no texto) vão para o sininho
  de notificações; clicar abre o chat.
- Edições substituem o texto ("editada"); mensagens apagadas somem. Respostas mostram
  a mensagem respondida e não repetem a citação antiga no texto. `/me` envia ação.
- Nomes de exibição das salas (displayname) no lugar do `@usuario`.
- Histórico: os últimos 50 eventos por sala, e "carregar mensagens anteriores" pagina
  para trás. A rolagem só segue o fim quando o leitor já está lá.
- Enter envia; Shift+Enter quebra a linha. Links abrem no navegador externo.
- Imagens e logs de até 5 MB, enviados como bytes brutos pelo IPC.
- Avatares e imagens passam pelo backend autenticado, usando thumbnails de até 1 MB.
  O cliente não executa HTML de mensagens; renderiza corpos como texto React.

## Moderação

- Ignorar usuário usa a lista `m.ignored_user_list` da conta: o servidor deixa de
  entregar as mensagens dele em todos os clientes. Os ignorados aparecem na barra
  lateral, com opção de voltar a mostrar.
- Denunciar envia a mensagem aos moderadores do servidor (`/rooms/{sala}/report/{evento}`),
  com motivo opcional.
- Quem tem permissão na sala (power levels do Matrix) vê também apagar mensagem,
  expulsar e banir, com motivo opcional e confirmação. Qualquer pessoa pode apagar as
  próprias mensagens; expulsar e banir só valem para quem tem nível menor. O servidor
  confere as permissões de novo.
- Regras da comunidade e moderadores ficam a cargo da administração das salas.

## Versão oficial

Ao abrir, o launcher calcula o SHA-256 do próprio executável e procura na release
da mesma versão o arquivo `PeliGames_<versão>_binaries.sha256`, gerado na compilação
das releases oficiais e publicado junto com os pacotes.
O resultado (oficial, compilada localmente ou não verificada) entra no relatório de
erro e na postagem do fórum do Discord. Uma versão modificada pode falsificar a
resposta; a verificação serve para identificar builds locais no suporte.

## Salas criptografadas

Salas criptografadas são identificadas e o envio direto é bloqueado. A opção Abrir
no Element permite usar um cliente com E2EE completo; não há fallback que envie texto
ou anexos sem criptografia numa sala criptografada.

## Element complementar

Uma janela remota isolada abre `app.element.io` para salas criptografadas e recursos
avançados. Ela não recebe capacidades IPC do launcher: default.json autoriza apenas
`main`. Navegação interna permanece em app.element.io e matrix.org; links HTTPS para
outros sites abrem no navegador externo. Sessão desse cliente é separada do chat
próprio. A configuração não concede novos acessos a páginas remotas.

## Preview e verificação

A preview simula login e mensagens, com aviso visível de demonstração; não envia
mensagens, denúncias nem arquivos reais. O backend compila e tem testes para a leitura
de eventos (respostas, edições, mensagens apagadas, nomes). Ainda falta validar na
versão nativa, com uma conta real: o SSO do matrix.org (que passou a usar o MAS), o
acesso ao espaço/salas, o envio, o upload e a denúncia.

Fontes oficiais:
- https://spec.matrix.org/latest/client-server-api/
- https://web-docs.element.dev/


## Suporte premium automático

A sala `!d6uopxWbqNBgbKYC8uTxjgvzIP4F955uQi3rerKKX2Q` aparece na lista com cadeado
para usuários sem benefício Discord. O conteúdo/compositor ficam ocultos enquanto
a autorização não estiver confirmada. As outras salas continuam livres.

Ao abrir o chat com Matrix e Discord conectados, o backend consulta o Worker,
aceita uma sala temporária de confirmação, envia um nonce com a sessão Matrix local,
e entrega ao Worker somente o ID desse evento. O Worker confere o remetente e o
cargo Discord, convida para a sala premium, e o backend aceita o convite. Nenhum
access token pessoal Matrix passa pelo frontend ou pelo Worker. A sala temporária
é abandonada/esquecida pelo backend ao final.

A validação local de benefícios decide o estado visual; a autorização real continua
no Worker e na sala privada Matrix. Erros mantêm a sala bloqueada e oferecem tentar
novamente. A revalidação diária/remoção roda no Worker mesmo com o launcher fechado.
A preview owner/free simula os estados; o teste real precisa do aplicativo nativo
recompilado e das duas contas conectadas. Detalhes em
`tools/discord-auth/MATRIX-PREMIUM.md`.
