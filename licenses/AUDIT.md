# Auditoria de terceiros — 2026-10-09

Base: código e histórico Git disponíveis até `0de82a3c372359e9a1a59128742c4917967ce292`, arquivos de dependências fixados e AppImage Linux x86_64 0.3.0 disponível em `Release/`. Esta análise registra evidências de procedência e distribuição; não certifica conformidade jurídica completa nem prova a origem de arquivos sem registro histórico.

AppImage examinado: `PeliGames_0.3.0_amd64.AppImage`, SHA-256 `43c5bb35ffb72ac48fd2c0636d86a6b761b5a4da072516c0d228b7156b39ee8a`.

Após esta auditoria, o titular solicitou a adoção de GPL-3.0-only para o código próprio atual. LICENSE, manifests, créditos e empacotamento foram atualizados. Isso não revoga permissões MIT já concedidas para cópias anteriores, não altera os binários publicados e não elimina as pendências de terceiros abaixo. A distribuição de novos binários deverá disponibilizar o código-fonte correspondente, incluindo os scripts necessários à compilação, conforme a GPL 3.0.

## Escopo e evidências

- Foram examinados os commits disponíveis, comentários de procedência, manifests, lockfiles, assets e avisos existentes.
- O inventário reúne 430 pacotes Rust do grafo Linux com `custom-protocol`, incluindo dependências transitivas e de compilação, e 11 pacotes de produção da interface. Não corresponde a 441 bibliotecas necessariamente presentes no executável. Outras plataformas e ferramentas de desenvolvimento não foram inventariadas integralmente.
- Os textos originais encontrados foram preservados em [Rust-Dependencies.txt](Rust-Dependencies.txt) e [Frontend-Dependencies.txt](Frontend-Dependencies.txt), com referências por SHA-256 no [inventário](dependency-inventory.json). Avisos ausentes no pacote foram buscados no repositório oficial; as referências estão em [upstream/sources.json](upstream/sources.json).
- Foram extraídos 100 arquivos de copyright dos pacotes nativos incluídos no AppImage, além dos textos comuns de licença presentes na imagem. Estão em [AppImage-Native-Notices.txt](AppImage-Native-Notices.txt). O conjunto não constitui um inventário binário completo nem comprova disponibilidade de fontes.

## Uso identificado

| Material | Evidência e tratamento |
| --- | --- |
| DLSSNR-AMD e DLSSNR-RDNA3 | Integração com backends obtidos separadamente. Licenças MIT e avisos internos preservados; autoria original de mochizuki0323 e manutenção do fork por mauri870 distinguidas. |
| React, Motion, APIs Tauri e bibliotecas Rust | Dependências reais declaradas nos manifests e lockfiles. Textos, autores declarados e versões preservados no inventário. As licenças dessas dependências não são substituídas pela licença do launcher. |
| Jersey 10 | Fonte distribuída em `public/fonts/jersey-10/`, com copyright The Soft Type Project Authors e OFL 1.1 original. |
| `public/nexus-mods.svg` | O próprio arquivo registra cópia de `Nexus-Mods/Vortex/assets/images/nexus.svg`. O repositório de origem declara GPL-3.0; o texto foi preservado. A alteração local registrada é o dimensionamento responsivo. Não foi encontrada uma licença específica alternativa para esse asset; sua condição e o uso da marca precisam ser confirmados. |
| `src/assets/bg_main.jpg` e `public/peligames.png` | O responsável pelo projeto informou nesta auditoria que foram feitos com IA. Modelo, termos do serviço e eventuais imagens de entrada não foram informados; esse relato não atribui uma licença de terceiros ou direitos exclusivos por presunção. |
| Bibliotecas nativas do AppImage | GTK, GLib, WebKitGTK, GStreamer e outras bibliotecas acompanham o pacote. Há licenças permissivas e copyleft. O conjunto inclui código IJG/libjpeg e seus créditos originais. |

Os módulos de jogos contêm referências de pesquisa a Vortex e outros projetos para formatos, caminhos e regras de reconhecimento. Nos arquivos e commits examinados não foi identificada uma cópia explícita de código desses gerenciadores nos módulos. Isso é uma conclusão limitada às evidências disponíveis, não uma garantia de ausência de cópia. Referência funcional, dependência efetiva e reutilização de material protegido são situações diferentes; uma ideia não foi tratada como código redistribuído.

## Avisos e fontes disponibilizados nesta revisão

- Créditos resumidos no README e textos completos na janela Sobre, disponíveis offline na próxima compilação.
- Configuração de empacotamento para incluir o diretório `licenses/` nos próximos instaladores.
- Fontes originais dos cinco crates MPL-2.0: cssparser, cssparser-macros, dtoa-short, option-ext e selectors, nas versões fixadas. Os arquivos em [sources/](sources/README.md) mantêm os avisos originais e são oferecidos sob MPL-2.0. Os hashes do [manifesto](sources/manifest.json) correspondem aos checksums do Cargo.lock. Não foram identificadas modificações locais nesses crates.

Para código MPL compilado e distribuído, é necessário informar como obter as partes cobertas em formato fonte, mesmo quando não modificadas; estas cópias tratam desse conjunto de crates, não das bibliotecas nativas. Referência: [Mozilla, FAQ MPL 2.0, Q8–Q10](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).

## Pendências de distribuição

1. **AppImage e código-fonte correspondente:** os avisos demonstram presença de componentes GPL/LGPL, incluindo `libjbig0` sob GPL-2.0-or-later. Ainda é necessário identificar as versões binárias exatas, reunir fontes, patches e instruções correspondentes e escolher uma forma válida de disponibilização para cada licença aplicável. Também falta conferir as condições de substituição/religação das bibliotecas LGPL e a combinação das licenças na cadeia de dependências. Copiar copyrights não resolve essas obrigações. Consulte os textos [GPL 2](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html) e [LGPL 2.1](https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html), especialmente suas condições de redistribuição de binários.
2. **Runtime AppImage:** a procedência, versão exata, avisos e fontes do runtime anexado ao AppImage precisam de verificação própria. O inventário das bibliotecas dentro do filesystem da imagem não cobre automaticamente esse runtime.
3. **Asset Nexus/Vortex:** confirmar a licença/autorização específica do ícone e as condições de redistribuição da cópia modificada, ou substituí-lo por material com autorização clara. Incluir a GPL do repositório e o crédito não resolve sozinho o enquadramento desse asset e não implica uma relicença automática de todo o código próprio.
4. **sigchld 0.2.5:** o manifest declara MIT e autor Jack O'Connor, mas não foi encontrado o texto original de licença/copyright no crate nem no commit upstream `0f279e73cefdac2cc66f19357df30a2682fb1570`. A ausência está marcada no inventário. É necessário esclarecer o aviso com a origem; não foi inventado um copyright.
5. **Arte gerada por IA:** manter a identificação do serviço e seus termos aplicáveis, além da origem de eventuais referências usadas na geração, caso seja necessária uma verificação de autorização dessa arte.
6. **Artefatos já publicados:** esta revisão não recompilou nem substituiu os binários da release 0.3.0. Os novos avisos e fontes precisam acompanhar uma nova distribuição; o commit de documentação, sozinho, não atualiza arquivos já baixados. DEB/RPM dependem de bibliotecas do sistema de forma diferente do AppImage e precisam de conferência específica do pacote final.

Proton, UMU, Winetricks e componentes internos dos backends são baixados separadamente. Isso não dispensa preservar os avisos dos pacotes redistribuídos, mas suas licenças não foram atribuídas ao binário PeliGames sem evidência de inclusão. A DLL e os pesos NVIDIA não são distribuídos pelo launcher.

## Reproduzir o inventário

Gerar `cargo metadata` a partir do Cargo.lock, com a plataforma Linux e a feature `custom-protocol`, e executar `python3 scripts/generate-license-notices.py caminho/metadata.json` com as dependências locais instaladas. O script preserva os textos encontrados; ele não substitui a revisão das condições de cada licença nem a inspeção do pacote final. Atualizações de dependências, assets ou empacotamento exigem nova conferência.
