<p align="center">

  <img src="public/images/readme-header.svg" alt="PeliGames" width="900" />
</p>

> **0.5.2 experimental:** presets de captura com qualidade constante e codec automático, correções de fluidez no AppImage/Wayland, recuperação de capas e melhorias na organização e redefinição dos dados. Veja as [notas da 0.5.2](docs/releases/0.5.2.md). Captura, geração de quadros e overlay são experimentais e dependem do jogo, GPU, Proton e ambiente gráfico. Consulte o [guia de uso e limitações](docs/overlay.md).

<p align="center"><strong>Português</strong> · <a href="README.en.md">English</a></p>

Launcher de jogos Windows para Linux com Proton, biblioteca de jogos, Pelinstall, ferramentas Wine, integração Nexus Mods e gerenciador de mods DLSSNR-AMD.

<p align="center">
  <img src="public/images/interface-preview.webp" alt="Interface do PeliGames com biblioteca de jogos organizada por origem" width="1200" />
</p>

<!-- section-links -->
<details>
<summary><strong>Índice — links diretos das seções</strong></summary>

Para compartilhar um tópico, clique no link e copie o endereço da página, ou use “Copiar endereço do link” com o botão direito.

- [Como funciona](#como-funciona)
- [Instalação](#instala%C3%A7%C3%A3o)
  - [Instalar o AppImage](#instalar-o-appimage)
- [Mods locais do Nexus](#mods-locais-do-nexus)
- [Biblioteca unificada e geração de quadros](#biblioteca-unificada-e-gera%C3%A7%C3%A3o-de-quadros)
- [DLSS 5 Neural Rendering em placas AMD (DLSSNR)](#dlss-5-neural-rendering-em-placas-amd-dlssnr)
  - [Placas e backends](#placas-e-backends)
  - [Preparar o modelo e instalar no jogo](#preparar-o-modelo-e-instalar-no-jogo)
  - [Rede inicial e opções do launcher](#rede-inicial-e-op%C3%A7%C3%B5es-do-launcher)
  - [Escolher o modo](#escolher-o-modo)
  - [Ajustes no jogo com Mochizuki 0.0.8](#ajustes-no-jogo-com-mochizuki-008)
  - [Inicialização pela Steam e outros launchers](#inicializa%C3%A7%C3%A3o-pela-steam-e-outros-launchers)
  - [Custo de processamento](#custo-de-processamento)
  - [Quando algo não funciona](#quando-algo-n%C3%A3o-funciona)
- [Jogos suportados](#jogos-suportados)
- [Compilar a partir do código](#compilar-a-partir-do-c%C3%B3digo)
- [Créditos e licença](#cr%C3%A9ditos-e-licen%C3%A7a)
- [Agradecimentos especiais](#agradecimentos-especiais)

</details>
<!-- /section-links -->

## Como funciona

1. **Biblioteca:** o PeliGames reúne num só lugar os jogos da Steam, da Epic Games nativa para Linux, do Heroic, do Lutris, do Bottles e do Faugus, além dos que você instala por ele.
2. **Instalar jogos e programas do Windows:** escolha o instalador (`.exe` ou `.msi`), o nome, a pasta e a versão do Proton. O PeliGames prepara o prefixo, roda o instalador e encontra sozinho o executável do jogo. Se houver dúvida entre executáveis, ele pergunta.
3. **Launchers (EA app, Battle.net, Ubisoft Connect, Epic):** os jogos que você instala por dentro do launcher aparecem em **Programas**, para abrir direto. Ao adicionar um deles à biblioteca, ele ganha um perfil próprio, com prefixo, Proton e mods separados, sem duplicar os arquivos do launcher.
4. **Mods do Nexus Mods:** abra o jogo na **Biblioteca → Mods**, escolha os mods no catálogo (ou importe um arquivo) e clique em **Instalar**. O PeliGames confere os requisitos, aplica o instalador FOMOD quando existe e implanta os arquivos de acordo com as regras do jogo, as mesmas usadas pelo Vortex. Os arquivos originais substituídos com autorização são preservados para restauração pelo gerenciador. Veja os [jogos suportados](docs/supported-games.md).
5. **Problemas:** **Reportar erro**, nas abas DLSSNR-AMD, geração de quadros, desempenho, capturas, opções e avançado, reúne os ajustes técnicos e logs daquela área. O relatório de mods Nexus continua disponível. Confira o texto filtrado em **Revisar envio** e confirme para publicar no fórum de suporte do Discord; vídeos, áudio e capturas de tela não são anexados. Os erros do launcher também aparecem no sininho de notificações.

## Instalação

Baixe AppImage, RPM ou DEB em [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM e DEB instalam os binários `peligames` e `pelinstall` em `/opt/PeliGames`, com comandos em `/usr/bin` e atalhos nos diretórios de aplicativos do sistema. Pelo terminal, use `peligames` ou `pelinstall /caminho/programa.exe`.
- AppImage reúne os dois módulos. Dê permissão de execução e abra o arquivo. Ele funciona direto, mas também pode ser instalado no seu usuário, como fazem o Gear Lever e o AppManager.
- O AppImage leva o próprio WebKitGTK (2.52) e precisa de glibc 2.39 ou mais nova: Ubuntu 24.04, Linux Mint 22, Debian 13, Fedora 40 e posteriores, Arch, CachyOS, Nobara e Bazzite. Os pacotes DEB e RPM têm o mesmo requisito.
- Para abrir um executável Windows no instalador: `./PeliGames.AppImage --install /caminho/programa.exe` (depois de instalado, `peligames --install /caminho/programa.exe`).

### Instalar o AppImage

Na primeira abertura, a tela de boas-vindas oferece **Instalar e abrir**. O mesmo botão fica em **Configurações → Ferramentas** para quem pulou essa etapa. A instalação não pede root nem FUSE:

- o conteúdo do AppImage é extraído em `~/.local/share/PeliGames/app` e roda direto de lá, o que abre mais rápido que o arquivo compactado;
- `~/.local/share/PeliGames/peligames` é o comando usado pelos atalhos, e `~/.local/bin/peligames` aponta para ele no terminal (um arquivo `peligames` seu nesse lugar nunca é substituído);
- o PeliGames e o Pelinstall aparecem no menu de aplicativos, na categoria Jogos; os atalhos de jogos e os links do Nexus passam a abrir a cópia instalada;
- o arquivo `.AppImage` baixado é removido depois da extração.

As atualizações baixam o AppImage novo, extraem ao lado e trocam a pasta `app`. A versão anterior fica em `app.previous` até a nova abrir e volta sozinha se a nova não iniciar.

Para desinstalar, apague `~/.local/share/PeliGames`, o link `~/.local/bin/peligames` e os atalhos `peligames-*.desktop` e `pelinstall-appimage.desktop` em `~/.local/share/applications`. Os dados do launcher ficam à parte, em `~/.config/PeliGames`.

Proton, UMU e Winetricks são baixados conforme necessário. Configurações, ferramentas, mods e dados da interface ficam em `~/.config/PeliGames`; para começar do zero, use **Configurações → Ferramentas → Redefinir PeliGames**, revisando as opções de preservar contas, jogos do Pelinstall e mods. O backend e os arquivos do mod ficam em `~/.config/PeliGames/components/Mods/DLSSNR`. O setup do mod, as teclas por jogo e os diretórios personalizados ficam em `~/.config/PeliGames/components/Mods/DLSSNR/config.json`; as preferências de inicialização do Neural Rendering ficam em `neural-startup.json` nessa mesma pasta. As preferências de capas e Proton permanecem em `~/.config/PeliGames/settings/config.json`. A pasta é organizada em `settings` (preferências), `profiles` (perfis), `library` (registros), `components`, `cache`, `logs`, `runtime` e `webview`. Atualizações migram o layout antigo automaticamente; os textos alterados ficam preservados em `migration-backup`. Prefixos e jogos permanecem nos diretórios escolhidos pelo usuário. Capas do SteamGridDB exigem uma chave pessoal opcional, configurada em Preferências.

## Mods locais do Nexus

Na aba **Mods** dos detalhes do jogo, conecte sua conta em **Configurações → Conexões → Nexus Mods** para acessar o catálogo e as coleções. Arquivos ZIP, 7z e RAR podem ser importados pelo botão **Mod Local**, sem conta conectada. Jogos sem módulo compatível exibem a explicação de indisponibilidade nessa mesma aba. Para receber downloads do site, habilite **Usar PeliGames nos links Nexus** e escolha **Mod Manager Download**.

Contas gratuitas confirmam o download na página do Nexus; Premium permite download direto. Acompanhe a fila em **Downloads Nexus** e, na lista de mods, confira os requisitos antes de **Instalar**, **Ativar** ou **Desativar**.

Integração independente, sem afiliação ao Nexus Mods. Reporte erros ao PeliGames, não ao Nexus Mods.

O suporte depende do jogo e do formato do pacote. A chave fica no chaveiro do sistema; AppImage requer `secret-tool` e um chaveiro disponível. Use **Logs** ou **Reportar erro** para diagnosticar problemas.

Em **Configurações → Ferramentas**, crie um backup ZIP dos jogos com mods Nexus instalados ou restaure um ZIP salvo em outro disco. A opção de recuperação por jogo mantém uma pasta oculta `.peligames` no disco dos jogos, permitindo recuperar os registros ao escanear novamente após trocar de distribuição. Use **Encontrar mods no SSD** para procurar cópias de recuperação em uma biblioteca inteira. Registros, pacotes, perfis e originais preservados ficam junto de cada jogo em `.peligames/mods/<nome-do-jogo>/`, e os pacotes levam o nome e a versão do mod; pastas e pacotes de versões anteriores recebem esses nomes automaticamente. O índice de localização é preservado fora das pastas redefinidas, inclusive para discos desconectados.

## Biblioteca unificada e geração de quadros

Nexus Mods, DLSSNR e geração de quadros ficam nas abas dos **detalhes de cada jogo**, acessíveis pela biblioteca principal. A Visão geral distingue a raiz da instalação, o prefixo e o destino específico do DLSSNR. Escolher o destino do mod não altera a pasta usada para identificar o jogo. Pastas manuais continuam permitindo adicionar uma coleção com várias subpastas de jogos.

Na aba **Geração de quadros**, configure o **MAKO Renderer 4.5.0** (multiplicador fixo ou adaptativo), versão estável verificada em 18/10/2026. O botão de instalação baixa o pacote oficial, valida a estrutura e confere o SHA-256 quando há um checksum de referência; a ativação e as configurações são específicas de cada instalação e valem na próxima execução. Componentes e perfis ficam em `~/.config/PeliGames/components/FrameGeneration`, sem arquivos novos nas pastas dos jogos ou ativação Vulkan global. A busca de lançamentos estáveis usa cache de seis horas e também pode ser solicitada na interface. Atualizações preservam os runtimes anteriores: em **Versões e backups**, selecione a versão para este jogo; as alterações são salvas automaticamente. A lixeira só remove backups sem vínculo com perfis. Versões maiores que precisem de adaptação da integração não são instaladas automaticamente.

O overlay é opcional, começa oculto e usa **F8** (configurável) para abrir e fechar dentro do jogo. Ele não registra atalho global no desktop e Esc continua reservado ao jogo. Multiplicador, modo adaptativo, FPS alvo, escala de movimento e modelo leve podem ser ajustados pelo launcher durante a partida, com salvamento automático; a ativação e o carregamento do overlay exigem reabrir o jogo. Os detalhes e limites estão no [guia do overlay](docs/overlay.md).

> **O overlay foi feito para jogos offline.** Ele funciona do lado Linux, sem DLL injetada no jogo, mas carrega bibliotecas no processo e intercepta teclado e mouse para o painel interativo. Em jogos online com anti-cheat ou proteção própria, isso pode ser interpretado como trapaça e levar a bloqueio ou banimento da conta. **O uso em jogos online é por conta e risco de quem joga**; o PeliGames não se responsabiliza por punições aplicadas por jogos ou servidores.

A geração exige uma instalação própria do **Lossless Scaling**: O MAKO utiliza a ramificação pública padrão e `Lossless.dll`. O PeliGames procura essas DLLs nas bibliotecas Steam e permite selecioná-las manualmente. Não fornece essas DLLs.

Use um único gerador de quadros. Desative o FG interno do jogo e do OptiScaler; o launcher bloqueia combinações conhecidas com MLFG, camadas de baixa latência DX12/Vulkan e captura Vulkan OBS. OptiScaler para upscaling e DLSSNR não são automaticamente considerados incompatíveis; o resultado depende do jogo. Configuração explícita de FG ativado no `OptiScaler.ini` impede ativar outro gerador. Requer Vulkan, diretamente ou via DXVK/VKD3D. A integração inicial é para jogos no host; launchers Flatpak informam a necessidade da extensão de runtime correspondente. MangoHUD pode contar somente os quadros reais.

A primeira ativação mostra a linha necessária nas opções de inicialização da Steam. É possível colar e verificar com a Steam aberta, ou pedir aplicação automática com reinício da Steam; ajustes do perfil não repetem essa etapa. Flatpak exige extensão e acesso aos perfis/DLL: use **Preparar / verificar Flatpak**, que mostra a ABI e os comandos específicos antes de liberar a ativação. O multiplicador vai de **2× a 5×** e o FPS alvo adaptativo de **10 a 1000**. Veja o [guia de uso e compatibilidade](docs/overlay.md) para requisitos e limites de cada launcher.

O auxiliar Rust **PeliRun** exporta perfis com identificadores legíveis, como `tomb-raider`, para Steam, Heroic, Lutris e outros launchers com suporte a wrapper/prefixo de comando. A interface fornece o executável e os argumentos adequados a cada formato; um perfil identifica a instalação, sem depender apenas do AppID da Steam.

Na Steam, uma linha fixa chama o auxiliar Rust `pelirun`, que lê o perfil atualizado do jogo. Depois de vincular essa linha uma vez, mudar MangoHUD, geração de quadros, variáveis ou argumentos não exige reiniciar a Steam: as opções de inicialização valem ao reabrir o jogo. Ajustes com recarga durante a partida dependem de cada ferramenta. A escolha do Proton continua em **Compatibilidade**, na Steam. Veja [o funcionamento e os limites do auxiliar](docs/steam-launch-runtime.md).

Referências: [MAKO Renderer](https://github.com/eugeniosegala/MAKO/blob/main/engine/README.md), [compatibilidade do MAKO](https://github.com/eugeniosegala/MAKO/blob/main/engine/docs/LAYER-CHAINING.md).

## DLSS 5 Neural Rendering em placas AMD (DLSSNR)

Nos detalhes do jogo, abra **Mods → DLSSNR-AMD** para instalar e configurar o Neural Rendering. O PeliGames gerencia o backend, os modelos e as opções de inicialização de cada instalação.

> Projetos experimentais (versões 0.0.x), sem afiliação com a NVIDIA nem com a AMD. **Não use em jogos online com anti-cheat**: DLLs injetadas podem levar ao banimento da conta. O instalador coloca arquivos na pasta do jogo; **Remover** apaga só o que ele instalou.

### Placas e backends

| Placa | Backend | Referência dos autores |
|---|---|---|
| RX 9000 (RDNA 4) | [DLSSNR-AMD, de mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD) | Mesa 26.2 ou superior e Proton recente; o 0.0.8 foi testado com GE-Proton 11-11 e Mesa 26.2.5 |
| RX 7000 (RDNA 3) | [DLSSNR-RDNA3, de mauri870](https://github.com/mauri870/DLSSNR-RDNA3/blob/rdna3/README-RDNA3.md) | Fork próprio para RX 7000; testado pelo autor na RX 7900 XTX com Mesa 26.2.3 |

O backend RDNA 4 usa instruções FP8; o fork RDNA 3 adapta o processamento para FP16. As opções mostradas pelo PeliGames dependem das capacidades do pacote instalado. Não se presume que os dois projetos tenham os mesmos recursos.

### Preparar o modelo e instalar no jogo

1. Abra **Configurações → Complementos → DLSSNR-AMD** e baixe o backend da sua GPU. **Importar backend local** aceita uma pasta com os pacotes Linux do projeto ou as subpastas de arquitetura já extraídas.
2. No assistente de configuração, use **Extrair de uma DLL** com sua `nvngx_dlssnr.dll`, ou **Usar arquivo existente** com modelos `.bin` válidos. Essas opções ficam disponíveis depois da preparação do backend. Os modelos são conferidos por SHA-256.
3. Na biblioteca, abra os detalhes do jogo e **Mods → DLSSNR-AMD**. Confira o destino do mod, selecione o modo, a arquitetura e a rede inicial e clique em **Instalar** ou **Reparar**.
4. Conclua **Configurar na Steam**, quando solicitado, ou use o perfil PeliRun no outro launcher. Depois, abra o jogo.

A DLL e os pesos da NVIDIA **não acompanham o PeliGames nem o download do backend**. Obtenha uma DLL compatível a partir de um jogo que a forneça. O instalador do backend utiliza `bash` e `python3`.

| DLL usada na extração | Modelos importados pelo launcher |
|---|---|
| **310.8.0** | `dlssnr.bin` — modelo antigo |
| **310.9.16** | `dlssnr.bin` e `dlssnr-small.bin` — modelos antigo e novo |

Os arquivos ficam em `~/.config/PeliGames/components/Mods/DLSSNR/backend-files/<rdna3|rdna4>/<32|64>/dlssnr-amd/`. Ao importar os modelos da 310.9.16, mantenha os dois `.bin` juntos. Modelos válidos em cache podem ser reutilizados nas próximas instalações.

### Rede inicial e opções do launcher

No backend Mochizuki 0.0.8, **Rede inicial** oferece:

| Opção | Rede | Modelo necessário |
|---|---|---|
| **Novo** | `CC_SMALL_FP8` | 310.9.16 |
| **Experimental** | `CC_SMALL_FP8_EXPERIMENTAL` | 310.9.16; pesos derivados durante a instalação |
| **Antigo** | `CC_SILVER_AARDWOLD` | 310.8.0 ou 310.9.16 |
| **int4** | `CC_SILVER_AARDWOLD_INT4_MIXED` | Modelo antigo; pesos derivados durante a instalação |

A rede **Experimental** pode alterar a imagem. Ela só aparece com backend compatível. Backends anteriores e RDNA 3 conservam os seletores e opções que seus scripts suportam; int4 não se combina com o modelo novo.

| Ajuste no PeliGames | Quando é aplicado |
|---|---|
| **Começar ligado** e **RenoDX** | Em um mod já instalado, são salvos automaticamente para a próxima abertura do jogo |
| Modo, arquitetura, rede inicial e atalho do menu | Ao instalar ou reparar |
| **VORT / LumeniteFX**, nas rotas ReShade compatíveis | Ao instalar ou reparar; VORT acompanha o backend, enquanto LumeniteFX exige download adicional do projeto oficial |

**Instalar ou reparar substitui o `dlssnr-amd.ini` pelos padrões do backend e pelas escolhas de instalação.** Guarde uma cópia dos ajustes personalizados antes dessa operação. Atualizar o backend no launcher não atualiza automaticamente os arquivos instalados nos jogos: repare cada jogo que deve receber a atualização.

### Escolher o modo

A tabela descreve as rotas do Mochizuki 0.0.8 para RX 9000. Com RDNA 3 ou pacotes anteriores, siga as rotas e os controles disponíveis no backend instalado.

| Jogo | Modo no launcher | Menu durante a partida |
|---|---|---|
| DirectX 11/12 ou Vulkan, 64 bits, com DLSS, FSR ou XeSS disponível | **OptiScaler** | **Insert**, por padrão, abre OptiScaler; **End** abre NR Control |
| DirectX 10/11/12 sem upscaler compatível | **ReShade** | **Home → Add-ons**, por padrão |
| Vulkan sem upscaler compatível | **Vulkan** | **Home → Add-ons**, por padrão |
| DirectX 9 | **DX9** | **Home → Add-ons**, por padrão |

No Mochizuki 0.0.8, a rota OptiScaler utiliza o OptiScaler original com o plugin `dlssnr-amd.asi`. Backends antigos podem usar o fork OptiScaler-NR. Ative um upscaler compatível nas configurações do jogo ao usar essa rota. Jogos 32 bits não usam OptiScaler; no RDNA 3, o PeliGames oferece somente 64 bits. OpenGL não é suportado.

### Ajustes no jogo com Mochizuki 0.0.8

O launcher mantém os controles de preparação e inicialização. **Camadas, processamento RenoDX/wilsjo2, passes rápidos ou tradicionais, proteção do HUD, comparação visual e diagnósticos de custo/VRAM** ficam no **NR Control**, na rota OptiScaler, ou em **ReShade → Add-ons**, nas demais rotas do backend 0.0.8. Os detalhes e limitações estão nas [notas oficiais do 0.0.8](https://github.com/mochizuki0323/DLSSNR-AMD/releases/tag/v0.0.8-linux-preview).

No OptiScaler, o menu de **Insert** cuida de upscaling e geração de quadros; o **End** abre os ajustes NR. Use o menu NR para alternar redes durante a partida; **Ctrl+F11** é o atalho padrão de troca nos backends compatíveis. Os ajustes ficam no `dlssnr-amd.ini` da pasta onde o mod foi instalado.

### Inicialização pela Steam e outros launchers

O PeliGames salva as opções impressas pelo instalador no perfil do jogo, substituindo as opções NR anteriores. Use **Configurar na Steam** ou a linha fornecida pelo **PeliRun** uma vez. Com o perfil vinculado, as próximas alterações valem ao reabrir o jogo, sem repetir a vinculação nem reiniciar a Steam. A seleção do Proton continua na Steam ou no launcher correspondente. Veja o [guia do PeliRun](docs/steam-launch-runtime.md).

Não acrescente camadas Vulkan de NR por conta própria à rota OptiScaler do 0.0.8. Rotas ReShade e backends anteriores podem precisar delas; siga o perfil gerado pelo instalador. Se a sincronização falhar, **Visualizar log** mostra as opções necessárias para configuração manual.

### Custo de processamento

Tempos publicados pelos autores, em milissegundos por quadro, somente da rede:

| GPU e rede | 1080p | 1440p | 4K |
|---|---|---|---|
| RX 9070 XT — Novo | 3,27 ms | 5,53 ms | 12,67 ms |
| RX 9070 XT — Experimental | 2,90 ms | 4,84 ms | 11,09 ms |
| RX 7900 XTX — Antigo | 15 ms | 26 ms | 56 ms |
| RX 7900 XTX — int4 do antigo | 13,3 ms | 22,8 ms | 50,8 ms |

São referências de versões e métodos de medição diferentes, sem incluir o restante do jogo nem todo o processamento do mod. Não representam o FPS final nem garantem o mesmo resultado em outra máquina. Confira as condições nas referências de [RDNA 4](https://github.com/mochizuki0323/DLSSNR-AMD#performance) e [RDNA 3](https://github.com/mauri870/DLSSNR-RDNA3/blob/rdna3/README-RDNA3.md#what-it-costs). Reduzir a resolução de processamento da rede pode diminuir o custo, com impacto na imagem.

### Quando algo não funciona

- Na primeira execução, aguarde a compilação da rede; o tempo depende do backend e da máquina.
- Confira `dlssnr-amd.log` e `OptiScaler.log` ou `ReShade.log` na pasta do mod. Instalar os arquivos não comprova que o NR está processando a imagem.
- Se uma opção não aparecer, confira a GPU selecionada, o backend instalado e os modelos disponíveis no assistente.
- Use **Reportar erro** na área DLSSNR-AMD do jogo. O relatório reúne configurações e logs; revise os dados filtrados antes de confirmar o envio ao suporte no Discord. **Copy a report**, nos diagnósticos do backend 0.0.8, também gera um relatório local.

## Jogos suportados

O PeliGames instala mods do Nexus Mods em **215 jogos**, com módulos baseados nas extensões do Vortex: Fallout, Skyrim, Starfield, Cyberpunk 2077, Baldur's Gate 3, Stardew Valley, Elden Ring, a família Resident Evil e Monster Hunter, além de dezenas de jogos Unreal e Unity.

| Concluído | Jogos |
|---|---|
| 100% — verificado em jogo real | 5 |
| 90% — paridade com a extensão Vortex | 67 |
| 80% — recursos da extensão ainda pendentes | 29 |
| 70% — gerado do template da família | 113 |
| 60% — sem suporte Vortex confirmado | 1 |

**[Ver a lista completa de jogos suportados →](docs/supported-games.md)**

## Compilar a partir do código

1. Instale as dependências de sistema do Tauri 2:
   - Debian/Ubuntu: `sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`
   - Fedora: `sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel` e `sudo dnf group install c-development`
2. Instale o [Rust](https://rustup.rs) e o [Bun](https://bun.sh).
   O auxiliar da Steam exige também `rustup target add x86_64-unknown-linux-musl` (em ARM64, use `aarch64-unknown-linux-musl`).
3. Baixe o código e as dependências da interface:
   ```
   git clone https://github.com/pelicanux/PeliGames.git
   cd PeliGames
   bun install
   cargo fetch --locked --manifest-path src-tauri/launch-runtime/Cargo.toml
   ```
4. Compile a interface (gera também os ícones):
   ```
   bun run build
   ```
5. Compile os dois executáveis, com a interface embutida:
   ```
   cargo build --release --bins --features custom-protocol --manifest-path src-tauri/Cargo.toml
   ```
6. Os executáveis ficam em `src-tauri/target/release/`: `peligames` abre o launcher e `pelinstall /caminho/programa.exe` abre o instalador.

Para gerar pacotes (AppImage, DEB e RPM) em `src-tauri/target/release/bundle/`, use `bun run tauri build`.

## Créditos e licença

PeliGames é desenvolvido por Pelicano e distribuído sob [GNU GPL 3.0](LICENSE). Versões derivadas distribuídas devem disponibilizar o código-fonte correspondente sob a mesma licença.

Integra [DLSSNR-AMD, de mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD), e [DLSSNR-RDNA3, de mauri870](https://github.com/mauri870/DLSSNR-RDNA3).

A tradução opcional das descrições de mods utiliza a API online [MyMemory, da Translated S.r.l.](https://mymemory.translated.net/). O texto é enviado ao serviço somente ao solicitar a tradução. Consulte os [avisos de uso, privacidade e cotas](licenses/MyMemory-Service.md).

Os componentes de terceiros mantêm suas próprias licenças. Os [créditos e avisos completos](licenses/README.md) acompanham o programa e podem ser consultados na janela **Sobre**.

## Agradecimentos especiais

Obrigado aos desenvolvedores e às comunidades dos projetos que tornam possível jogar no Linux e dão suporte às ferramentas utilizadas pelo PeliGames:

- [GE-Proton — GloriousEggroll](https://github.com/GloriousEggroll/proton-ge-custom)
- [Proton CachyOS — equipe CachyOS](https://github.com/CachyOS/proton-cachyos)
- [UMU Launcher — Open Wine Components](https://github.com/Open-Wine-Components/umu-launcher)
- [Wine](https://www.winehq.org/) e [Proton — Valve e colaboradores](https://github.com/ValveSoftware/Proton)
- [DXVK](https://github.com/doitsujin/dxvk)
- [VKD3D-Proton](https://github.com/HansKristian-Work/vkd3d-proton)
- [Winetricks](https://github.com/Winetricks/winetricks)

Conheça e apoie esses projetos. Nosso agradecimento também a quem testa, relata problemas e contribui para melhorar os jogos no Linux.
