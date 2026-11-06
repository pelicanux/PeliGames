# Licenças e avisos de terceiros

O código do PeliGames usa a [GNU GPL 3.0](../LICENSE) (`GPL-3.0-only`). Os componentes de terceiros distribuídos com o launcher mantêm as próprias licenças, e seus avisos originais acompanham os pacotes. No programa, eles ficam em **Sobre → Licenças**.

| Componente | Aviso |
| --- | --- |
| Bibliotecas Rust do launcher, PeliRun e overlay | `generated/Rust-Dependencies.txt`, gerado ao empacotar |
| React, Motion, i18next, react-i18next e APIs Tauri | `generated/Frontend-Dependencies.txt`, gerado ao empacotar |
| Código adaptado no overlay (vkBasalt/vkDaedalus, MangoHUD, Khronos) | [Atribuições](PeliGames-Overlay-NOTICES.md) |
| WebRTC e LiveKit, usados na transmissão de tela | [Avisos](LiveKit-WebRTC-Third-Party.md) |
| musl libc, no auxiliar estático do PeliRun | [Copyright](Musl-COPYRIGHT.txt) |
| Verificação dos instaladores (osslsigncode, OpenSSL e zlib) | `generated/Installer-Verifier-Notices.txt`; código do osslsigncode em `generated/osslsigncode-<versão>-source.tar.gz`, incluídos ao compilar |
| Bibliotecas nativas do AppImage | [Avisos dos pacotes](AppImage-Native-Notices.txt) |
| GDK modificada do AppImage | [Origem e fonte correspondente](../scripts/gtk3-wayland/README.md) |
| Fonte Jersey 10 | [SIL OFL 1.1](../public/fonts/jersey-10/OFL.txt) |
| Ícone do Nexus Mods, do Vortex | [GPL-3.0](upstream/Vortex-Nexus-Icon/LICENSE.md) |
| Ícones do Lutris e do Faugus Launcher | [Lutris](upstream/Lutris/LICENSE) e [Faugus](upstream/Faugus-Assets/LICENSE) |

`scripts/license-notices.sh` gera os avisos das dependências a partir das versões fixadas nos `Cargo.lock` e no `package.json`. Quando um pacote não traz o texto da licença, o gerador usa o texto original guardado em `upstream/<pacote>/`; se faltar algum, a compilação para.

O código-fonte das dependências MPL-2.0 está disponível, sem modificações, no endereço do crates.io indicado para cada versão nos avisos Rust.

Proton, UMU, Winetricks e os backends DLSSNR são baixados à parte e mantêm as próprias licenças; não fazem parte do pacote do PeliGames.

A instalação de EA app, Battle.net e Ubisoft Connect usa um verificador incorporado ao executável. O usuário não precisa instalar `osslsigncode` nem conceder acesso de administrador. A compilação prepara o verificador com OpenSSL e zlib estáticos; requer os cabeçalhos e bibliotecas estáticas desses componentes no ambiente de desenvolvimento. O CMake, quando ausente, é obtido localmente pelo script, sem instalação global. Downloads de compilação têm versões e SHA-256 fixados em `scripts/installer-verifier.json`.
