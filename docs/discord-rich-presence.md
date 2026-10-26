# Atividade do PeliGames no Discord

Em **Configurações → Conexões → Discord**, ative **Compartilhar atividade no Discord**. O recurso começa desligado.

O Discord desktop precisa estar aberto com o compartilhamento de atividades permitido nas configurações de privacidade. A presença usa a conta aberta nesse aplicativo; ela é independente da conta vinculada no PeliGames para verificar benefícios. Não exige token de bot nem senha.

## O que compartilhar

- **Mostrar o jogo em execução:** publica o nome de um jogo iniciado pelo PeliGames quando a sessão estiver em execução. Preparação de prefixos e inicialização não são anunciadas como jogo ativo.
- **Mostrar gravação e replay:** indica gravação de gameplay, replay instantâneo ativo ou ambos, quando a captura do PeliGames estiver ativa.

Desativar essas opções oculta os respectivos detalhes. Com ambas desligadas e o compartilhamento principal ligado, a atividade mostra apenas o uso do PeliGames. Nenhum vídeo, clipe, caminho local ou nome de arquivo é enviado pela integração.

Ao desativar o compartilhamento principal, o PeliGames remove a presença que ele publicou. Ao sair do launcher, a conexão também é encerrada. Outras atividades e processos do Discord não são controlados.

A conexão ocorre por IPC local, com detecção de sockets do Discord nativo e de instalações em sandbox. Se o Discord estiver fechado ou indisponível, a presença tenta reconectar em segundo plano enquanto o compartilhamento estiver habilitado. O painel informa o estado da conexão.

## Aplicação Discord

O recurso reutiliza o ID público da aplicação Discord do projeto. O nome e o ícone exibidos pelo Discord vêm da configuração dessa aplicação no Developer Portal. Os mantenedores podem substituir o ID na compilação com `PELIGAMES_DISCORD_APPLICATION_ID`; isso não é uma configuração exigida de quem instala o launcher.

A prévia no navegador simula os controles e não publica atividade. O compartilhamento efetivo depende do aplicativo desktop compilado e do Discord aberto.

Referência: [protocolo oficial de Rich Presence](https://github.com/discord/discord-rpc/blob/master/documentation/hard-mode.md).
