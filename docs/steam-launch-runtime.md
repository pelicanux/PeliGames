# Inicialização persistente de jogos Steam

O PeliGames fornece um auxiliar em Rust, `pelirun`, separado da interface.
Uma linha fixa nas opções de inicialização chama esse auxiliar e entrega a ele o
comando completo da Steam. A cada início do jogo, ele lê o JSON atualizado e
executa os wrappers, as variáveis e os argumentos configurados no launcher.
O PeliGames pode estar fechado durante essa execução.

## Configuração pelo usuário

1. Salve as opções desejadas nos detalhes do jogo.
2. Em **Configurar na Steam**, copie a linha gerada.
3. Cole em **Propriedades → Geral → Opções de inicialização**, preservando seus
   argumentos adicionais. Feche a tela de propriedades para gravar.
4. Use **Já adicionei · verificar** no PeliGames.

A linha tem este formato, com o caminho real daquela instalação:

```text
'/caminho/Steam/config/peligames-launcher/pelirun' --app 203160 -- %command%
```

Esse vínculo é feito uma vez por jogo e conta Steam. As próximas alterações de
MangoHUD, variáveis, argumentos, wrappers e seleção do componente MAKO atualizam
o JSON, sem editar novamente a linha e sem reiniciar a Steam. Uma cópia do
auxiliar atende a todos os perfis daquela instalação.

Ao migrar um comando antigo, a aplicação automática reconhece o script anterior
e as linhas geradas que consegue confirmar. Se encontrar um wrapper antigo que
não pode remover com segurança, preserva a linha e orienta a substituição manual
uma vez, evitando carregar duas cópias do gerador de quadros.

A alternativa de aplicação automática continua disponível e encerra a Steam
normalmente antes de editar seus arquivos. Ela serve para estabelecer o vínculo
ou mudar o Proton; não é necessária para atualizar perfis já vinculados.

## Próxima execução e alterações durante o jogo

| Alteração | Quando passa a valer |
| --- | --- |
| Ativar/remover um wrapper, camada Vulkan ou variável de inicialização | Ao reabrir o jogo |
| Instalar/reparar os arquivos de um mod | Segue o instalador e os backups existentes; o jogo precisa recarregar os arquivos |
| Ajustes do MAKO com recarga de configuração | Seguem a recarga oferecida pelo MAKO |
| Opções do MangoHUD com recarga suportada | Seguem o comportamento do MangoHUD |
| Versão do Proton | Continua sob controle de **Compatibilidade**, na Steam |

O auxiliar mantém o comando original da Steam, incluindo seu runtime e Proton.
Ele não muda o Proton selecionado pela Steam durante a partida. O mini painel
experimental e a camada Vulkan são componentes separados desse auxiliar.

## Arquivos e isolamento

Dentro de `Steam/config/peligames-launcher/`:

- `pelirun`: executável estável, incorporado ao pacote do PeliGames;
- `profiles/<AppID>.json`: comando com marcador, alterações do ambiente,
  argumentos adicionais e informações para atualizar o perfil.

As instalações nativa e Flatpak usam seus próprios diretórios. Os requisitos de
extensão e acesso aos perfis/DLL do MAKO no Flatpak continuam sendo verificados
pelo fluxo de preparação existente. O auxiliar não concede acesso ao host nem
contorna o isolamento do Flatpak. O funcionamento completo dentro de cada
runtime ainda precisa de teste real.

O JSON é substituído por renomeação atômica e usa permissões `0600`; diretórios
novos usam `0700`. O leitor verifica versão, AppID, tamanho, proprietário e tipo
do arquivo. O comando original é tratado como uma lista de argumentos, inclusive
caminhos com espaços. O auxiliar não usa `sh -c` para interpretar o perfil.
Variáveis desabilitadas podem ser removidas explicitamente do ambiente herdado.
Outras variáveis da Steam são mantidas.

O estado dos mods continua pertencendo ao gerenciador e aos backups existentes.
O auxiliar recebe suas exigências de inicialização; não replica o instalador de
mods nem modifica a conta Steam a cada execução.

Na redefinição, somente `profiles/` entra no mesmo registro de recuperação das
preferências. O executável permanece acessível à linha já salva na Steam. Sem
um JSON, ele executa o comando original; um perfil inválido gera um erro claro.
Se a redefinição for restaurada, os perfis anteriores também são restaurados.

## Compilação e verificação

O alvo principal Linux incorpora uma compilação estática do auxiliar para não
depender da versão de glibc do runtime da Steam. O `build.rs` compila somente o
pequeno pacote `src-tauri/launch-runtime`, em diretório separado. Ele exige o alvo
Rust `x86_64-unknown-linux-musl` (ou `aarch64-unknown-linux-musl` em ARM64) e suas
dependências já baixadas. É possível fornecer um executável estático previamente
construído pela variável `PELIGAMES_LAUNCH_HELPER`; o ELF é validado antes de ser
incorporado.

Foram adicionados testes para o contrato de perfis, atualização atômica,
preservação do comando/argumentos da Steam, remoção de variáveis, rejeição de
perfis inválidos e execução normal após redefinição. Os testes de integração do
auxiliar usam apenas comandos pequenos em pastas temporárias.

Validação realizada em 19/10/2026:

- Compilação da interface de produção e dos binários Linux, com o `pelirun`
  estático incorporado; geração do AppImage.
- Seis testes do auxiliar no alvo musl, 39 testes da integração Steam e oito
  testes de redefinição/recuperação passaram. Os diagnósticos locais ignorados
  por padrão foram executados separadamente para o jogo selecionado.
- Os dois binários e o AppImage passaram em `--verify-ui`, com os recursos da
  interface de produção incorporados.
- O auxiliar executou dentro do runtime Steam Linux. No Tomb Raider (203160),
  duas execuções receberam os perfis com MangoHUD ativado e desativado, mantendo
  o mesmo processo Steam e a mesma linha de inicialização. Na segunda execução,
  o jogo recebeu `MANGOHUD=0` e não carregou `libMangoHud`.
- A verificação Rust confirmou a linha salva na conta ativa e o Proton
  selecionado. Ao concluir, o perfil anterior de monitoramento foi restaurado
  e os marcadores temporários do teste foram removidos.

A validação real acima cobre a Steam nativa. O teste completo com Steam Flatpak
continua pendente; os testes de isolamento das instalações não substituem essa
execução dentro do sandbox.

## Referências da arquitetura

- [Goverlay: bgmod](https://github.com/benjamimgois/goverlay/blob/main/bgmod.lpr):
  referência do fluxo de executável fixo e configuração externa. A implementação
  do PeliGames é em Rust e usa seu próprio contrato; não incorpora código Pascal.
- [Valve: runtimes Steam Linux](https://github.com/ValveSoftware/steam-runtime/blob/master/doc/reporting-steamlinuxruntime-bugs.md).
- [Flatpak: permissões de sandbox](https://docs.flatpak.org/en/latest/sandbox-permissions.html).

## Perfis universais do PeliRun (implementação local)

O helper também aceita `--profile pg-a73c92e4 -- comando [argumentos]`.
O identificador tem `pg-` e oito caracteres hexadecimais. Ele é gerado pelo
PeliGames, com verificação de colisão e bloqueio entre processos; não é um ID da
Steam, da Epic ou do Heroic. Renomear o jogo e trocar o Proton não altera o ID.
A associação automática usa a pasta canônica da instalação (resolvendo links
simbólicos), nunca apenas o título. Outra instalação em outra pasta recebe
outro perfil. Um perfil existente não é associado automaticamente a uma pasta
movida: a confirmação dessa associação exige um fluxo separado.

Em **detalhes do jogo → Opções → PeliRun → Usar em outro launcher**, a aplicação
exporta o mesmo perfil nos formatos:

- **Steam:** linha completa, com `%command%`.
- **Heroic:** executável do wrapper e argumentos em campos separados, sem
  `%command%`.
- **Lutris:** prefixo de comando completo, sem `%command%`.
- **Outro:** prefixo genérico para qualquer launcher que forneça o comando
  original ao wrapper. Um campo que apenas acrescenta argumentos ao jogo não
  pode executar o PeliRun.

Os campos têm botões de copiar e exibem o identificador curto. Não são gerados
executáveis por jogo. O helper compartilhado fica em
`$XDG_DATA_HOME/pelirun/pelirun` (normalmente
`~/.local/share/pelirun/pelirun`); os perfis ficam em `profiles/pg-xxxxxxxx.json`.
O arquivo v2 guarda comando, ambiente, argumentos e origem das preferências,
independentemente de qual launcher forneceu o comando de execução.

Ao salvar configurações de jogos externos, PeliGames/Pelinstall ou Nexus, um
perfil universal já existente é atualizado. A configuração global do MangoHUD
também regenera esses perfis. A exportação cria o perfil quando necessário.
Instalação/remoção de mods continua seguindo os gerenciadores existentes e
seus requisitos de inicialização entram no perfil, sem duplicar os backups.
Não há alteração automática da versão do Proton no launcher de destino.

### Compatibilidade com os comandos já existentes

Na Steam nativa, ao preparar o perfil, o antigo `profiles/<AppID>.json` passa a
ser uma referência privada ao arquivo universal. A chamada antiga `--app`
continua funcionando e lê o mesmo perfil da chamada `--profile`, sem cópias de
configuração divergentes. O verificador da Steam reconhece ambos os formatos,
e a migração evita encadear duas chamadas do helper.

A Steam Flatpak mantém o perfil legado dentro do seu próprio diretório; não se
migra automaticamente para um caminho que sua sandbox pode não acessar. A
exportação universal informa a necessidade de acesso ao executável, ao perfil
e aos componentes utilizados. Nenhuma permissão Flatpak é concedida pelo fluxo.

O perfil usa leitura privada com limite de tamanho e rejeição de symlinks,
validação do ID e substituição atômica com fsync. O helper aceita os marcadores
antigo e novo, preserva o comando/argumentos originais e não interpreta shell.
A ausência de perfil após redefinição preserva a execução original. Arquivos
inválidos geram erro, sem executar um comando não validado. Os perfis universais
entram no registro de recuperação da redefinição; o binário fixo é preservado.
As restrições da build pública também são aplicadas aos perfis universais.

### Referências de configuração dos launchers

- [Heroic: configurações do wrapper](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/public/locales/en/translation.json).
- [Lutris: opção Command prefix](https://github.com/lutris/lutris/blob/master/lutris/sysoptions.py).

Esta mudança ainda não foi empacotada nem testada graficamente em Heroic,
Lutris ou runtimes Flatpak. Os testes isolados executam o helper real com
comandos pequenos e arquivos temporários, sem iniciar jogos ou modificar
configurações pessoais.

### Encerramento e confirmação do reinício

A aplicação automática registra as etapas em `launcher.log` e emite
`steam-settings-progress` para o botão: verificação, encerramento, finalização,
aplicação e abertura. Requisitantes `steam -shutdown`, processos zumbis e o
serviço auxiliar do runtime não são tratados como um cliente que mantém o VDF
aberto.

Com o cliente ativo, primeiro solicita `-shutdown` e aguarda aproximadamente
oito segundos. Se não encerrar, verifica novamente a existência de jogos e
pode enviar SIGTERM ao cliente principal confirmado; depois de mais quatro
segundos, SIGKILL é o último recurso, com mais três segundos de espera. Só
processos do usuário, da instalação selecionada, presentes no início do fluxo
e com executável conhecido podem receber sinais. A identidade é revalidada
com PID/start-time e pidfd, evitando atingir um PID reutilizado. Não são usados
`pkill`, sinais para grupos nem encerramento de jogos. Um jogo em execução,
um processo não verificável ou uma nova instância interrompe a operação e
preserva as pendências.

Os arquivos só são alterados após confirmar o encerramento. A abertura é
observada por até 12 segundos; criar o subprocesso inicializador não basta
para informar sucesso. Se as configurações forem aplicadas mas a Steam não
voltar, a interface informa essa situação e atualiza o estado das pendências,
evita exigir reaplicação e orienta abrir o cliente manualmente.

Esse tratamento foi verificado com processos e relógio simulados, além de
checagem Rust/TypeScript; não foi executado um reinício real da Steam durante
o desenvolvimento da correção.

### Controles MAKO pelo launcher durante a partida

A aba de geração de quadros distingue execução em andamento de operações que
bloqueiam a interface. Com o jogo aberto, multiplicador, adaptativo, FPS alvo,
escala de movimento e modelo leve podem ser enviados ao perfil ativo por
`update_frame_generation_live`. O salvamento automático envia esses controles após 450 ms sem novas alterações. DLL,
ativação, versão do runtime e carregamento do overlay continuam protegidos.

Essa operação não salva configurações de execução gerais, não prepara um novo
wrapper e não altera opções da Steam. Ela modifica apenas seis chaves de
recarga do TOML existente, atualiza o JSON de preferências e a representação
`live.json` lida pelo painel interno. Preserva a pausa da geração e outras
configurações do documento. Formatos inesperados ou chaves duplicadas são
recusados. A recarga depende de o jogo ter sido iniciado com MAKO; salvar um
perfil não injeta MAKO em um jogo que começou sem ele.


### Salvamento automático da geração de quadros

Os controles do perfil dispensam Salvar/Aplicar. A fila agrupa alterações por
450 ms, serializa gravações e envia uma alteração pendente ao sair da aba.
Carregar a tela não dispara uma gravação. Erros ficam no painel com opção de
tentar novamente; alterações comuns não geram notificações repetidas. A
configuração inicial em um launcher externo continua sendo oferecida quando
uma ativação exige vincular o wrapper.

A confirmação manual de que outros geradores estão desligados foi removida,
inclusive do contrato de preferências e da validação Rust. Perfis antigos que
contenham esse campo continuam legíveis por Serde. As verificações efetivas
de conflitos com configuração local de FG, HDR e outras camadas permanecem.
O programa não presume conseguir detectar todas as opções internas de um jogo.
