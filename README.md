<p align="center">

> **Publicação 0.4.4:** geração de quadros MAKO, overlay PeliGames e seu atalho estão em desenvolvimento e indisponíveis nesta versão. A aba permanece visível e desabilitada. DLSSNR-AMD e monitoramento continuam disponíveis.
  <img src="public/images/readme-header.svg" alt="PeliGames" width="900" />
</p>

<p align="center"><strong>Português</strong> · <a href="README.en.md">English</a></p>

Launcher de jogos Windows para Linux com Proton, biblioteca de jogos, Pelinstall, ferramentas Wine, integração Nexus Mods e gerenciador de mods DLSSNR-AMD.

<p align="center">
  <img src="public/images/interface-preview.webp" alt="Interface do PeliGames com biblioteca de jogos organizada por origem" width="1200" />
</p>

## Como funciona

1. **Biblioteca:** o PeliGames reúne num só lugar os jogos da Steam, da Epic Games nativa para Linux, do Heroic, do Lutris, do Bottles e do Faugus, além dos que você instala por ele.
2. **Instalar jogos e programas do Windows:** escolha o instalador (`.exe` ou `.msi`), o nome, a pasta e a versão do Proton. O PeliGames prepara o prefixo, roda o instalador e encontra sozinho o executável do jogo. Se houver dúvida entre executáveis, ele pergunta.
3. **Launchers (EA app, Battle.net, Ubisoft Connect, Epic):** os jogos que você instala por dentro do launcher aparecem em **Programas**, para abrir direto. Ao adicionar um deles à biblioteca, ele ganha um perfil próprio, com prefixo, Proton e mods separados, sem duplicar os arquivos do launcher.
4. **Mods do Nexus Mods:** abra o jogo na **Biblioteca → Mods**, escolha os mods no catálogo (ou importe um arquivo) e clique em **Instalar**. O PeliGames confere os requisitos, aplica o instalador FOMOD quando existe e implanta os arquivos de acordo com as regras do jogo, as mesmas usadas pelo Vortex. Os arquivos originais substituídos com autorização são preservados para restauração pelo gerenciador. Veja os [jogos suportados](docs/supported-games.md).
5. **Problemas:** **Reportar erro** gera um relatório do jogo e pode enviá-lo direto ao fórum de suporte do Discord do PeliGames. Os erros do launcher também aparecem no sininho de notificações.

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

Proton, UMU e Winetricks são baixados conforme necessário. Configurações, ferramentas, mods e dados da interface ficam em `~/.config/PeliGames`; para começar do zero, use **Configurações → Ferramentas → Redefinir PeliGames**, revisando as opções de preservar contas, jogos do Pelinstall e mods. O backend e os arquivos do mod ficam em `~/.config/PeliGames/Mod/DLSSNR`. O setup do mod, as teclas por jogo e os diretórios personalizados ficam em `~/.config/PeliGames/Mod/DLSSNR/config.json`; as preferências de inicialização do Neural Rendering ficam em `neural-startup.json` nessa mesma pasta. As preferências de capas e Proton permanecem em `~/.config/PeliGames/config.json`. Prefixos e jogos permanecem nos diretórios escolhidos pelo usuário. Capas do SteamGridDB exigem uma chave pessoal opcional, configurada em Preferências.

## Mods locais do Nexus

Na aba **Mods** dos detalhes do jogo, conecte sua conta em **Configurações → Conexões → Nexus Mods** para acessar o catálogo e as coleções. Arquivos ZIP, 7z e RAR podem ser importados pelo botão **Mod Local**, sem conta conectada. Jogos sem módulo compatível exibem a explicação de indisponibilidade nessa mesma aba. Para receber downloads do site, habilite **Usar PeliGames nos links Nexus** e escolha **Mod Manager Download**.

Contas gratuitas confirmam o download na página do Nexus; Premium permite download direto. Acompanhe a fila em **Downloads Nexus** e, na lista de mods, confira os requisitos antes de **Instalar**, **Ativar** ou **Desativar**.

Integração independente, sem afiliação ao Nexus Mods. Reporte erros ao PeliGames, não ao Nexus Mods.

O suporte depende do jogo e do formato do pacote. A chave fica no chaveiro do sistema; AppImage requer `secret-tool` e um chaveiro disponível. Use **Logs** ou **Reportar erro** para diagnosticar problemas.

Em **Configurações → Ferramentas**, crie um backup ZIP dos jogos com mods Nexus instalados ou restaure um ZIP salvo em outro disco. A opção de recuperação por jogo mantém uma pasta oculta `.peligames` no disco dos jogos, permitindo recuperar os registros ao escanear novamente após trocar de distribuição. Use **Encontrar mods no SSD** para procurar cópias de recuperação em uma biblioteca inteira. Novas instalações mantêm registros, pacotes, perfis e originais preservados junto de cada jogo em `.peligames/mods/<id>/`; registros antigos são migrados antes da redefinição. O índice de localização é preservado fora das pastas redefinidas, inclusive para discos desconectados.

## Biblioteca unificada e geração de quadros

Nexus Mods, DLSSNR e geração de quadros ficam nas abas dos **detalhes de cada jogo**, acessíveis pela biblioteca principal. A Visão geral distingue a raiz da instalação, o prefixo e o destino específico do DLSSNR. Escolher o destino do mod não altera a pasta usada para identificar o jogo. Pastas manuais continuam permitindo adicionar uma coleção com várias subpastas de jogos.

Na aba **Geração de quadros**, configure o **MAKO Renderer 4.5.0** (multiplicador fixo ou adaptativo), versão estável verificada em 18/10/2026. O botão de instalação baixa o pacote oficial, valida a estrutura e confere o SHA-256 quando há um checksum de referência; a ativação e as configurações são específicas de cada instalação e valem na próxima execução. Componentes e perfis ficam em `~/.config/PeliGames/FrameGeneration`, sem arquivos novos nas pastas dos jogos ou ativação Vulkan global. A busca de lançamentos estáveis usa cache de seis horas e também pode ser solicitada na interface. Atualizações preservam os runtimes anteriores: em **Versões e backups**, selecione a versão para este jogo e salve. A lixeira só remove backups sem vínculo com perfis. Versões maiores que precisem de adaptação da integração não são instaladas automaticamente.

A geração exige uma instalação própria do **Lossless Scaling**: O MAKO utiliza a ramificação pública padrão e `Lossless.dll`. O PeliGames procura essas DLLs nas bibliotecas Steam e permite selecioná-las manualmente. Não fornece essas DLLs.

Use um único gerador de quadros. Desative o FG interno do jogo e do OptiScaler; o launcher bloqueia combinações conhecidas com MLFG, camadas de baixa latência DX12/Vulkan e captura Vulkan OBS. OptiScaler para upscaling e DLSSNR não são automaticamente considerados incompatíveis; o resultado depende do jogo. Configuração explícita de FG ativado no `OptiScaler.ini` impede ativar outro gerador. Requer Vulkan, diretamente ou via DXVK/VKD3D. A integração inicial é para jogos no host; launchers Flatpak informam a necessidade da extensão de runtime correspondente. MangoHUD pode contar somente os quadros reais.

A primeira ativação mostra a linha necessária nas opções de inicialização da Steam. É possível colar e verificar com a Steam aberta, ou pedir aplicação automática com reinício da Steam; ajustes do perfil não repetem essa etapa. Flatpak exige extensão e acesso aos perfis/DLL: use **Preparar / verificar Flatpak**, que mostra a ABI e os comandos específicos antes de liberar a ativação. O multiplicador vai de **2× a 5×** e o FPS alvo adaptativo de **10 a 1000**. Veja [a revisão da integração](docs/mako-integration.md) para requisitos e limites de cada launcher.

Na Steam, uma linha fixa chama o auxiliar Rust `pelirun`, que lê o perfil atualizado do jogo. Depois de vincular essa linha uma vez, mudar MangoHUD, geração de quadros, variáveis ou argumentos não exige reiniciar a Steam: as opções de inicialização valem ao reabrir o jogo. Ajustes com recarga durante a partida dependem de cada ferramenta. A escolha do Proton continua em **Compatibilidade**, na Steam. Veja [o funcionamento e os limites do auxiliar](docs/steam-launch-runtime.md).

Referências: [MAKO Renderer](https://github.com/eugeniosegala/MAKO/blob/main/engine/README.md), [compatibilidade do MAKO](https://github.com/eugeniosegala/MAKO/blob/main/engine/docs/LAYER-CHAINING.md).

## DLSS 5 Neural Rendering em placas AMD (DLSSNR)

A aba **DLSSNR-AMD** roda o modelo de Neural Rendering (NR) do DLSS 5 da NVIDIA em placas AMD Radeon. Quem faz esse trabalho são dois projetos independentes, que reimplementam a rede em shaders Vulkan, executados no próprio dispositivo Vulkan do jogo (DXVK / vkd3d-proton sob o Proton). O PeliGames baixa o backend certo para a sua placa, executa o instalador dele com as opções escolhidas na interface e guarda o modelo para as próximas instalações. As informações abaixo resumem a documentação desses projetos e explicam como o PeliGames os usa.

> Projetos experimentais (versões 0.0.x), sem afiliação com a NVIDIA nem com a AMD. **Não use em jogos online com anti-cheat**: DLLs injetadas podem levar ao banimento da conta. O instalador coloca arquivos na pasta do jogo; **Remover** apaga só o que ele instalou.

### Placas e backends

| Placa | Backend | Testado pelos autores |
|---|---|---|
| RX 9000 (RDNA 4) | [DLSSNR-AMD, de mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD) | RX 9070 XT, Mesa 26.2, GE-Proton |
| RX 7000 (RDNA 3) | [DLSSNR-RDNA3, de mauri870](https://github.com/mauri870/DLSSNR-RDNA3) (fork do anterior) | RX 7900 XTX, Mesa 26.2.3, GE-Proton 11-7 |

- O PeliGames identifica a série da placa e oferece o backend correspondente em **Configurações → Mods → DLSSNR-AMD**. O download vem das releases do GitHub de cada projeto e fica em `~/.config/PeliGames/Mod/DLSSNR/backend-files/<rdna3|rdna4>/<32|64>`.
- A RX 7000 não tem as instruções de matriz FP8 da RDNA 4. O fork reproduz a mesma rede com FP16 e chega à mesma fidelidade, mas custa cerca de 2,6 vezes mais por quadro.
- Requisitos: Mesa 26.2 ou mais novo (driver RADV) e um Proton recente. O instalador do backend usa `bash` e `python3`.

### O modelo

A `nvngx_dlssnr.dll` e os pesos pertencem à NVIDIA e **não acompanham o PeliGames nem o download do backend**. Esse download fornece o instalador, as ferramentas de extração e os componentes do mod. Obtenha uma DLL compatível por fora do launcher, a partir de um jogo que a forneça.

1. Na **Configuração inicial**, escolha RDNA 4 / RX 9000 ou RDNA 3 / RX 7000 e baixe o backend. Em **Arquivos locais**, selecione a pasta com os pacotes do backend baixados manualmente das releases dos mantenedores. Essa opção importa o backend, não substitui a seleção do modelo de IA.
2. Espere o download e a verificação terminarem. **Extrair de uma DLL** e **Usar arquivo existente** só ficam disponíveis com o backend pronto; arquivos em cache podem ser reutilizados. O caminho sublinhado abaixo do download abre a pasta onde ele foi salvo.
3. Escolha **Extrair de uma DLL** e indique a sua `nvngx_dlssnr.dll` compatível, ou use **Usar arquivo existente** para importar um modelo `.bin` válido. Os modelos são conferidos por SHA-256.

| DLL usada na extração | Arquivos gerados pelo backend compatível |
|---|---|
| **310.8.0** | `dlssnr.bin` — modelo antigo |
| **310.9.16** | `dlssnr.bin` e `dlssnr-small.bin` — modelos antigo e novo, em arquivos separados |

Os arquivos permanecem no diretório padrão do backend: `~/.config/PeliGames/Mod/DLSSNR/backend-files/<rdna3|rdna4>/<32|64>/dlssnr-amd/`, conforme as arquiteturas disponíveis. Não é necessário mover o `.bin` para a raiz `Mod/DLSSNR`. Ao importar o modelo novo já extraído, mantenha `dlssnr-small.bin` ao lado de `dlssnr.bin`; o launcher importa os dois.

Nos detalhes do jogo, o backend **0.0.7 da RX 9000** usa **Rede inicial → Novo / Antigo / int4**. As escolhas correspondem a `CC_SMALL_FP8` (310.9.16), `CC_SILVER_AARDWOLD` (310.8.0) e `CC_SILVER_AARDWOLD_INT4_MIXED` (derivado do antigo). O instalador prepara as redes disponíveis juntas; a escolha no launcher define qual inicia após instalar ou reparar. No jogo, alterne no **NR Control (End)**, em **ReShade → Add-ons**, ou com **Ctrl+F11**, sem reinstalar a cada troca. Redes sem pesos ou sem suporte ficam indisponíveis no menu do backend. O launcher utiliza a linha completa impressa pelo instalador com a camada int4 quando ela está disponível, mesmo se a rede inicial for outra.

Backends anteriores e o fork RDNA3 mantêm o fluxo que seus scripts suportam: **Modelo NR → Novo / Antigo**, quando disponível, e a opção separada de int4. Nesses backends, após mudar as opções do launcher, use **Reparar**. Não se presume que o fork RDNA3 tenha recebido as mudanças do 0.0.7.

Quem atualiza o PeliGames não precisa apagar o backend anterior. Para habilitar o modelo novo, atualize também o backend em **Configurações → Mods → DLSSNR-AMD** e extraia a DLL **310.9.16** na configuração inicial. Atualizar somente o launcher não fornece a DLL nem gera os modelos novos.

### Backend Mochizuki 0.0.7 (19/10/2026)

A versão Linux estável 0.0.7 foi publicada para x86_64 e i686 junto de um pacote Windows experimental. O atualizador seleciona somente os assets Linux da arquitetura correspondente. O arquivo interno `backend-files/rdna4/version.txt` continua registrando a tag do pacote com o sufixo `-rdna4`; os nomes das redes não substituem essa versão. Os hashes dos modelos 310.8.0/310.9.16 não mudaram: modelos válidos em cache podem ser reutilizados.

O backend acrescentou troca de redes durante o jogo, política de memória **Residency**, indicador de rede **OSD**, diagnóstico **Engine data** e opções de rede/log no menu do próprio mod. Também corrigiu crescimento de VRAM em mudanças de resolução, leitura de texturas e diferenças no processamento Natural/Cinematic. Esses recursos vêm do backend atualizado; não exigem que o launcher reproduza seus controles nem permanecem disponíveis em uma instalação antiga do mod. Atualize o backend e use **Reparar / reinstalar** em cada jogo que deve recebê-los. O PeliRun reaproveita a linha fixa já vinculada; a alteração do perfil não exige reiniciar a Steam.

**LumeniteFX** é uma alternativa opcional de vetores de movimento para as rotas ReShade, oferecida pelo script oficial com `--lumenite`; não se aplica ao OptiScaler. É um projeto separado e exige um download adicional do autor. O PeliGames mantém VORT como padrão e não ativa nem baixa LumeniteFX automaticamente nesta integração. [Notas oficiais do release](https://github.com/mochizuki0323/DLSSNR-AMD/releases/tag/v0.0.7).

### Rotas

Escolha a rota pela API do jogo. O PeliGames converte o nome para o que o backend instalado entende: as versões novas chamam as rotas do ReShade de `reshade-feeder`, `reshade-feeder-vulkan` e `reshade-feeder-dx9`, enquanto o fork da RX 7000 ainda usa `reshade`, `vulkan` e `dx9`.

| O jogo usa | Rota | Como funciona | Onde ajustar no jogo |
|---|---|---|---|
| DirectX 11/12 com opção de DLSS, FSR ou XeSS | **OptiScaler** (só 64 bits) | [OptiScaler-NR](https://github.com/wilsjo2/OptiScaler-DLSSNR-PreSR-Multipass) com o backend como DLSS-NR, usando os vetores de movimento do jogo. Ative DLSS, FSR ou XeSS no jogo. | **Insert**: menu do OptiScaler. **End**: NR Control (só RX 9000) |
| DirectX 10/11/12 sem upscaler | **ReShade** | ReShade com vetores de movimento estimados pela imagem (VORT) | **Home** → aba **Add-ons** |
| Vulkan | **Vulkan** | ReShade carregado como camada Vulkan | **Home** → aba **Add-ons** |
| DirectX 9 | **DX9** | O DXVK converte o D3D9 em Vulkan; a rota ajusta a direção da profundidade dos jogos antigos | **Home** → aba **Add-ons** |
| OpenGL | não suportado | | |

- Jogos de 32 bits usam o pacote de 32 bits, sem a rota OptiScaler. Na RX 7000, o pacote de 32 bits ainda não foi testado pelo autor, então o PeliGames só oferece 64 bits.
- A tecla de menu escolhida no PeliGames é gravada no `ReShade.ini` ou no `OptiScaler.ini` do jogo.
- As configurações ficam no `dlssnr-amd.ini`, na pasta do jogo, e as mudanças valem na hora. **Reparar** reinstala do zero: o backend desinstala pelo registro `dlssnr-amd-install.txt` e o `dlssnr-amd.ini` volta ao padrão. Guarde uma cópia se tiver ajustes.
- Rodar o modelo abaixo da resolução de saída é o que mais pesa no desempenho. Nas rotas do ReShade, use **Model resolution**: 50% praticamente dobra o FPS. Na rota OptiScaler, use **Generate model before upscaler**.

### Opções

| Opção | RX 9000 | RX 7000 | O que faz |
|---|---|---|---|
| **RenoDX** | todas as rotas | não disponível | Processa a imagem antes e depois do NR como o [add-on DLSS5 do RenoDX](https://github.com/PEQHUB/RenoDX-DLSS5-Generic). A rede é a mesma. Todos os ajustes do NR passam para a seção `[RenoDX]`: no NR Control (End) na rota OptiScaler, ou em Home → Add-ons nas rotas do ReShade. Vale depois de reiniciar o jogo. |
| **int4 misto** | todas as rotas | todas as rotas | Disponível com o modelo antigo (310.8.0). Segunda rede, de 7% a 11% mais rápida, com parte das contas em int4; a imagem muda um pouco. **Precisa das variáveis `VK_ADD_LAYER_PATH` e `VK_INSTANCE_LAYERS` nas opções de inicialização** (veja abaixo). **Ctrl+F11** alterna entre o int4 e a rede padrão. Com ele, o NR demora mais para entrar quando o jogo abre. |
| **NR Control** | rota OptiScaler | não disponível | Menu próprio do NR na tecla **End**: mostra o custo do NR e liga o int4 e o Preprocess. Também precisa das variáveis de camada Vulkan. |
| **Preprocess** | todas as rotas | todas as rotas | Seção `[Preprocess]` do `dlssnr-amd.ini`, desligada por padrão. Ajusta exposição, curva, contraste e saturação da imagem que a rede recebe e corrige jogos que escurecem ou ficam granulados com o NR (como 007 First Light). **Ctrl+F10** compara com e sem. |

**Opções de inicialização automáticas.** O backend informa no fim da instalação as variáveis que o jogo precisa, por exemplo `WINEDLLOVERRIDES="dxgi=n,b"` e, com o int4 misto ou a rota OptiScaler, também `VK_ADD_LAYER_PATH` e `VK_INSTANCE_LAYERS`. O PeliGames grava essas variáveis sozinho nas opções de inicialização do jogo, a cada instalação ou reparo. Ao desligar o int4 e reparar, ou ao remover o mod, ele tira só o que o mod tinha adicionado; o resto do que você escreveu continua igual. Essas opções valem quando o jogo é iniciado pelo PeliGames. Em jogos da Steam, feche a Steam antes, para que ela receba as opções; se preferir abrir pela Steam, copie a linha mostrada no fim da instalação para **Propriedades → Opções de inicialização**. Sem essas variáveis, o jogo usa a rede padrão, e o Ctrl+F11 e a tecla End não fazem nada (o `dlssnr-amd.log` avisa).

### Desempenho medido pelos autores

Tempo da rede por quadro, sem contar o resto do jogo:

| Placa | 1080p | 1440p | 4K |
|---|---|---|---|
| RX 9070 XT | 5,6 ms | 9,7 ms | 21,9 ms |
| RX 9070 XT, int4 misto | 5,0 ms | 8,7 ms | 19,6 ms |
| RX 7900 XTX | 15 ms | 26 ms | 56 ms |
| RX 7900 XTX, int4 misto | 13,3 ms | 22,8 ms | 50,8 ms |

Na RX 7000, o 4K nativo não chega a um FPS jogável: use Model resolution de 50% ou menos (cerca de 16 ms em 4K).

### Quando algo não funciona

- Na primeira execução em cada jogo, a rede leva de 10 a 20 segundos para compilar (com o int4, mais). Até lá a imagem aparece sem NR, o que não significa falha.
- Os logs ficam na pasta do jogo: `dlssnr-amd.log`, mais o `ReShade.log` ou o `OptiScaler.log`, conforme a rota. O `dlssnr-amd.log` diz se o RenoDX, o int4 e o NR estão ativos e o que falta.
- A tecla End só existe na rota OptiScaler. Nas rotas do ReShade, tudo fica em Home → Add-ons.
- A **F6** liga e desliga o NR com o RenoDX ativo, para comparar a imagem.

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

A tradução opcional das descrições de mods utiliza a API online [MyMemory, da Translated S.r.l.](https://mymemory.translated.net/). O texto é enviado ao serviço somente ao solicitar a tradução. Consulte os [avisos de uso, privacidade e cotas](licenses/MyMemory-Service.md) e a [análise da implementação e alternativas](docs/nexus-translation-review.md).

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
