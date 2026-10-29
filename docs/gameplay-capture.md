# Captura de gameplay

Abra **Configurações → Complementos → GPU Screen Recorder** e clique em **Instalar componente de captura**. O PeliGames baixa um pacote portátil com o motor GPU Screen Recorder e suas bibliotecas, verifica o SHA-256, testa os controles IPC e instala somente os executáveis de gravação e seu auxiliar. Não é necessário abrir a interface de outro gravador nem instalar um pacote pelo terminal.

O componente gerenciado tem preferência sobre o gravador nativo do sistema. Se não houver componente gerenciado, uma instalação nativa compatível pode ser utilizada. A versão Flatpak do gravador ainda não é iniciada por esta integração.

**Reparar componente** baixa novamente a versão compatível fixada pelo launcher. Se o download estiver corrompido ou a preparação falhar, a instalação anterior permanece disponível. **Remover componente** pede confirmação e remove apenas o runtime do PeliGames; vídeos, configurações e programas instalados no sistema são preservados. Encerre a captura antes de instalar, reparar ou remover.

O pacote portátil x86_64 é mantido por [pkgforge-dev](https://github.com/pkgforge-dev/gpu-screen-recorder-AppImage), listado entre as distribuições comunitárias na documentação do [GPU Screen Recorder](https://git.dec05eba.com/gpu-screen-recorder/about/). Esta integração fixa a versão 6.1.4 e não executa o instalador gráfico nem hooks de atualização do pacote. O código-fonte do motor e o texto da licença GPL-3.0 são identificados na pasta do componente. Os drivers da GPU e os serviços de captura do desktop continuam sendo fornecidos pelo sistema.

## Painéis de captura

Nos detalhes do jogo, a aba **Capturas** organiza os controles em quatro subabas:

- **Configurações:** presets de qualidade, vídeo, fonte de captura e áudio.
- **Gravação e replay:** iniciar e encerrar sessões, duração do replay e controles integrados ao overlay.
- **Atalhos:** atribuição das teclas de gravação e replay.
- **Vídeos:** arquivos salvos deste jogo, reprodução e acesso à pasta.

Trocar de subaba não interrompe uma captura. Os ajustes de vídeo e áudio ficam bloqueados enquanto ela estiver ativa. Durante a inicialização do replay, o botão continua marcado e permite cancelar com outro clique. **Cancelar captura** também interrompe a espera de autorização e libera uma nova tentativa.

### Presets de qualidade

| Preset | Limite de resolução | FPS | Qualidade constante (QP) |
| --- | --- | --- | --- |
| Baixa qualidade | 480p | 30 | Média |
| Padrão | 720p | 60 | Alta |
| Alta qualidade | 1080p | 60 | Muito alta |

Os presets usam qualidade constante (QP) e codec automático conforme o suporte da GPU, sem exigir CBR nem fixar um codec por fabricante. O bitrate e o tamanho dos arquivos variam conforme o conteúdo; não há uma estimativa fixa de espaço por minuto. Preservam o contêiner, a fonte e o áudio escolhidos. **Personalizado** permite ajustar os controles sem aplicar outro preset; editar os valores de qualidade também seleciona esse modo. Preferências existentes não são substituídas ao abrir o painel.

## Escolher a fonte

- **Portal:** em Wayland, o desktop solicita a escolha e a autorização de uma janela ou tela. Usa PipeWire e xdg-desktop-portal, sem root. A captura é SDR.
- **Direta:** grava o monitor selecionado inteiro, incluindo outras janelas e o desktop. Em Wayland, o auxiliar KMS do GPU Screen Recorder pode precisar de autorização de administrador. Quando necessário, o próprio gravador aciona o pedido de senha do sistema. O PeliGames não recebe nem armazena senhas e nunca executa sua interface como root. Se o auxiliar já tiver a permissão necessária, a autenticação pode ser dispensada pelo sistema.

A autorização depende da instalação do GPU Screen Recorder e das políticas da distribuição. Cancelar a seleção do portal ou a autenticação pode impedir a inicialização; a mensagem do gravador aparece no painel.

## Gravação e replay

O padrão inicial é **720p, 60 FPS, MKV e qualidade constante (QP) alta**, com áudio automático do jogo e microfone desligado. O codec automático prioriza AV1, depois HEVC e H.264, conforme o suporte confirmado da GPU. Os ajustes, incluindo a duração do replay, são salvos por jogo e restaurados ao reabrir o launcher. Preferências já salvas são preservadas.

**Gravar sessão** inicia um vídeo SDR com FPS selecionável entre 24 e 240. A resolução pode acompanhar a fonte ou ser limitada a 480p, 720p, 1080p, 1440p ou 4K, preservando a proporção da imagem. Escolha o contêiner **MP4**, **MKV** ou **WebM** e, separadamente, o codec **H.264**, **HEVC (H.265)**, **AV1** ou **VP9**. Os codecs disponíveis são confirmados pelo comando `--info` do gravador, usando o driver da GPU de captura. Opções sem suporte ficam desabilitadas, e o backend verifica novamente antes de iniciar. WebM aceita AV1 ou VP9; H.264 e HEVC exigem MP4 ou MKV. A integração não troca silenciosamente para codificação por CPU. O formato escolhido também vale para replays. **Parar e salvar gravação** finaliza o arquivo.

**Replay instantâneo** mantém em memória os últimos segundos configurados, com opções de 1, 3 e 5 minutos ou duração personalizada de até 15 minutos. **Salvar replay** grava esse trecho em disco. Com replay ativo, também é possível iniciar e parar uma gravação de sessão, compartilhando o mesmo codificador.

O controle de qualidade oferece bitrate constante (CBR), qualidade constante (QP) e bitrate variável (VBR). Em CBR, escolha de 1 a 200 Mb/s; por exemplo, 5 Mb/s equivalem a aproximadamente 37,5 MB por minuto, além do áudio. CBR e VBR continuam disponíveis nos ajustes manuais, conforme o suporte do codificador e driver. Em QP e VBR, escolha qualidade média, alta, muito alta ou ultra; o tamanho depende do conteúdo. FPS e qualidade maiores podem aumentar a carga da GPU. O buffer só fica ativo após uma ação explícita; não inicia automaticamente com o launcher ou o jogo.

Em **Fonte de áudio**, escolha a saída padrão, um dispositivo específico ou um aplicativo. A captura de uma saída inclui os aplicativos e chamadas enviados a ela. Aplicativos só aparecem enquanto reproduzem som: abra o seletor depois que o jogo começar a tocar. A lista é atualizada automaticamente ao abrir o seletor de áudio ou microfone. O microfone tem um seletor de entrada próprio e começa desligado. Quando ambas as fontes estão ligadas, são misturadas na mesma faixa.

Resolução, FPS, qualidade, formato, codec e áudio também podem ser configurados no overlay do jogo. Encerre a captura para alterar essas opções; o jogo pode continuar aberto. Só uma sessão de captura do PeliGames pode estar ativa. Fechar os detalhes do jogo mantém a captura; o indicador na barra de título permite encerrá-la. Ao fechar o jogo identificado, a captura é encerrada automaticamente: uma gravação em andamento é finalizada e o buffer de replay não salvo é descartado. Os clipes já salvos permanecem na pasta. A captura iniciada antes de abrir o jogo passa a acompanhar essa execução quando ela é identificada. Ao sair do launcher, o processo recebe um pedido de encerramento para finalizar os arquivos. Processos de outros gravadores não são controlados.

## Clipes

Os arquivos ficam na pasta de vídeos do usuário, dentro de `PeliGames`, em uma subpasta identificada por jogo. O painel lista até 200 vídeos recentes e permite reproduzi-los ou abrir a pasta. Nenhum vídeo é enviado automaticamente.

A prévia no navegador apenas simula estes controles; ela não solicita permissões, captura áudio/tela ou cria arquivos reais. Os atalhos configurados atuam somente no jogo com o Overlay PeliGames ativo; não há registro global de teclas no desktop.

Referência: [GPU Screen Recorder](https://git.dec05eba.com/gpu-screen-recorder/about/).

## Controles dentro do jogo

Na aba **Capturas**, escolha fonte, áudio e qualidade. Os ajustes são vinculados automaticamente ao jogo durante a sessão do launcher, sem começar a gravar. Ative o Overlay PeliGames na aba Desempenho: os controles de captura ficam integrados ao mesmo painel Vulkan usado pelo MAKO, com os atalhos configurados na aba Atalhos.

Ative o **Overlay PeliGames** na aba **Desempenho** e abra o jogo. Use o atalho configurado do overlay para abrir o painel: **Gravar sessão**, **Iniciar replay**, **Salvar replay**, **Parar e salvar gravação** e **Encerrar captura**. Durante um replay, a gravação de sessão usa o mesmo codificador. A captura pelo portal continua pedindo seleção pelo sistema; a direta pode pedir autorização do auxiliar do gravador.

Mantenha o launcher aberto. O painel compartilha o estado com a aba Capturas e o indicador da barra de título. Cada processo do jogo carrega seus próprios ajustes de captura salvos. Abrir os detalhes de outro jogo no launcher não muda o perfil usado pelo overlay. Se houver uma captura em outro jogo, os controles ficam indisponíveis neste painel; gerencie essa captura no launcher. Ocultar o overlay mantém a captura; sair do launcher a encerra. Na prévia de demonstração, nenhum vídeo é capturado.

O painel cresce conforme a resolução da tela e as ferramentas disponíveis, com controles distribuídos pela largura e rolagem apenas quando o conteúdo excede a altura disponível. Ele continua podendo ser arrastado dentro da imagem do jogo.

O painel dentro do jogo segue os requisitos do overlay Vulkan do PeliGames. A ponte de captura desta versão atende jogos com acesso ao mesmo sistema de arquivos do launcher; a Steam Flatpak ainda não recebe essa ponte.

### Ajustar pelo overlay

O painel de captura dentro do jogo também oferece formato do vídeo, saída ou aplicativo de áudio, entrada do microfone com atualização automática da lista ao abrir cada seletor. Abra a aba Capturas do jogo no launcher para vincular os ajustes automaticamente. As escolhas do overlay são usadas na próxima gravação ou replay. Encerre a captura atual antes de mudar essas opções; o jogo pode continuar aberto. Os ajustes feitos no overlay são recuperados ao voltar à aba Capturas durante a mesma sessão do launcher.

### Codec e GPU

O launcher e o overlay mostram a GPU identificada e os codecs de hardware disponíveis. O modo **Automático** prioriza H.264 em MP4/MKV e AV1 em WebM, escolhendo somente um codec compatível confirmado. Abrir o seletor de codec no launcher atualiza a detecção; no overlay, ela é carregada ao abrir o painel e atualizada junto das fontes de áudio. Se a detecção falhar, os codecs específicos não são habilitados e iniciar a captura retorna um erro.

### NVIDIA

O mesmo componente de captura atende AMD, Intel e NVIDIA. Em NVIDIA, o GPU Screen Recorder usa o NVENC e o painel identifica esse codificador junto à GPU. H.264, HEVC e AV1 são liberados conforme a detecção do gravador; nenhuma geração de placa recebe AV1 apenas pelo nome do modelo. Sem codecs de hardware confirmados, a gravação é bloqueada com orientação para verificar o driver e as bibliotecas CUDA/NVENC da NVIDIA.

O pacote portátil gerenciado inclui o FFmpeg com negociação da API NVENC em tempo de execução, mas não instala nem substitui o driver NVIDIA do sistema. Compatibilidade de captura e codificação depende da placa e do driver. A captura pelo portal continua limitada a sessões Wayland; em X11, use a captura direta.

### Áudio automático do jogo

A opção **Áudio deste jogo · automático** identifica, ao iniciar a captura, os fluxos de áudio associados ao jogo por processo, pasta ou AppID da Steam. Funciona com jogos nativos e Wine/Proton, incluindo múltiplas fontes do mesmo jogo. O jogo precisa estar aberto e ter criado uma fonte de áudio.

A consulta usa `pw-dump`, das ferramentas do PipeWire, sem alterar saídas nem volumes. Se a ferramenta não estiver disponível, se o processo não puder ser identificado ou se outro aplicativo usar o mesmo nome de áudio, o painel informa o motivo e permite escolher a fonte manualmente. Não há substituição automática pelo áudio de todo o sistema. As preferências de áudio já salvas continuam válidas.

A seleção é conferida no início da captura e corresponde ao jogo preparado no launcher; escolher outra janela no portal não troca o áudio. Durante a sessão, o backend acompanha os aplicativos pelos nomes resolvidos. Evite abrir outro aplicativo Wine com o mesmo nome enquanto grava. Para mudar o jogo ou a fonte, encerre a captura e inicie outra.

### Atalhos de captura

Na aba **Capturas → Atalhos de captura**, clique no controle da ação e pressione a tecla ou combinação desejada. É possível atribuir **Iniciar ou parar gravação**, **Ligar ou desligar replay** e **Salvar clipe do replay**. Use **Remover** para deixar uma ação sem atalho ou **Padrão** para recuperar a combinação original daquela ação; **Esc** cancela a atribuição. As preferências são compartilhadas entre os jogos, salvas automaticamente.

Ative o Overlay PeliGames e abra a aba Capturas do jogo. As teclas funcionam com o jogo em foco, mesmo com o painel do overlay fechado; não são atalhos globais do desktop. Reabra o jogo após alterar uma combinação. O launcher verifica duplicações e conflitos com o atalho do overlay, do MangoHUD e da busca da biblioteca.

Parar a gravação salva o arquivo e mantém um replay que já esteja ativo. Encerre a gravação antes de desligar o replay. Salvar um clipe exige que o replay esteja ligado. Atribuir uma tecla não inicia a captura. Ao usar o atalho, a primeira gravação ainda pode abrir a seleção de janela do portal ou a autorização da captura direta.

### Redução de ruído do microfone

Em **Capturas → Configurações**, ative **Microfone** para escolher a entrada e, se desejar, **Reduzir ruído do microfone (RNNoise)**. A opção começa desligada e também está disponível no painel do overlay. Usa o mesmo filtro RNNoise das chamadas do Matrix, somente no microfone; o áudio do jogo permanece independente. Encerre a gravação ou replay antes de alterar o filtro.

O filtro requer `pactl`, `parec` e `pacat`, fornecidos pelos pacotes de ferramentas PulseAudio da distribuição, com PulseAudio ou PipeWire compatível com PulseAudio. Durante a captura, o launcher cria uma saída virtual temporária para entregar o microfone filtrado ao gravador, sem mudar os dispositivos padrão. Ao parar, cancelar ou ocorrer uma falha, encerra os processos do filtro e remove essa saída. Se faltar alguma ferramenta, informa o erro; desligue a redução para usar a entrada diretamente.

### Pasta das gravações

Em **Capturas → Configurações → Pasta de gravações e clipes**, use **Escolher pasta** para mudar o destino padrão de todos os jogos. O launcher salva a preferência e continua separando cada jogo em sua própria subpasta. **Restaurar padrão** volta à pasta `PeliGames` dentro de Vídeos do usuário. Encerre gravações e replay antes de trocar o destino. A mudança vale para os próximos vídeos; os arquivos anteriores permanecem na pasta original e podem ser acessados por ela.

### Galeria e reprodução de vídeos

A aba **Capturas → Vídeos** mostra as gravações do jogo em cards com miniatura, título, duração, resolução, FPS, codec, data e tamanho. As informações vêm do arquivo gravado. Miniaturas estáticas são preparadas sob demanda, uma por vez, com baixa prioridade, e guardadas em cache. A geração depende de `ffmpeg` e `ffprobe` no sistema; sem eles, os vídeos continuam acessíveis com uma indicação de informações indisponíveis. Não há reprodução ao passar o mouse.

Clique na miniatura para abrir o arquivo original no reprodutor padrão do sistema. A reprodução depende dos formatos e codecs aceitos pelo aplicativo escolhido. O PeliGames não inclui um player próprio nem converte ou copia o vídeo para reproduzi-lo. As miniaturas e informações da galeria são geradas separadamente pelo FFmpeg/FFprobe e continuam disponíveis para os formatos que essas ferramentas conseguem ler. **Editar título** renomeia o arquivo na pasta e atualiza o título da galeria, mantendo a extensão e o conteúdo do vídeo. Nomes já usados na mesma pasta são recusados, sem sobrescrever arquivos. Encerre gravações e replay antes de renomear vídeos. Miniaturas podem ser preparadas com o jogo aberto; durante uma captura, a geração fica adiada até ela terminar. Use **Atualizar vídeos** para recarregar a lista.

### Replay com duração personalizada e gravação manual

Em **Capturas → Gravação e replay → Duração do replay**, escolha 1, 3 ou 5 minutos, ou **Personalizado** para informar de 1 a 900 segundos no campo superior, que passa a ser editável. Confirme com Enter ou ao sair do campo. O painel estima a memória do vídeo conforme a duração e o bitrate. Encerre o replay antes de mudar a duração.

**Gravar sessão** inicia uma gravação manual. Use **Parar e salvar gravação**, no launcher ou overlay, para decidir quando terminar. A duração configurada para o replay não limita a gravação da sessão. O indicador na barra de título também permite encerrar a captura.
