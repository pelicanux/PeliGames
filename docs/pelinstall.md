# Pelinstall

Assistente externo do PeliGames para instalar um arquivo Windows local ou adicionar um executável existente à biblioteca. Usa o mesmo frontend Tauri, componentes de vidro/neon, paleta, catálogo de Protons, UMU e biblioteca Rust do launcher. O binário é separado; os serviços de instalação e execução são compartilhados.

## Testar pelo gerenciador de arquivos

1. Registre o aplicativo com `bash scripts/register-pelinstall.sh`. Em um `.exe` ou `.msi`, use **Abrir com → Outro aplicativo**, procure **Pelinstall** e selecione a entrada na lista.
2. Na tela inicial, escolha **Instalar** ou **Adicionar**. Adicionar requer um `.exe` existente; `.msi` permanece disponível no modo de instalação. Depois escolha **Padrão** ou **Escolher**. No padrão, o destino final é `~/Games/PeliGames/<nome>`; um destino escolhido é usado exatamente como informado.
3. Clique em **Próximo**, selecione um Proton instalado e avance.
4. Informe o nome e, se desejar, marque **Criar atalho na área de trabalho**. Clique em **Instalar** ou **Adicionar**, conforme o modo escolhido.
5. Em Instalar, complete as telas do instalador Windows. O módulo espera o Proton finalizar e registra os executáveis encontrados no prefixo na biblioteca do PeliGames. Em Adicionar, salva diretamente o executável, o prefixo e o Proton na biblioteca, sem executar o programa ou um instalador; o prefixo é preparado pelo Proton na primeira execução.
6. Se houver vários executáveis e o atalho estiver habilitado, escolha qual receberá o atalho. O encerramento do instalador, sozinho, não garante que houve uma instalação: sem executáveis encontrados, a tela explica que é necessário conferir a instalação no launcher.

Também é possível executar:

```sh
./Release/Pelinstall "/caminho/instalador.exe"
./Release/PeliGames --install "/caminho/instalador.exe"
```

`Release/Pelinstall.desktop` fornece os tipos MIME de executáveis Windows. O script de registro copia essa entrada para a pasta de aplicativos do usuário e atualiza os catálogos do desktop, sem definir uma associação padrão. O caminho da entrada aponta para esta pasta Release; após mover o projeto, recompile e registre novamente. Selecionar a entrada na lista evita depender do campo de comando manual do Dolphin, que apresentou erro de comando vazio durante o teste. O teste direto do binário também funciona sem trocar associações.

## Ícone e capa

O painel inicial mostra o ícone extraído do executável Windows, sem executar esse arquivo. Para entradas já registradas, usa o ícone do executável salvo e exibe o nome atribuído abaixo. Arquivos sem um ícone válido recebem o símbolo genérico de documento.

Ao digitar o nome na etapa final, o Pelinstall usa a mesma pesquisa automática de capas do launcher. A capa encontrada é salva na instalação ou no registro adicionado e permanece após novas varreduras. Na reparação, uma capa encontrada para um registro sem capa também é salva. Uma busca sem resultado ou uma falha de rede não impede a instalação. Duplicar mantém a pesquisa pelo título original quando o nome sugerido termina em `(cópia)`.

## Executável já registrado

Ao abrir um executável, o Pelinstall consulta somente a biblioteca própria do PeliGames. Compara o caminho real do executável instalado e do instalador original, sem deduzir identidade pelo nome. Se houver correspondência, mostra **Duplicar**, **Reparar**, **Desinstalar** e **Configurações**, em duas colunas. Configurações abre o PeliGames diretamente no painel do registro selecionado. Havendo vários registros, permite escolher qual será utilizado. Sem correspondência, mantém **Instalar** e **Adicionar**.

Duplicar inicia uma nova configuração e continua permitindo registros repetidos. Reparar mantém o executável aberto pelo usuário e o prefixo do registro selecionado. Vai diretamente à escolha do Proton, seguida do nome preenchido para edição opcional, e mantém a identidade do registro. Se o Proton e o prefixo forem os mesmos, salva as alterações e conclui diretamente, sem executar o arquivo Windows. Ao trocar o Proton (ou o prefixo via backend), executa `umu-run wineboot -u` com o prefixo e runner selecionados; só salva as alterações após a atualização terminar com sucesso e verificar `drive_c` e `system.reg`. A operação não abre o executável nem aguarda o jogo fechar. Não solicita outro instalador nem exibe a etapa de destino; Voltar na etapa de Proton retorna ao menu inicial. Na etapa de nome, verifica os atalhos do registro na área de trabalho, no diretório XDG de aplicativos (`~/.local/share/applications` por padrão) e em `~/Applications`. Se encontrar, oferece **Remover atalho**; caso contrário, **Criar atalho na área de trabalho**. A escolha é opcional e só é aplicada após a reparação concluir sem erro de registro. A remoção afeta somente arquivos `.desktop` criados pelo PeliGames para essa entrada, incluindo cópias nos diretórios pesquisados; links simbólicos e atalhos de outros registros são preservados. Falhas e cancelamentos não salvam as alterações do registro; a atualização pode ter modificado arquivos do prefixo antes de falhar.

Desinstalar sempre solicita confirmação: remove permanentemente o prefixo completo, seus jogos, programas, configurações e saves, além de todos os registros que usam esse prefixo. Os atalhos de todas as entradas removidas também são excluídos da área de trabalho e das pastas padrão de aplicativos pesquisadas. Executáveis externos ao prefixo e atalhos de outros registros são preservados.

## Atalhos e execução em segundo plano

Os atalhos `.desktop` executam `PeliGames --launch-game <entrada registrada>`. O processo lê o prefixo, Proton, executável e configurações avançadas da biblioteca, inicia a sessão e monitora seus processos sem criar a janela do launcher.

Uma falha ao iniciar, saída com erro, encerramento muito rápido sem confirmação de abertura ou padrões de falha conhecidos nos logs abre uma janela pequena com o diagnóstico, caminho do log e **Abrir PeliGames**. Esse botão inicia o launcher com a entrada selecionada e o relatório da tentativa. Uma saída rápida pode ser legítima; a mensagem informa essa possibilidade. Não há garantia de detectar todas as falhas gráficas de um aplicativo: o monitor combina estado do processo, código de saída e indícios nos logs, sem tratar todo `fixme` ou `err` do Wine como travamento.

O comando de execução ativa `PROTON_LOG=1` e direciona os logs para a pasta da instalação. A análise considera o log UMU e arquivos de log do Proton atualizados nesta sessão; ignora logs antigos. Os relatórios ficam em `~/.config/peligames/launch-reports/<hash-da-entrada>.json` e são removidos após uma execução posterior bem-sucedida da mesma entrada.

O prefixo e os logs permanecem diretamente em `<destino>/prefix` e `<destino>/logs`. A área de trabalho segue a configuração XDG do sistema. Os atalhos usam nomes de arquivo únicos, sem sobrescrever outros atalhos. Um destino já registrado não é reutilizado para uma nova instalação.

## Integração e proteção de dados

- Biblioteca: mesmos `installed-destinations.json` e `installation.json` usados pelo launcher; uma trava entre processos protege leitura e gravação dos registros. O grid atualiza os jogos próprios ao recuperar o foco.
- Prefixos: uma trava por prefixo impede iniciar duas sessões PeliGames/Pelinstall simultâneas nele, além da verificação dos processos Wine existentes.
- Arquivos: validação de caminho, assinatura `.exe`/`.msi` e runner antes de executar; nomes e caminhos são argumentos literais, sem shell.
- Navegação: **Próximo** só habilita após a escolha exigida. Cancelar o seletor de pasta não altera a escolha. Fechar uma configuração incompleta pede confirmação. Durante a instalação, **Cancelar instalação** encerra a sessão após confirmação; o X oferece a mesma confirmação e fecha somente após os processos terminarem. O cancelamento preserva o prefixo/logs e não registra a tentativa na biblioteca.
- Monitoramento da instalação: o backend acompanha o processo UMU e seus descendentes, incluindo processos que conservam o prefixo depois de a janela Windows fechar. O cancelamento considera apenas o grupo desta sessão e seus processos identificados; não encerra Wine/Steam globalmente. Ausência de dependências, como WebView, continua sendo um erro do programa Windows: esta alteração permite interromper a tentativa sem bloquear o Pelinstall, mas não instala dependências automaticamente.
- Janela: o painel ocupa toda a janela, sem a imagem de fundo do launcher ao redor. A barra de título permite arrastar e oferece **Minimizar** e **Fechar**, preservando a paleta e o vidro/neon. Não há maximização; o limite é 560 × 490. O assistente ajusta a altura ao conteúdo entre 260 e 490 pixels, com largura mínima de 480. Os blocos numerados das etapas foram removidos; o nome do executável está maior e mais destacado. Destino e Proton exibem somente os controles necessários; a última etapa mantém nome, atalho e informações do destino. O fechamento autorizado usa a permissão `core:window:allow-destroy`.
- A aparência é compartilhada pela origem/configuração Tauri do PeliGames. O módulo não precisa abrir o launcher para instalar.

## Compilação e validação

```sh
distrobox enter ubuntu-dev -- bash scripts/build-binaries.sh
cargo test --manifest-path tools/proton-service/Cargo.toml --offline
cargo build --manifest-path tools/proton-service/Cargo.toml --offline
python3 tools/proton-service/tests/pelinstall-smoke.py
desktop-file-validate Release/Pelinstall.desktop
./Release/PeliGames --verify-ui
./Release/Pelinstall --verify-ui
```

O script compila os dois binários com `--features custom-protocol`, usa todos os threads disponíveis e `/tmp` como destino temporário; nesta máquina `/tmp` é tmpfs. Não gera AppImage, RPM ou DEB. O recurso de produção embute HTML, JavaScript e CSS no binário: `--release` sozinho não ativa esse modo do Tauri. Uma proteção de compilação recusa release sem esse recurso; antes de substituir os binários, `--verify-ui` verifica o modo de produção e a presença dos arquivos embutidos sem precisar de servidor gráfico ou localhost.

A prévia visual está em `http://127.0.0.1:1424/preview.html?module=pelinstall`; a tela de diagnóstico, em `?module=launch-error`. A prévia não instala software pelo assistente. O teste de integração usa arquivos e processos simulados em uma pasta temporária, exercitando o backend real sem executar um instalador Windows. A validação visual cobriu destino, Proton, nome obrigatório, atalho, retorno e confirmação ao fechar. Uma instalação completa com software Windows real ainda deve ser testada pelo usuário no binário.

Validação nesta implementação: 37 testes Rust aprovados, incluindo cancelamento de processos filhos sem atingir outros programas; binários compilados no container Ubuntu com 16 threads e tmpfs; entrada `.desktop` validada; teste de integração aprovado com instalação simulada, falhas e avisos de logs, limpeza de relatórios e execução dos dois binários nativos sem servidor gráfico. Alguns ambientes de desktop podem pedir **Permitir execução** na primeira abertura de um atalho criado.

Referências técnicas: [argumentos e escapes de Desktop Entry](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html) e [configuração de janelas Tauri](https://v2.tauri.app/reference/config/). Implementação própria sobre os serviços do PeliGames.

### Entradas repetidas

Adicionar permite cadastrar o mesmo executável, com o mesmo nome e no mesmo destino/prefixo, mais de uma vez. Cada cadastro recebe um identificador independente (`peligames:<UUID>`); nomes, Proton, executável e ajustes avançados são salvos por entrada. O manifesto existente é preservado e recebe o novo cadastro. As varreduras não eliminam essas entradas. Registros anteriores que usam o caminho físico como identidade continuam funcionando.

O gerenciador de mods mantém uma apresentação separada da biblioteca principal, inclusive para entradas PeliGames; as chaves de seleção e animação incluem o contexto e o launcher. O caminho físico continua sendo usado para modificar os arquivos do jogo. A desinstalação continua apagando o prefixo inteiro e removendo todos os cadastros que o compartilham, conforme a confirmação já exibida.

### Ícones dos atalhos

Ao criar um atalho, PeliGames/Pelinstall extrai o ícone do executável registrado do jogo/programa (não do instalador). O backend Rust lê os recursos RT_GROUP_ICON e RT_ICON de arquivos PE32/PE32+, seleciona a maior imagem válida do primeiro grupo utilizável e gera um PNG preservando a transparência. Funciona com ícones bitmap e PNG embutidos, sem iniciar o EXE e sem depender de Dolphin, Wine, FFmpeg ou icoutils.

Os PNGs ficam em `$XDG_CONFIG_HOME/peligames/icons/executables/` (normalmente `~/.config/peligames/icons/executables/`), identificados pelo hash do conteúdo. O campo `Icon` do arquivo `.desktop` aponta para o PNG permanente. Se não houver ícone válido, usa o logo embutido do PeliGames; se a pasta de ícones não puder ser gravada, mantém o ícone genérico do sistema. Atalhos antigos permanecem como foram criados; novos atalhos recebem esse recurso automaticamente.

A leitura tem limites de bytes, recursos e dimensões, e arquivos incompletos ou malformados não impedem a criação do atalho. A implementação do leitor PE é própria, baseada na especificação da Microsoft; as bibliotecas Rust `ico` e `png` decodificam e convertem as imagens.

### Retorno e identificação do executável instalado

Todas as etapas de configuração têm Voltar, incluindo o destino, que retorna ao menu inicial de instalar/adicionar. Reparar continua retornando da seleção do Proton diretamente ao menu inicial. Durante uma instalação em andamento, o controle é Cancelar; a conclusão não reinicia o instalador.

A varredura mantém os executáveis descobertos e prioriza destinos de atalhos Windows nos desktops e menus Iniciar do prefixo. O leitor próprio de `.lnk` interpreta LinkInfo (ANSI/Unicode), caminho relativo/diretório de trabalho, argumentos e o bloco de ambiente. Também lê App Paths, LauncherAppPath/DesktopAppPath e DisplayIcon de desinstalação nos arquivos Wine `system.reg` e `user.reg`. DisplayIcon é apenas uma indicação mais fraca, pois pode apontar para outro componente. Entradas explicitamente adicionadas e duplicadas mantêm suas identidades.

Só recomenda caminhos existentes no drive C do prefixo, com assinatura MZ; não segue links para o desktop Linux, aceita caminhos externos ou sugere desinstaladores, redistribuíveis, atualizadores e relatórios de erro. Quando um launcher deixa um atalho antigo, pode recuperar o mesmo sufixo em uma única subpasta de versão no diretório da aplicação em Program Files. Se houver mais de uma versão correspondente, não escolhe por essa regra. A análise tem limites de tamanho, profundidade e quantidade. Nem todo instalador cria atalhos ou registra esses campos; nesses casos, permanece a seleção entre os executáveis encontrados.

A conclusão permite escolher um executável, ver a origem da indicação, criar o atalho solicitado e usar Iniciar. Argumentos encontrados no atalho são armazenados na entrada e passados individualmente, sem shell, ao lançar o executável registrado. Trocar manualmente o executável limpa esses argumentos. Iniciar usa `PeliGames --launch-game`, preservando o monitoramento e o diagnóstico de erros sem abrir a biblioteca. Não inicia automaticamente nenhum programa.

Verificação somente leitura do prefixo (não registra jogos):

```sh
tools/proton-service/target/debug/peligames-proton-service installation-targets /caminho/do/prefixo
```

Referências: [formato Shell Link](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-shllink/747629b3-b5be-452a-8101-b9a2ec49978c) e [App Paths](https://learn.microsoft.com/en-us/windows/win32/shell/app-registration).

### Identidade e execução compartilhada

O ícone extraído e o nome ficam juntos, com o nome abaixo do ícone, em todas as etapas e na conclusão. Antes de preencher Nome, mostra o nome do executável (ou o nome já registrado na biblioteca); ao digitar, acompanha o nome escolhido imediatamente.

Iniciar na conclusão usa o monitor em segundo plano do PeliGames. As sessões de jogos/programas iniciados pelo PeliGames agora publicam o identificador da entrada, estado e log em `$XDG_CONFIG_HOME/peligames/executions/`. O registro vincula a sessão ao PID e ao instante de criação do monitor; arquivos de processos mortos ou PIDs reutilizados não são tratados como jogos rodando. A pasta é privada e os arquivos são gravados atomicamente. Instaladores e atualização de prefixo não aparecem como sessões de jogo na biblioteca.

Ao abrir o launcher, ou durante a consulta periódica já existente, a interface consulta esse registro. A entrada correspondente mostra Em execução e Parar. Um pedido de parada feito em outra instância é recebido pelo monitor original e usa a mesma identificação de grupo, descendentes, prefixo e instante de criação de PID já usada pelo supervisor. Não usa `killall`, nem encerra Wine/Steam globalmente. A parada solicitada não gera a mensagem de falha de abertura do atalho. Quando termina, o monitor remove o registro e o botão volta a Iniciar. A detecção também reconhece um launcher que transfere a execução para outro EXE da mesma sessão, excluindo serviços comuns de inicialização do Wine. A biblioteca já aberta recebe um aviso local para reler os jogos quando detectar uma nova sessão compartilhada.

Os testes exercitam a consulta e a parada por um segundo processo, rejeição de identificadores inválidos, identidade do monitor/PID, registros redirecionados, transferência a outro executável e preservação de um processo alheio. O ensaio usa apenas programas simulados em diretórios temporários; não executa o EA App nem instala programas reais.

### Fechamento normal e reabertura do atalho

O monitor não transforma palavras de exceção no log, nem uma saída rápida com sucesso, em uma janela de falha. O resultado do processo/supervisor determina a falha; os logs podem complementar um erro real. A busca por palavras no log também não cancela automaticamente um programa que demora para abrir. Isso evita tratar mensagens de componentes auxiliares (como as exceções de Xalia encontradas no log do 7-Zip) como falha do aplicativo.

Abrir novamente o mesmo atalho enquanto a entrada está em execução reutiliza o estado do monitor existente, sem lançar outra sessão nem mostrar conflito de prefixo. A publicação inicial é aguardada brevemente em caso de duas aberturas simultâneas. Entradas diferentes e instalações concorrentes continuam respeitando o bloqueio do prefixo. Os testes de integração verificam abertura repetida e simultânea, saída rápida bem-sucedida, log com exceção e saída bem-sucedida, parada solicitada e erro de saída real.

A identificação do Pelinstall fica à direita em todas as etapas: ícone de 40px com o nome abaixo. Informações, campos e botões compactos ficam à esquerda, incluindo o menu inicial e os fluxos de instalar, adicionar e reparar. As notas menores ficam abaixo das duas colunas.

### Layout da conclusão

Na conclusão de instalar, adicionar ou reparar, o resumo fica à esquerda e a identidade (ícone e nome) fica à direita. A seleção de executável ocupa toda a largura abaixo desse cabeçalho. Quando Criar atalho está disponível, divide a linha com Iniciar; com apenas uma ação, o botão ocupa a largura inteira. Atalho criado/removido e caminho do log continuam abaixo.

Para conferir a composição sem instalar programas, a prévia aceita `?module=pelinstall&stage=complete`, com três executáveis simulados. Criar atalho nessa demonstração altera apenas o estado visual; Iniciar não executa programas. O modo de demonstração está limitado à prévia do navegador.

### Seleção de executável e atualização do atalho na reparação

Os seletores de registro e de executável usam o menu do launcher, com cores do tema, seleção destacada, teclado e posicionamento limitado à janela. O menu inicial mostra os nomes dos registros, sem repetir o caminho do prefixo.

Na etapa Nome da reparação é possível escolher entre os executáveis registrados no mesmo prefixo. O executável atual vem selecionado; a reparação salva a escolha na entrada original, mantendo sua identidade. A opção Atualizar atalho aplica o nome e o ícone desse executável aos atalhos existentes depois de uma reparação bem-sucedida. Atualizar e remover são mutuamente exclusivos e começam desativados. Sem atalho existente, permanece Criar atalho na área de trabalho.

A atualização preserva o caminho e as permissões de cada `.desktop`, verifica a identificação exata da entrada e não segue arquivos simbólicos nem altera atalhos de outros jogos. O comando do atalho continua abrindo o identificador da biblioteca, que passa a usar o executável selecionado. Testes cobrem a persistência da troca de executável e a atualização dos atalhos sem duplicá-los ou alterar outras entradas.

A identidade na coluna direita é centralizada na altura dos controles, com um pequeno deslocamento para baixo. Na reparação, a existência do atalho é consultada novamente na etapa Nome, ao recuperar o foco da janela e antes de salvar. Com atalho existente, mostra Remover atalho da área de trabalho; sem atalho, mostra Criar atalho na área de trabalho. Durante uma falha de consulta, essas ações permanecem desabilitadas. A prévia também consulta o estado atual, em vez de reutilizar a varredura inicial. A busca inclui o desktop configurado, o antigo padrão Desktop e os diretórios de aplicativos por usuário. Reconhece o comando exato com argumentos entre aspas ou sem aspas, sem depender do texto de comentário.

### Barra de título

A área de arrastar mantém o cursor padrão. Pelinstall usa a fonte Jersey 10 do launcher, mantendo seu ícone. O botão PeliGames, sem ícone, fica antes de minimizar e fechar, com a mesma fonte e o brilho do nome do launcher ao passar o mouse. Abre o launcher com o comando existente e preserva a janela e a etapa atual do instalador. Os controles são excluídos da área de arrastar.

### Destino inicial no drive C

Novas instalações reconhecidas pelo cabeçalho de dados do Inno Setup recebem `/DIR=C:\Games\<nome>`, mantendo o destino no drive C do prefixo selecionado. A identificação lê até 16 MiB com memória limitada; não muda os mapeamentos de unidades do Proton, não acrescenta esse argumento ao iniciar jogos ou reparar prefixos e não o envia a EXEs desconhecidos/MSI. Instaladores de outros formatos continuam escolhendo o destino conforme suas próprias regras. A unidade S pode ser um mapeamento do Proton e não significa falha por si só.

Referências: [parâmetro /DIR do Inno Setup](https://jrsoftware.org/ishelp/topic_setupcmdline.htm) e [mapeamentos de unidades do Proton](https://github.com/ValveSoftware/Proton/blob/master/proton).

### Wrappers por jogo

Em Compatibilidade Proton e Wine, junto à prioridade de DLLs, Comando do wrapper permite cadastrar executável ou caminho do script e argumentos, editar, remover e ativar cada item. Cadastros novos começam desativados. A busca por wrapper/scripts encontra o editor. O backend persiste os itens junto aos ajustes da entrada e encadeia os wrappers ativos na ordem da lista, antes de Gamescope, ferramentas de desempenho e UMU. O script recebe o comando seguinte e deve encaminhá-lo, por exemplo usando `exec "$@"` em um script shell. Scripts executados diretamente precisam ter permissão de execução; também é possível usar `/bin/bash` como executável e o caminho do script nos argumentos.

Os argumentos são separados respeitando aspas e passados individualmente; o launcher não interpreta comandos de shell nem expande variáveis. Há limites de 16 wrappers e 4096 bytes por campo. Aspas incompletas e executáveis indisponíveis produzem mensagens de erro. Testes verificam persistência, itens desativados, cadeia com GameMode, espaços, argumentos vazios/literais e execução de um wrapper simulado antes do jogo.

## Menu das capas e logs de execução

O grid e a capa selecionada compartilham as ações Iniciar, Desinstalar, Detalhes e Exibir logs. Desinstalar uma entrada PeliGames abre a confirmação existente de exclusão do prefixo e dos atalhos. Para Steam, início e desinstalação são encaminhados à Steam; jogos de outros launchers precisam ser registrados no PeliGames para execução monitorada.

Cada entrada PeliGames mantém duas execuções em `~/.config/peligames/game-logs/<hash-do-identificador>/`: `current` e `previous`. Cada registro reúne a saída padrão/erros e os arquivos de diagnóstico produzidos pelo Proton. A próxima execução move o registro atual para anterior e remove o anterior mais antigo, somente depois de obter a trava do prefixo. Instalações e reparações não substituem esses logs. A consulta atualiza a cada 1,2 segundos e continua disponível após fechar o jogo; lê no máximo 1 MiB por registro para preservar a resposta da interface.
