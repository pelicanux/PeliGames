# Atividade do PeliGames no Discord

Em **Configurações → Conexões → Discord**, ative **Mostrar gravação e replay no Discord**. O recurso começa desligado.

O Discord desktop precisa estar aberto com o compartilhamento de atividades permitido nas configurações de privacidade. A presença usa a conta aberta nesse aplicativo; ela é independente da conta vinculada no PeliGames para verificar benefícios. Não exige token de bot nem senha.

## Quando aparece

A atividade própria do **PeliGames** aparece somente quando uma captura estiver ativa:

- **Gravando tela:** gravação em andamento.
- **Replay instantâneo ativo:** o buffer de replay está capturando, mesmo antes de salvar um clipe.
- **Gravando tela · replay ativo:** os dois recursos em uso.

Com o launcher na biblioteca, um jogo aberto sem captura, a seleção do portal em andamento ou a captura encerrada, o PeliGames não publica atividade. O nome do jogo nunca é incluído. Nenhum vídeo, áudio, clipe, caminho local ou nome de arquivo é enviado pela integração.

Ao parar a gravação, a atividade só permanece se o replay continuar ativo. Ao encerrar ambos, desativar o compartilhamento ou sair do launcher, o PeliGames remove sua própria presença. O tempo exibido corresponde ao estado atual da captura, não ao tempo de jogo ou de uso da biblioteca.

A aplicação do PeliGames é separada da integração que um jogo tenha com o Discord. Ela não altera a presença de outras aplicações; a exibição e a prioridade entre atividades dependem do cliente Discord e das preferências da pessoa.

A conexão ocorre por IPC local, com detecção de sockets do Discord nativo e de instalações em sandbox. A presença tenta reconectar em segundo plano enquanto houver captura ativa e compartilhamento habilitado. Sem captura, não mantém conexão RPC para anunciar a biblioteca. O painel informa o estado da conexão.

Preferências antigas são lidas sem reativar o compartilhamento de capturas de quem o havia desligado. As opções anteriores de compartilhar jogo e biblioteca foram removidas.

## Aplicação Discord

O recurso reutiliza o ID público da aplicação Discord do projeto. O nome e o ícone exibidos pelo Discord vêm da configuração dessa aplicação no Developer Portal. Os mantenedores podem substituir o ID na compilação com `PELIGAMES_DISCORD_APPLICATION_ID`; isso não é uma configuração exigida de quem instala o launcher.

A prévia no navegador simula os controles e não publica atividade. O compartilhamento efetivo depende do aplicativo desktop compilado e do Discord aberto.

Referências: [protocolo oficial de Rich Presence](https://github.com/discord/discord-rpc/blob/master/documentation/hard-mode.md) e [atividades por aplicação](https://discord.com/developers/docs/slayer-sdk/classdiscordpp_1_1UserHandle.html).
