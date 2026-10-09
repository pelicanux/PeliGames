# PeliGames

Launcher de jogos Windows para Linux com Proton, biblioteca de jogos, Pelinstall, ferramentas Wine, integração Nexus Mods e gerenciador de mods DLSSNR-AMD.

**0.3.0 — versão experimental para testes muito iniciais.** A numeração do novo launcher começa em 0.3.0, separada da linha antiga 0.8.x. A publicação serve somente para testes e poderá ser removida posteriormente.

## Instalação

Baixe AppImage, RPM ou DEB em [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM e DEB instalam os binários `peligames` e `pelinstall` em `/opt/PeliGames`, com comandos em `/usr/bin` e atalhos nos diretórios de aplicativos do sistema. Pelo terminal, use `peligames` ou `pelinstall /caminho/programa.exe`.
- AppImage reúne os dois módulos. Dê permissão de execução e abra o arquivo. A primeira abertura registra o launcher e “Abrir com Pelinstall” para o usuário. Mantenha o AppImage no mesmo local para preservar os atalhos.
- Para abrir um executável Windows no instalador: `./PeliGames.AppImage --install /caminho/programa.exe`.

Proton, UMU e Winetricks são baixados conforme necessário. Configurações e ferramentas ficam em `~/.config/PeliGames`. O backend e os arquivos do mod ficam em `~/.config/PeliGames/Mod/DLSSNR`. O setup do mod, as teclas por jogo e os diretórios personalizados ficam em `~/.config/PeliGames/Mod/DLSSNR/config.json`; as preferências de inicialização do Neural Rendering ficam em `neural-startup.json` nessa mesma pasta. As preferências de capas e Proton permanecem em `~/.config/PeliGames/config.json`. As configurações antigas são separadas automaticamente, preservando os valores existentes. Prefixos e jogos permanecem nos diretórios escolhidos pelo usuário. Capas do SteamGridDB exigem uma chave pessoal opcional, configurada em Preferências.

## Mods locais do Nexus

O grid Nexus exige catálogo e suporte de jogo confirmado no Vortex. A validação cruza os IDs Nexus com o manifesto público de extensões do Vortex e os domínios verificados das extensões embutidas. Uma lista local datada permite uso offline; registros antigos sem confirmação não são presumidos compatíveis. Jogos ocultos continuam salvos, com seus mods preservados. Esse critério não equivale ao filtro de compatibilidade de cada mod do site nem garante suporte a todos os pacotes: a instalação também valida o módulo e o formato do arquivo.

A aba **Conta Nexus** permite validar uma chave pessoal para testes locais, verificar a conexão e desconectar. A chave é guardada pelo `secret-tool` no chaveiro do sistema (Secret Service), sem ser salva nos arquivos do launcher. Para AppImage ou binário avulso, instale `libsecret-tools` no Debian/Ubuntu ou `libsecret` no Fedora e mantenha um serviço de chaveiro disponível. Os pacotes DEB/RPM declaram essa dependência. O login pelo navegador depende do registro do PeliGames no Nexus; a aba Catálogo consulta os mods em destaque, novos e atualizados, as descrições e os arquivos disponíveis. É possível abrir um mod pelo link ou ID e associar outros jogos ao domínio correspondente no Nexus. No painel Nexus, clique em **Usar PeliGames nos links Nexus** para associar `nxm://` ao executável atual (binário, DEB/RPM ou AppImage). No site, **Mod Manager Download** envia o arquivo ao launcher, que baixa e importa para o jogo associado, reutilizando a janela aberta. Contas gratuitas precisam do link temporário autorizado no site; contas Premium também podem usar links sem esse token. Se houver mais de uma instalação correspondente, escolha o destino no painel de downloads. Depois, confira os requisitos e clique em **Instalar** na aba Mods. Downloads manuais continuam disponíveis. Coleções, atualização automática de mods e SSO ainda não estão habilitados. O arquivo pode ter até 2 GiB; downloads incompletos e cancelados são removidos.

Ao escolher um arquivo no Catálogo ou clicar em Instalar em um requisito, o pop-up **Selecionar mods para baixar** reúne o principal e os requisitos Nexus recursivos, com caixas de seleção e escolha de uma versão de arquivo por mod. A fila contém até 16 mods, elimina ciclos/duplicações e mantém requisitos externos como links separados. Conta gratuita: confirme **Mod Manager Download** no navegador; a próxima página só abre quando o arquivo anterior foi baixado e importado. Premium usa download direto. Fechar o pop-up não interrompe a fila; ela pode ser acompanhada em **Downloads Nexus**. Erros pausam a seleção, e cancelar um item cancela seus downloads restantes. A fila dura a sessão do launcher. Os arquivos aparecem na aba Mods; a instalação e escolhas FOMOD continuam sendo confirmadas ali.

A biblioteca Nexus utiliza módulos com regras de instalação para os jogos compatíveis. O suporte pode atender edições Windows via Proton ou instalações nativas Linux, conforme o módulo identificado. Importe os arquivos ZIP, 7z ou RAR das dependências e dos mods; use **Instalar**, **Ativar** ou **Desativar**.

Os arquivos e perfis permanentes ficam em `~/.config/PeliGames/Mod/Nexus`. Alterações são bloqueadas enquanto o jogo está em execução. **Logs** mostra a execução atual e a anterior, incluindo os logs dos carregadores reconhecidos pelo módulo. ZIP, 7z e RAR são lidos com limites de extração, sem executar comandos do pacote. Instaladores XML FOMOD mostram as etapas e opções no pop-up de instalação, com seleção única ou múltipla, arquivos obrigatórios, flags e arquivos condicionais. As escolhas ficam salvas com o mod e são reutilizadas na ativação. Dependências baixadas com FOMOD permanecem importadas até confirmar as escolhas na aba Mods. A instalação continua limitada aos destinos reconhecidos pelo módulo do jogo; scripts C# e condições FOMOD de versão ou de estado de plugins ainda não são interpretados. A ativação de plugins é tratada conforme as regras do módulo do jogo.

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
