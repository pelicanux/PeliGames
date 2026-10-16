<p align="center">
  <img src="public/images/readme-header.svg" alt="PeliGames" width="900" />
</p>

<p align="center"><strong>Português</strong> · <a href="README.en.md">English</a></p>

Launcher de jogos Windows para Linux com Proton, biblioteca de jogos, Pelinstall, ferramentas Wine, integração Nexus Mods e gerenciador de mods DLSSNR-AMD.

<p align="center">
  <img src="public/images/interface-preview.webp" alt="Interface do PeliGames com biblioteca de jogos organizada por origem" width="1200" />
</p>

## Como funciona

1. **Biblioteca:** o PeliGames reúne num só lugar os jogos da Steam, do Heroic, do Lutris, do Bottles e do Faugus, além dos que você instala por ele.
2. **Instalar jogos e programas do Windows:** escolha o instalador (`.exe` ou `.msi`), o nome, a pasta e a versão do Proton. O PeliGames prepara o prefixo, roda o instalador e encontra sozinho o executável do jogo. Se houver dúvida entre executáveis, ele pergunta.
3. **Launchers (EA app, Battle.net, Ubisoft Connect, Epic):** os jogos que você instala por dentro do launcher aparecem em **Programas**, para abrir direto. Ao adicionar um deles à biblioteca, ele ganha um perfil próprio, com prefixo, Proton e mods separados, sem duplicar os arquivos do launcher.
4. **Mods do Nexus Mods:** em **Mods → Nexus Mods**, adicione o jogo, escolha os mods no catálogo (ou importe um arquivo) e clique em **Instalar**. O PeliGames confere os requisitos, aplica o instalador FOMOD quando existe e implanta os arquivos de acordo com as regras do jogo, as mesmas usadas pelo Vortex. Ativar, desativar e remover mods não altera os arquivos originais do jogo. Veja os [jogos suportados](docs/supported-games.md).
5. **Problemas:** **Reportar erro** gera um relatório do jogo e pode enviá-lo direto ao fórum de suporte do Discord do PeliGames. Os erros do launcher também aparecem no sininho de notificações.

## Instalação

Baixe AppImage, RPM ou DEB em [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM e DEB instalam os binários `peligames` e `pelinstall` em `/opt/PeliGames`, com comandos em `/usr/bin` e atalhos nos diretórios de aplicativos do sistema. Pelo terminal, use `peligames` ou `pelinstall /caminho/programa.exe`.
- AppImage reúne os dois módulos. Dê permissão de execução e abra o arquivo. Ele funciona direto, mas também pode ser instalado no seu usuário, como fazem o Gear Lever e o AppManager.
- Para abrir um executável Windows no instalador: `./PeliGames.AppImage --install /caminho/programa.exe` (depois de instalado, `peligames --install /caminho/programa.exe`).

### Instalar o AppImage

Na primeira abertura, a tela de boas-vindas oferece **Instalar e abrir**. O mesmo botão fica em **Configurações → Ferramentas** para quem pulou essa etapa. A instalação não pede root nem FUSE:

- o conteúdo do AppImage é extraído em `~/.local/share/PeliGames/app` e roda direto de lá, o que abre mais rápido que o arquivo compactado;
- `~/.local/share/PeliGames/peligames` é o comando usado pelos atalhos, e `~/.local/bin/peligames` aponta para ele no terminal (um arquivo `peligames` seu nesse lugar nunca é substituído);
- o PeliGames e o Pelinstall aparecem no menu de aplicativos, na categoria Jogos; os atalhos de jogos e os links do Nexus passam a abrir a cópia instalada;
- o arquivo `.AppImage` baixado é removido depois da extração.

As atualizações baixam o AppImage novo, extraem ao lado e trocam a pasta `app`. A versão anterior fica em `app.previous` até a nova abrir e volta sozinha se a nova não iniciar.

Para desinstalar, apague `~/.local/share/PeliGames`, o link `~/.local/bin/peligames` e os atalhos `peligames-*.desktop` e `pelinstall-appimage.desktop` em `~/.local/share/applications`. Os dados do launcher ficam à parte, em `~/.config/PeliGames`.

Proton, UMU e Winetricks são baixados conforme necessário. Configurações, ferramentas, mods e dados da interface ficam em `~/.config/PeliGames`; para começar do zero, basta apagar essa pasta. O backend e os arquivos do mod ficam em `~/.config/PeliGames/Mod/DLSSNR`. O setup do mod, as teclas por jogo e os diretórios personalizados ficam em `~/.config/PeliGames/Mod/DLSSNR/config.json`; as preferências de inicialização do Neural Rendering ficam em `neural-startup.json` nessa mesma pasta. As preferências de capas e Proton permanecem em `~/.config/PeliGames/config.json`. Prefixos e jogos permanecem nos diretórios escolhidos pelo usuário. Capas do SteamGridDB exigem uma chave pessoal opcional, configurada em Preferências.

## Mods locais do Nexus

Conecte sua chave pessoal em **Conta Nexus** e escolha mods no **Catálogo**, ou importe arquivos ZIP, 7z e RAR pelo botão **Mod Local**. Para receber downloads do site, habilite **Usar PeliGames nos links Nexus** e escolha **Mod Manager Download**.

Contas gratuitas confirmam o download na página do Nexus; Premium permite download direto. Acompanhe a fila em **Downloads Nexus** e, na lista de mods, confira os requisitos antes de **Instalar**, **Ativar** ou **Desativar**.

Integração independente, sem afiliação ao Nexus Mods. Reporte erros ao PeliGames, não ao Nexus Mods.

O suporte depende do jogo e do formato do pacote. A chave fica no chaveiro do sistema; AppImage requer `secret-tool` e um chaveiro disponível. Use **Logs** ou **Reportar erro** para diagnosticar problemas.

Em **Configurações → Ferramentas**, crie um backup ZIP dos jogos com mods Nexus instalados ou restaure um ZIP salvo em outro disco. A opção de recuperação por jogo mantém uma pasta oculta `.peligames` no disco dos jogos, permitindo recuperar os registros ao escanear novamente após trocar de distribuição. Use **Encontrar mods no SSD** para procurar cópias de recuperação em uma biblioteca inteira. A `.config` continua sendo o armazenamento principal.

## DLSS 5 Neural Rendering em placas AMD (DLSSNR)

A aba **DLSS 5 NR** roda o modelo de Neural Rendering (NR) do DLSS 5 da NVIDIA em placas AMD Radeon. Quem faz esse trabalho são dois projetos independentes, que reimplementam a rede em shaders Vulkan, executados no próprio dispositivo Vulkan do jogo (DXVK / vkd3d-proton sob o Proton). O PeliGames baixa o backend certo para a sua placa, executa o instalador dele com as opções escolhidas na interface e guarda o modelo para as próximas instalações. As informações abaixo resumem a documentação desses projetos e explicam como o PeliGames os usa.

> Projetos experimentais (versões 0.0.x), sem afiliação com a NVIDIA nem com a AMD. **Não use em jogos online com anti-cheat**: DLLs injetadas podem levar ao banimento da conta. O instalador coloca arquivos na pasta do jogo; **Remover** apaga só o que ele instalou.

### Placas e backends

| Placa | Backend | Testado pelos autores |
|---|---|---|
| RX 9000 (RDNA 4) | [DLSSNR-AMD, de mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD) | RX 9070 XT, Mesa 26.2, GE-Proton |
| RX 7000 (RDNA 3) | [DLSSNR-RDNA3, de mauri870](https://github.com/mauri870/DLSSNR-RDNA3) (fork do anterior) | RX 7900 XTX, Mesa 26.2.3, GE-Proton 11-7 |

- O PeliGames identifica a série da placa e oferece o backend correspondente em **Configurações → Mods → Gerenciar backend**. O download vem das releases do GitHub de cada projeto e fica em `~/.config/PeliGames/Mod/DLSSNR/backend-files/<rdna3|rdna4>/<32|64>`.
- A RX 7000 não tem as instruções de matriz FP8 da RDNA 4. O fork reproduz a mesma rede com FP16 e chega à mesma fidelidade, mas custa cerca de 2,6 vezes mais por quadro.
- Requisitos: Mesa 26.2 ou mais novo (driver RADV) e um Proton recente. O instalador do backend usa `bash` e `python3`.

### O modelo

Os pesos são da NVIDIA e não acompanham o PeliGames nem os backends. Na primeira instalação, aponte o seu `nvngx_dlssnr.dll` **versão 310.8.0** (ou um zip que contenha só ele). O backend recusa outras versões e confere as 599 entradas extraídas contra hashes conhecidos. O PeliGames guarda o `dlssnr.bin` resultante em `~/.config/PeliGames/Mod/DLSSNR/` e o reaproveita nas instalações seguintes, sem pedir a DLL de novo.

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
| **int4 misto** | todas as rotas | todas as rotas | Segunda rede, de 7% a 11% mais rápida, com parte das contas em int4; a imagem muda um pouco. **Precisa das variáveis `VK_ADD_LAYER_PATH` e `VK_INSTANCE_LAYERS` nas opções de inicialização** (veja abaixo). **Ctrl+F11** alterna entre o int4 e a rede padrão. Com ele, o NR demora mais para entrar quando o jogo abre. |
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

O PeliGames instala mods do Nexus Mods em **202 jogos**, com módulos baseados nas extensões do Vortex: Fallout, Skyrim, Starfield, Cyberpunk 2077, Baldur's Gate 3, Stardew Valley, Elden Ring, a família Resident Evil e Monster Hunter, além de dezenas de jogos Unreal e Unity.

| Concluído | Jogos |
|---|---|
| 100% — verificado em jogo real | 5 |
| 90% — paridade com a extensão Vortex | 67 |
| 80% — recursos da extensão ainda pendentes | 29 |
| 70% — gerado do template da família | 100 |
| 60% — sem suporte Vortex confirmado | 1 |

**[Ver a lista completa de jogos suportados →](docs/supported-games.md)**

## Compilar a partir do código

1. Instale as dependências de sistema do Tauri 2:
   - Debian/Ubuntu: `sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`
   - Fedora: `sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel` e `sudo dnf group install c-development`
2. Instale o [Rust](https://rustup.rs) e o [Bun](https://bun.sh).
3. Baixe o código e as dependências da interface:
   ```
   git clone https://github.com/pelicanux/PeliGames.git
   cd PeliGames
   bun install
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
