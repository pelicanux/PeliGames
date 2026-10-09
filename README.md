# PeliGames

Launcher de jogos Windows para Linux com Proton, biblioteca de jogos, Pelinstall, ferramentas Wine, integração Nexus Mods e gerenciador de mods DLSSNR-AMD.

**0.3.0 — versão experimental para testes muito iniciais.** A publicação serve somente para testes e poderá ser removida posteriormente.

## Instalação

Baixe AppImage, RPM ou DEB em [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM e DEB instalam os binários `peligames` e `pelinstall` em `/opt/PeliGames`, com comandos em `/usr/bin` e atalhos nos diretórios de aplicativos do sistema. Pelo terminal, use `peligames` ou `pelinstall /caminho/programa.exe`.
- AppImage reúne os dois módulos. Dê permissão de execução e abra o arquivo. A primeira abertura registra o launcher e “Abrir com Pelinstall” para o usuário. Mantenha o AppImage no mesmo local para preservar os atalhos.
- Para abrir um executável Windows no instalador: `./PeliGames.AppImage --install /caminho/programa.exe`.

Proton, UMU e Winetricks são baixados conforme necessário. Configurações e ferramentas ficam em `~/.config/PeliGames`. O backend e os arquivos do mod ficam em `~/.config/PeliGames/Mod/DLSSNR`. O setup do mod, as teclas por jogo e os diretórios personalizados ficam em `~/.config/PeliGames/Mod/DLSSNR/config.json`; as preferências de inicialização do Neural Rendering ficam em `neural-startup.json` nessa mesma pasta. As preferências de capas e Proton permanecem em `~/.config/PeliGames/config.json`. As configurações antigas são separadas automaticamente, preservando os valores existentes. Prefixos e jogos permanecem nos diretórios escolhidos pelo usuário. Capas do SteamGridDB exigem uma chave pessoal opcional, configurada em Preferências.

## Mods locais do Nexus

Conecte sua chave pessoal em **Conta Nexus** e escolha mods no **Catálogo**, ou importe arquivos ZIP, 7z e RAR pelo botão **Mod Local**. Para receber downloads do site, habilite **Usar PeliGames nos links Nexus** e escolha **Mod Manager Download**.

Contas gratuitas confirmam o download na página do Nexus; Premium permite download direto. Acompanhe a fila em **Downloads Nexus** e, na lista de mods, confira os requisitos antes de **Instalar**, **Ativar** ou **Desativar**.

O suporte depende do jogo e do formato do pacote. A chave fica no chaveiro do sistema; AppImage requer `secret-tool` e um chaveiro disponível. Use **Logs** ou **Reportar erro** para diagnosticar problemas.

## Desenvolvimento e compilação

Requer Linux, Rust, Bun e as dependências de compilação do [Tauri 2](https://v2.tauri.app/start/prerequisites/), incluindo WebKitGTK 4.1 e libarchive (`libarchive-dev` no Debian/Ubuntu ou `libarchive-devel` no Fedora). Para RPM, instale também `rpm`, `rpmbuild` e `cpio`.

```sh
bun install --frozen-lockfile
bun run tauri dev
# Binários locais:
bash scripts/build-binaries.sh
# Binários + AppImage, RPM e DEB:
bash scripts/build-release.sh
```

Os scripts usam todos os threads disponíveis e um diretório temporário em `/tmp`. Para compilar em RAM, `/tmp` deve ser tmpfs. As saídas ficam em `Release/`. Os binários incluem a interface e não dependem de localhost.

## Créditos e licença

PeliGames é desenvolvido por Pelicano e distribuído sob a licença [MIT](LICENSE).

O gerenciador de mods integra os projetos [DLSSNR-AMD, de mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD), e [DLSSNR-RDNA3, de mauri870](https://github.com/mauri870/DLSSNR-RDNA3). As licenças e avisos desses componentes estão em [licenses/](licenses/README.md) e na janela Sobre do aplicativo.

O launcher utiliza Proton, UMU e Winetricks, mantidos por seus respectivos projetos. Esses componentes têm suas próprias licenças e são obtidos separadamente pelo aplicativo.

## Módulos de suporte a jogos

O gerenciador Nexus identifica a pasta pelos arquivos exigidos e pelo cabeçalho do executável. O nome do jogo e seu catálogo Nexus não habilitam instalação por conta própria. Os módulos incluídos ficam em `src-tauri/resources/nexus-games/`, com definições pesquisadas nas fontes dos autores e nas extensões de suporte do Vortex. As regras desses módulos já são utilizadas pelo instalador; os perfis e arquivos anteriormente gerenciados permanecem no mesmo formato.

Módulos locais adicionais são arquivos JSON em `~/.config/PeliGames/Mod/Nexus/modules` (ou no diretório de configuração definido por XDG). Eles são lidos nas consultas à biblioteca. Em **Configurações** do jogo, o launcher mostra o módulo identificado, sua versão, os destinos e eventuais erros ao carregar módulos. Definições inválidas são ignoradas e não substituem módulos já existentes para o mesmo jogo e plataforma.

Um módulo de instalação de arquivos declara:

- `schema: 1`, `version`, `domain` Nexus, `name` e `platform` (`native` ou `proton`).
- `engine: "deployment"`, `executable`, opcionalmente `launch_executable`, `required` (arquivos relativos à raiz) e `header` (bytes iniciais do executável: `[77, 90]` para PE ou `[127, 69, 76, 70]` para ELF).
- `steam_id` opcional, usado na detecção do prefixo e na execução.
- `frameworks` declara arquivos necessários, domínio/ID Nexus ou URL externa e os destinos/extensões que exigem cada carregador. `nexus_name` registra o título oficial, separado do nome traduzido ou abreviado; ao preparar arquivos/downloads, o título retornado pela API é conferido e uma divergência bloqueia o requisito. URLs Nexus explícitas também precisam corresponder ao domínio e ID cadastrados; `dll_overrides` habilita DLLs presentes na execução Proton. `notes` e `rejected_prefixes` expõem e aplicam os limites do módulo.
- `destinations` (diretórios relativos permitidos) e `allowed_files` opcionais (destinos de arquivos individuais).
- `routes`, avaliadas em ordem: `prefix` copia o conteúdo de uma pasta do pacote para um destino; `file` mapeia um arquivo específico; `extension` direciona arquivos pela extensão, preservando somente o nome do arquivo. `root_only: true` limita essa última regra à raiz do pacote. `manifest` identifica a raiz de um mod por `manifest.json`, `ModInfo.xml`, `SubModule.xml`, `content.xml` ou outro marcador declarado, mantendo os arquivos juntos numa pasta própria.

Exemplo genérico de rota: `{"kind":"prefix","source":"Content/","target":"Mods/{mod_id}/"}`. O destino precisa estar autorizado na definição do módulo. Prefixos e destinos de pastas terminam em `/`. Rotas podem usar `{mod_id}` no destino para isolar arquivos por mod. Nenhum caminho pode ser absoluto, conter `..` ou sair dos destinos permitidos; os executáveis e arquivos usados para identificar o jogo não podem ser destinos de instalação.

O motor de arquivos prepara um perfil, verifica conflitos e aplica somente os arquivos registrados, com reversão se a operação falhar. Aceita ZIP, 7z, RAR e arquivos de mods soltos nos formatos declarados, como FBMOD, FBPACK, PACKAGE e TS4SCRIPT. A aceitação do arquivo não dispensa a validação das regras do jogo. O leitor libarchive normaliza 7z/RAR em um cache validado dentro de `Mod/Nexus/games/<id>/archive-cache`; remover o mod também remove esse cache. Alguns módulos utilizam motores específicos e perfis isolados, conforme o carregador necessário. A integração atual não executa extensões JavaScript do Vortex, instaladores FOMOD por script C# ou scripts arbitrários de módulos. Jogos que dependem de alteração de bancos de dados, ordem de carregamento ou ferramentas especiais precisam de um motor específico antes de habilitar esses formatos. Um módulo precisa de destinos pesquisados e validados para o jogo; este mecanismo não deduz automaticamente as regras de todos os jogos do Nexus.

### Planejamento e limites da instalação

Ao instalar um arquivo importado, uma janela mostra os destinos e os carregadores ausentes. **Instalar** nas dependências usa o fluxo Nexus de seleção de arquivos e autorização/download. Instaladores XML FOMOD mostram as escolhas antes de aplicar os arquivos; cada alternativa instala somente o conteúdo selecionado.

Os destinos `@local` e `@documents` identificam pastas do usuário Windows no prefixo selecionado. Cópia e ativação participam da mesma transação de reversão. Caminhos que apontam para fora dos destinos permitidos são recusados.

Pacotes sem regras reconhecidas ou dependências presentes são recusados. Alguns formatos precisam de processamento por gerenciadores especializados, mesclagem de scripts, configuração de carregadores ou ferramentas externas. A identificação do jogo não significa suporte a todos os mods e versões disponíveis. Os testes automatizados usam arquivos e diretórios temporários e não substituem a validação dentro dos jogos.

### Pastas dos jogos no Nexus

Perfis, arquivos importados, configurações preservadas e downloads temporários ficam em `~/.config/PeliGames/Mod/Nexus/games/<nome-do-jogo>--<id>/`, com subpastas `archives`, `profiles`, `settings` e `downloads`. O identificador diferencia instalações do mesmo jogo; o nome da pasta permanece estável quando o título da biblioteca é editado.

As pastas antigas que continham apenas o ID são renomeadas automaticamente quando não há uma sessão em execução. Um link de compatibilidade mantém os caminhos antigos funcionando para perfis, backups e mods já aplicados. Os caminhos dos arquivos importados na biblioteca são atualizados. Se houver conflito entre uma pasta antiga e uma nova, nenhuma delas é sobrescrita. Os logs continuam em `Logs/Nexus/<nome>-<id>/erro`.
