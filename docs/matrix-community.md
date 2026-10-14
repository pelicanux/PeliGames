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

A lista usa somente salas reais retornadas pelo Matrix, com seus nomes originais.
A sala privada configurada também pode ser descoberta após a confirmação de participação,
mesmo quando não aparece na hierarquia do espaço. Nenhuma entrada "Suporte premium"
é criada pelo cliente. Quando a sala configurada estiver visível, o cadeado indica
que o benefício Discord ainda não foi validado. O conteúdo/compositor ficam ocultos enquanto
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

## Salas de voz nativas (2026-10-14)

Os canais de voz ficam na lista de canais, como no Discord: clicar entra na chamada (ou troca de canal). Um painel fixo abaixo dos canais mostra a conexão de voz ([sair da voz]), a redução de ruído, a conta (foto, nome, quem está falando) e os botões de microfone, de silenciar o áudio (desliga recepção e microfone) e a engrenagem. A engrenagem abre Perfil (foto e nome, globais no Matrix), Voz e áudio (microfone e saída via PipeWire/`pactl`, trocáveis durante a chamada; volume do microfone e da recepção de 0 a 200%; medidor; redução de ruído; ganho automático AGC2 do WebRTC) e Conta (desconectar). As preferências ficam em `peligames.matrixVoice` (localStorage) e valem para as próximas chamadas. Quem fala é destacado na lista do canal (nível RMS acima de 0,015, mantido por 300 ms). Fechar o chat ou navegar entre bibliotecas mantém a chamada; sair, desconectar a conta ou encerrar o aplicativo libera os dispositivos. Há uma chamada ativa por processo.

Chat: contador de não lidas por canal (contagem do servidor, zerada por marcador de leitura privado `m.read.private`), "fulano está digitando…" (`m.typing`), edição das próprias mensagens (`m.replace`) e reações com emoji (`m.reaction`; reagir de novo remove). Imagens e avatares ficam em cache enquanto o launcher está aberto.

A sinalização usa Matrix JS SDK 44.0.0, com a camada oficial Rust/WASM de criptografia. Uma única sincronização atende ao chat e aos eventos de dispositivo; ela continua limitada às salas da comunidade e à sala em chamada. Como no Element Call, a mídia só é criptografada de ponta a ponta em salas criptografadas (`m.room.encryption`); nelas, as chaves de mídia são enviadas por mensagens Olm. Em salas sem criptografia, todos os clientes enviam e esperam áudio sem E2EE: criptografar só do nosso lado fazia o Element tocar os quadros cifrados como ruído. O patch em `patches/matrix-js-sdk@44.0.0.patch` carrega as chaves dos dispositivos antes do envio nesses casos. O banco criptográfico usa uma chave aleatória protegida pelo chaveiro do sistema. O access token Matrix permanece no Rust.

O áudio usa LiveKit Rust, CPAL e os filtros já testados, sem depender do WebRTC da webview. A mídia usa AES-GCM, chaves individuais e derivação HKDF compatível com o provedor do Element Call (janela de ratchet 10, anel de 256 chaves). As filas de reprodução são independentes e limitadas a 100 ms por faixa; a mixagem satura a soma para evitar transbordamento.

A autorização OpenID e os tokens de mídia ficam no Rust. O primeiro transporte é o serviço anunciado por Matrix.org, `https://livekit-jwt.call.matrix.org`, através do endpoint legado `/sfu/get`, com identidade `user_id:device_id`. O backend confirma participação na sala antes de pedir o token. A proteção do áudio privado depende também da distribuição criptografada das chaves aos dispositivos dos participantes; o endpoint legado, isoladamente, não deve ser considerado uma verificação de acesso à sala. Chamadas com outros serviços/SFUs ainda não são atendidas por esta integração.

Validação automática: duas instâncias reais do SDK, com contas fictícias e homeserver em memória, trocaram chaves Olm e rotacionaram a chave após uma saída. O servidor LiveKit local transportou áudio sintético nas duas direções, em modos normal e criptografado; a chave incorreta impediu áudio audível. A autorização OpenID no serviço Matrix.org também foi validada com uma sessão existente, sem capturar áudio nem publicar participação. Isso não substitui a validação de uma conversa entre duas contas reais no serviço Matrix.org, nem o teste de interoperabilidade com Element e redes diferentes. A prévia do navegador continua sendo uma simulação e não captura microfone nem entra em chamadas reais.

Compilação: frontend e intermediários Rust ficam em `/tmp` (tmpfs). Após validação, os binários e os avisos de dependências são copiados para `Release`.

Criação de canais: o backend consulta `m.room_versions` em `/capabilities` e usa explicitamente a versão padrão anunciada pelo servidor. Nas versões 12 e posteriores, remove o criador de `power_level_content_override.users`, inclusive quando esse usuário veio das permissões do espaço; o criador já tem poder infinito nessas versões. Nas versões anteriores, mantém o nível 100 do criador. Os moderadores herdados e as permissões de participação na chamada são preservados. Recusas mostram HTTP e um código Matrix validado, sem expor texto arbitrário da resposta. Testes cobrem os pedidos de canais de texto/voz nas versões 11 e 12 e a filtragem das mensagens de erro. Regra oficial: [criadores em salas v12](https://spec.matrix.org/v1.16/rooms/v12/).

Referências oficiais: [Matrix JS SDK](https://github.com/matrix-org/matrix-js-sdk), [autorização MatrixRTC/LiveKit](https://github.com/element-hq/lk-jwt-service), [provedor de chaves do Element Call](https://github.com/element-hq/element-call/blob/main/src/e2ee/matrixKeyProvider.ts) e [SDK LiveKit Rust](https://github.com/livekit/rust-sdks).
