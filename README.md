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

## Créditos e licença

PeliGames é desenvolvido por Pelicano e distribuído sob [GNU GPL 3.0](LICENSE). Versões derivadas distribuídas devem disponibilizar o código-fonte correspondente sob a mesma licença.

Integra [DLSSNR-AMD, de mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD), e [DLSSNR-RDNA3, de mauri870](https://github.com/mauri870/DLSSNR-RDNA3).

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
