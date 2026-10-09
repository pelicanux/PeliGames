# PeliGames

Launcher de jogos Windows para Linux com Proton, biblioteca de jogos, Pelinstall, ferramentas Wine, integração Nexus Mods e gerenciador de mods DLSSNR-AMD.

**0.3.0 — versão experimental para testes muito iniciais.** A publicação serve somente para testes e poderá ser removida posteriormente.

## Instalação

Baixe AppImage, RPM ou DEB em [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM e DEB instalam os binários `peligames` e `pelinstall` em `/opt/PeliGames`, com comandos em `/usr/bin` e atalhos nos diretórios de aplicativos do sistema. Pelo terminal, use `peligames` ou `pelinstall /caminho/programa.exe`.
- AppImage reúne os dois módulos. Dê permissão de execução e abra o arquivo. A primeira abertura registra o launcher e “Abrir com Pelinstall” para o usuário. Mantenha o AppImage no mesmo local para preservar os atalhos.
- Para abrir um executável Windows no instalador: `./PeliGames.AppImage --install /caminho/programa.exe`.

Proton, UMU e Winetricks são baixados conforme necessário. Configurações e ferramentas ficam em `~/.config/PeliGames`. O backend e os arquivos do mod ficam em `~/.config/PeliGames/Mod/DLSSNR`. O setup do mod, as teclas por jogo e os diretórios personalizados ficam em `~/.config/PeliGames/Mod/DLSSNR/config.json`; as preferências de inicialização do Neural Rendering ficam em `neural-startup.json` nessa mesma pasta. As preferências de capas e Proton permanecem em `~/.config/PeliGames/config.json`. Prefixos e jogos permanecem nos diretórios escolhidos pelo usuário. Capas do SteamGridDB exigem uma chave pessoal opcional, configurada em Preferências.

## Mods locais do Nexus

Conecte sua chave pessoal em **Conta Nexus** e escolha mods no **Catálogo**, ou importe arquivos ZIP, 7z e RAR pelo botão **Mod Local**. Para receber downloads do site, habilite **Usar PeliGames nos links Nexus** e escolha **Mod Manager Download**.

Contas gratuitas confirmam o download na página do Nexus; Premium permite download direto. Acompanhe a fila em **Downloads Nexus** e, na lista de mods, confira os requisitos antes de **Instalar**, **Ativar** ou **Desativar**.

O suporte depende do jogo e do formato do pacote. A chave fica no chaveiro do sistema; AppImage requer `secret-tool` e um chaveiro disponível. Use **Logs** ou **Reportar erro** para diagnosticar problemas.

## Créditos e licença

O código próprio do PeliGames, desenvolvido por Pelicano, é distribuído sob a licença [GNU GPL 3.0](LICENSE) (`GPL-3.0-only`). Os componentes de terceiros mantêm suas respectivas licenças e autorias.

Ao distribuir o programa ou versões derivadas, disponibilize o código-fonte correspondente sob GPL 3.0, mantendo os avisos de autoria e licença. O programa é fornecido sem garantia, nos limites permitidos por lei.

| Componente | Autoria e licença |
| --- | --- |
| [DLSSNR-AMD](https://github.com/mochizuki0323/DLSSNR-AMD) e [DLSSNR-RDNA3](https://github.com/mauri870/DLSSNR-RDNA3) | mochizuki0323 e mauri870 — MIT; os avisos dos componentes internos acompanham os backends. |
| [React](https://github.com/facebook/react) e React DOM | Meta e colaboradores — MIT. |
| [Motion](https://github.com/motiondivision/motion) | Framer, Matt Perry e colaboradores — MIT. |
| [Tauri e plugins](https://github.com/tauri-apps) | The Tauri Programme in the Commons Conservancy e colaboradores — MIT ou Apache-2.0. |
| [Jersey 10](https://github.com/scfried/soft-type-jersey) | The Soft Type Project Authors — SIL Open Font License 1.1. |
| [Ícone Nexus Mods](public/nexus-mods.svg), obtido do [Vortex](https://github.com/Nexus-Mods/Vortex) | Nexus Mods / Black Tree Gaming e colaboradores — repositório de origem sob GPL-3.0; a marca pertence ao seu titular. |
| Bibliotecas Rust e componentes nativos Linux | Autores e licenças individuais, incluindo MIT, Apache-2.0, BSD, ISC, Unicode, MPL, GPL e LGPL; consulte os avisos completos. |

Os [textos de licença, copyrights e avisos de terceiros](licenses/README.md) são preservados separadamente. A licença GPL do PeliGames não substitui as licenças desses componentes nem concede direitos sobre marcas ou conteúdo de jogos.

Proton, UMU, Winetricks e os backends são obtidos separadamente pelo aplicativo e mantêm suas próprias licenças. A DLL e os pesos de modelos da NVIDIA não são incluídos no launcher.
