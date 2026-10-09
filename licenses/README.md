# Licenças e avisos de terceiros

O código próprio do PeliGames usa [MIT](../LICENSE). Dependências, fontes, imagens e ferramentas externas não são relicenciadas automaticamente pelo launcher. Créditos resumidos não substituem os textos e copyrights exigidos por cada licença.

| Componente | Avisos preservados |
| --- | --- |
| DLSSNR-AMD — mochizuki0323 | [MIT original](DLSSNR-AMD-LICENSE.txt) e [componentes do backend](DLSSNR-AMD-THIRD-PARTY.md) |
| DLSSNR-RDNA3 — mauri870, a partir de mochizuki0323 | [MIT original do fork](DLSSNR-RDNA3-LICENSE.txt); o copyright original de mochizuki0323 foi mantido |
| React, React DOM, Scheduler, Motion e APIs Tauri | [Avisos das dependências da interface](Frontend-Dependencies.txt) |
| Bibliotecas Rust, inclusive dependências transitivas e de compilação | [Avisos completos](Rust-Dependencies.txt) e [inventário por versão](dependency-inventory.json) |
| Jersey 10 — The Soft Type Project Authors | [SIL OFL 1.1 original](../public/fonts/jersey-10/OFL.txt) |
| SVG Nexus Mods copiado do Vortex | [GPL-3.0 do repositório de origem](upstream/Vortex-Nexus-Icon/LICENSE.md); [arquivo-fonte e origem](../public/nexus-mods.svg) |
| Bibliotecas nativas presentes no AppImage 0.3.0 | [Copyrights e avisos originais dos pacotes](AppImage-Native-Notices.txt) |
| Extração de ícones de executáveis — ico e png | [Avisos preservados](Executable-Icon-LICENSES.txt) |

O inventário Rust é conservador: inclui dependências de compilação e não afirma que todos os pacotes são redistribuídos no executável. Os documentos idênticos são deduplicados pelo hash SHA-256; cada componente aponta para os textos originais correspondentes. As expressões `OR` representam alternativas de licença, e `AND` exige a combinação indicada; preservamos os documentos disponibilizados pelo pacote, sem transformar essas expressões em uma licença única.

As fontes originais das dependências MPL-2.0 estão em [sources/](sources/README.md). Os textos adicionais obtidos dos repositórios oficiais têm suas URLs e referências registradas em [upstream/sources.json](upstream/sources.json).

**Pendências e limites da verificação:** consulte a [auditoria](AUDIT.md). A presença destes avisos, por si só, não certifica o cumprimento de todas as obrigações de distribuição GPL/LGPL do AppImage.

Proton, UMU, Winetricks e os backends são baixados separadamente. Seus pacotes e componentes internos mantêm as próprias licenças. Os avisos do backend não demonstram que esses componentes estão dentro do binário PeliGames. A DLL e os pesos de modelos da NVIDIA não são incluídos nem cobertos pelo MIT do launcher.
