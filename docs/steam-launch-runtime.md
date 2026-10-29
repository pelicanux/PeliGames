# Usar o PeliRun com outros launchers

O PeliRun lê o perfil salvo pelo PeliGames a cada início do jogo. Depois de vinculá-lo uma vez, alterações no perfil valem na próxima execução sem trocar repetidamente o comando no launcher externo. O PeliGames pode estar fechado.

## Configurar pela primeira vez

Configure o jogo no PeliGames e copie o comando fornecido pela interface. O identificador legível, como `tomb-raider`, pertence à instalação cadastrada; não precisa ser obtido na Steam ou na Epic. Não invente o identificador nem copie o de outro jogo.

- **Steam:** cole a linha completa nas opções de inicialização, incluindo `%command%`, e use a verificação. A alternativa de aplicação automática fecha e reabre a Steam. Uma edição manual salva pela própria Steam não exige reiniciar o cliente.
- **Heroic:** use os campos de Wrapper e argumentos indicados pelo PeliGames. Não acrescente `%command%`.
- **Lutris:** use Prefixo de comando, conforme a instrução exibida. Não acrescente `%command%`.
- **Outros launchers:** é necessário um campo que permita executar um wrapper e entregar a ele o comando real do jogo. Campos de argumentos comuns não garantem esse funcionamento.
- **Jogos iniciados pelo PeliGames:** o launcher aplica o perfil associado à instalação.

Use os caminhos e argumentos apresentados na interface: eles dependem do local da instalação e do launcher. Steam Flatpak exige acesso aos arquivos pelo sandbox; um caminho válido no host pode não estar disponível nele.

## Quando as alterações entram em vigor

Variáveis, argumentos, camadas e bibliotecas de inicialização são lidos ao reabrir o jogo. Isso não significa que toda opção possa mudar durante uma partida: recarga ao vivo depende de cada ferramenta. O Proton usado pela Steam continua sendo escolhido em **Propriedades → Compatibilidade** na própria Steam.

O executável do PeliRun é compartilhado; cada instalação tem seu próprio perfil JSON. Renomear um jogo não exige um novo identificador. Identificadores antigos no formato `pg-xxxxxxxx` continuam funcionando por meio dos aliases do perfil. Apagar ou corromper um perfil não apaga os perfis dos demais jogos.

## Localizar os arquivos

Os perfis universais ficam em `~/.local/share/pelirun/profiles`. Use o comando gerado na interface para localizar o executável correto, especialmente em instalações Flatpak. Evite mover ou excluir o executável enquanto houver jogos vinculados a ele.
