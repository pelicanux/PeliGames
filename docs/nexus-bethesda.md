# Jogos Bethesda nos módulos Nexus

Atualizado em 10 de outubro de 2026.

Suporte aos jogos Creation Engine e Gamebryo seguindo as extensões públicas do Vortex. Nenhum destes recursos foi testado em jogo real; `runtime_verified` continua `false`. Atualizado em 11 de outubro de 2026 com arquivos BSA, ENB/dinput e New Vegas.

## Script extender

O campo `script_extender` (SKSE64, SKSE, F4SE, xNVSE, FOSE, xOBSE, SFSE, SKSEVR, F4SEVR e OBSE64) faz quatro coisas:

- **Raiz do jogo:** o loader (`skse64_loader.exe` etc.) e as DLLs com o prefixo do extender (`skse64_*.dll`) vão para a raiz, também dentro de um invólucro de até uma pasta. O código-fonte do pacote não é implantado.
- **Iniciar com mods:** com o loader instalado, o jogo é aberto por ele (`mod_launch_executable`).
- **Requisito:** plugins na pasta do extender (`Data/SKSE/Plugins` etc.) exigem o extender, com link para a página Nexus.
- **Logs:** os logs do extender em `Documentos/My Games/...` entram nos logs do loader.

No Oblivion Remastered, o OBSE64 (página 282) fica em `OblivionRemastered/Binaries/Win64`, com plugins em `OBSE/Plugins` dessa pasta. O campo `folder` do extender indica essa pasta; o padrão é a raiz do jogo. Como na extensão Vortex, vale só para a edição Steam.

As páginas Nexus foram conferidas pelo nome na API pública. Quando o extender fica em outro jogo Nexus (Enderal usa os do Skyrim), o campo `domain` aponta para lá e a página vira só um link: ela não confirma a identidade de um download do jogo.

### Falta, versão e erros (script-extender-installer e script-extender-error-check do Vortex)

- **Extender faltando:** sem o loader na pasta do jogo, as verificações do painel expandido avisam com o nome do extender, a versão do executável do jogo e a loja (pelos arquivos Steam/GOG/Epic). “Baixar script extender” abre o fluxo de requisitos (escolha do arquivo e download); o Vortex escolhe sozinho o arquivo cuja descrição cita a versão do jogo, aqui a versão aparece para a escolha.
- **Erros:** depois que o jogo fecha, os logs do extender (`skse64.log`, `nvse.log`, `f4se.log`… e o `_editor.log`) são lidos; plugins “reported as incompatible…”, “disabled, fatal error…” e “couldn't load plugin … (Error 126/193)” aparecem nas verificações e na linha do mod dono da DLL. “Ignorar” esconde até o log ser escrito de novo.
- **Não se aplica:** a correção do caminho da ferramenta de início (aqui o loader é sempre o da pasta do jogo) e a busca de versão nova no GitHub, que o Vortex só faz para extenders sem página no Nexus (nenhum da família); a atualização vem da checagem geral de mods, que o PeliGames ainda não tem.

## Ajustes de INI (arquivos soltos)

O campo `ini_settings` grava `[Archive] bInvalidateOlderFiles=1` e `sResourceDataDirsFinal=` no mesmo INI que o `gamebryo-archive-invalidation` do Vortex usa:

- **INI principal:** Skyrim.ini (Skyrim e Skyrim SE), Fallout4.ini, Fallout.ini (Fallout 3 e New Vegas), Oblivion.ini e Enderal.ini.
- **INI Custom:** StarfieldCustom.ini, SkyrimVR.ini e Fallout4Custom.ini (Fallout 4 VR) são criados se não existirem.
- **INI que o jogo cria:** se ainda não existe, a instalação para e pede para iniciar o jogo uma vez.
- **Quando grava:** só com mods instalados. Depois que o arquivo passa a ser gerenciado, ele continua nos perfis seguintes, sem mudanças quando não há mais mods; assim, a remoção do último mod não apaga o INI do jogador. O arquivo é lido do prefixo a cada implantação, então edições manuais são preservadas.

## Arquivos BSA/BA2

Como a extensão `gamebryo-archive-invalidation` do Vortex (campo `archives`):

- **Data dos arquivos originais:** a cada implantação, os BSA/BA2 do próprio jogo (pelos prefixos do Vortex, por exemplo `Fallout - `, `DeadMoney -`) mais novos que a data do jogo recebem essa data (2006-02-01 para Oblivion e Fallout 3/NV, 2008-11-01 para Skyrim e Fallout 4). Só a data de modificação muda; assim os arquivos soltos dos mods têm prioridade.
- **Redirecionamento de BSA** (Oblivion, Fallout 3 e New Vegas): opção no painel expandido, desligada por padrão como no Vortex. Ligada, a implantação gera `PeliGames - BSA Redirection.bsa` (formato 103/104, com um `dummy/dummy.dds` vazio) e o coloca primeiro em `SArchiveList`, com `bInvalidateOlderFiles=1` e `bUseArchives=1`. Desligada, o arquivo sai e a lista volta sem ele. Mudar a opção implanta o perfil de novo.

## Tipos ENB e dinput

Como `modtype-enb` e `modtype-dinput` do Vortex (campo `root_packages`): um pacote com `enbseries.ini` ou `dinput8.dll` vai para a pasta do jogo, relativo a esse arquivo (pastas `enbseries`, `enbcache`, `reshade-shaders`; arquivos `.dll`, `.ini`, `.fx`, `.fxh`). Duas variantes no mesmo pacote são recusadas. O Proton carrega `d3d9.dll` (Oblivion, Fallout 3/NV, Skyrim, Enderal), `d3d11.dll` (Skyrim SE/VR, Enderal SE, Fallout 4/VR) ou `dxgi.dll` (Starfield) e `dinput8.dll` do jogo quando existem. Arquivos originais continuam protegidos pela substituição com consentimento.

## Fallout New Vegas

- **Ferramentas:** FNVEdit, Wrye Bash e FNV 4GB Patcher, como na extensão do Vortex.
- **4GB Patcher:** o instalador do Vortex coloca `FNVpatch.exe`, `FalloutNVpatch.exe` ou `Patcher.exe` na pasta do jogo; aqui o patcher é executado pela ferramenta.
- **Verificações** (extensão da comunidade "Fallout New Vegas Sanity Checks", site/mods/945, exigida pela do Vortex; o código não é público, as regras vêm da descrição):
  - `Data/FalloutNV_lang.esp` (plugin de tradução): pode ser deixado de lado, com cópia guardada na pasta do jogo no PeliGames, e restaurado;
  - `FalloutNV.exe` sem a marca de endereço grande (patch de 4 GB): link para o FNV 4GB Patcher (newvegas/mods/62552);
  - `nvse_loader.exe` com versão menor que 6 (NVSE antigo do silverlock): link para o xNVSE;
  - um `.override` vazio ao lado de cada BSA de mod, para ele ter prioridade sobre os anteriores.
  - Ficam de fora a configuração padrão do GECK (conteúdo não publicado) e os avisos de Windows (Base Address Randomization, driver AMD).
- As verificações aparecem no painel expandido; o botão "Mostrar mais opções" ganha um marcador quando há algo pendente.

## Edições

| Jogo | Nexus | Identificação | Lista de plugins |
|---|---|---|---|
| Skyrim VR | skyrimspecialedition | `SkyrimVR.exe`, Steam 611670 | `Skyrim VR/plugins.txt` (asterisco) |
| Fallout 4 VR | fallout4 | `Fallout4VR.exe`, Steam 611660 | `Fallout4VR/plugins.txt` (asterisco) |
| Enderal | enderal | `TESV.exe` + `Enderal Launcher.exe`, Steam 933480 | `enderal/plugins.txt` |
| Enderal SE | enderalspecialedition | `SkyrimSE.exe` + `Enderal Launcher.exe`, Steam 976620 | `Enderal Special Edition/plugins.txt` (asterisco) |

- **Mods das edições VR:** vêm da página do jogo normal, como no Vortex (`nexusPageId`). Plugins `.esl` exigem o Skyrim VR ESL Support (106712) ou o Daytripper 4 (91141). O arquivo de cada um foi confirmado na prévia pública de arquivos do Nexus.
- **Fonte do Enderal SE:** a extensão é publicada no Nexus (site/mods/218). O código comparado é o do repositório público do autor, que pode estar atrás da versão publicada. O Vortex não aplica invalidação de arquivos a ele.
- **Adiados:**
  - **Nehrim:** a extensão do Vortex devolve a pasta do Oblivion, o que torna a identidade ambígua.
  - **Fallout London:** é um ajudante do Fallout 4 (importa os arquivos GOG como mod e reescreve as listas de BA2 no INI), não uma edição com pasta própria.

### Descoberta de jogos derivados

Quando mais de um módulo reconhece a pasta:

1. O manifesto Steam decide, se apontar um único módulo.
2. Se não decidir, vence o módulo cuja identidade contém estritamente a dos outros. O Enderal tem tudo o que o Skyrim exige mais o launcher, então funciona também fora do Steam.

Identidades iguais continuam proibidas. A inclusão de uma identidade em outra só é aceita para pares declarados no teste `no_module_identity_is_contained_in_another`.

## Pacotes BAIN

Um pacote BAIN é um arquivo em que todas as pastas de topo seguem o formato `NN Nome` (dois dígitos e um espaço). Os arquivos soltos no topo são tratados como documentação. Ele aparece como um grupo de opções no fluxo do FOMOD, reaproveitando a escolha, a confirmação e a reimplantação:

- **Seleção:** só o primeiro pacote vem marcado, como no instalador BAIN do Vortex para Morrowind (extensão da comunidade, site/mods/1295).
- **Destino:** o conteúdo é relativo a `Data`; uma pasta `Data/` explícita também é aceita.
- **Prioridade:** os pacotes são aplicados na ordem dos números, e um pacote posterior substitui arquivos de um anterior.
- **Limites:** o `wizard.txt` não é executado (o aviso aparece no nome do instalador). Conteúdo funcional fora dos pacotes recusa a instalação.
- **Abrangência:** vale só para jogos com ativação Bethesda ou Morrowind.

## Condições de FOMOD

- **Arquivos:** as condições "arquivo existe" são relativas à pasta de mods do jogo, como no Vortex: `Data` (ou `Data Files` no Morrowind).
- **Plugins:** as condições sobre plugins usam a ordem de carga.
  - *Active*: o plugin está ativo na lista de plugins ou é ativado implicitamente pelo jogo (`Skyrim.esm`, DLCs). Os implícitos vêm da `libloot`.
  - *Inactive*: o plugin está instalado (no jogo ou no perfil do PeliGames), mas desativado.
  - *Missing*: o plugin não está instalado.
- **Leitura da lista:** a consulta só é feita quando um FOMOD pergunta sobre um plugin. A lista é lida do arquivo real; a `libloot` sozinha só encontra a grafia `Plugins.txt`.

## Grafia de arquivos no prefixo

No Windows, `Plugins.txt` e `plugins.txt` são o mesmo arquivo; no Linux, não. Para caminhos em AppData e Documentos, o PeliGames usa a grafia que já existe no disco, segmento por segmento e só quando há uma única correspondência. Assim, uma lista criada pelo jogo com outra grafia é atualizada, em vez de a implantação ser recusada como ambígua. Se as duas grafias existirem, a recusa continua.

## Página de plugins

Como o `gamebryo-plugin-management` do Vortex: botão **Plugins** no painel expandido (`nexus_plugins.rs`, `NexusPlugins.tsx`).

- **Lista:** todos os plugins da pasta Data na ordem de carga, lida pela `libloot` a partir de uma cópia do `plugins.txt` com as duas grafias (no Linux a biblioteca só acha uma). Mostra índice (`0A`, `FE:001` para light, `FD:01` para medium), mod de origem ou “Fora do PeliGames”, marcas (do jogo, Master, Light, Medium, carrega BSA, grupo do LOOT), masters e masters faltando.
- **Ativar/desativar:** cada plugin, inclusive os de fora do PeliGames. Os plugins do próprio jogo (lista `nativePlugins` do Vortex, campo `plugins.native`) ficam sempre ativos. Nos jogos com asterisco o plugin desativado fica na lista sem `*`; nos outros, sai da lista.
- **Ordem:** com “Ordenar com LOOT automaticamente” (ligado por padrão, como no Vortex) o LOOT ordena a cada mudança e implantação, se a masterlist foi baixada. Desligado, a ordem é manual (subir/descer) e a implantação a respeita. “Ordenar agora” faz uma ordenação LOOT avulsa.
- **Jogos por data** (Oblivion, Fallout 3, New Vegas, Skyrim LE, Enderal): a ordem vira data de modificação, um minuto por plugin a partir do mais antigo da lista, para todos os plugins da lista, como a libloadorder faz.
- **Mensagens do LOOT** (com masterlist): avisos e erros, plugin sujo (ITM/UDR/navmesh e ferramenta), incompatíveis ativos e requisitos ausentes.
- **Limite:** 255 plugins comuns (254 com light, 253 com medium), 4096 light e 256 medium, como o teste do Vortex.
- **Escolhas:** `plugins.json` na pasta do jogo no PeliGames (desativados, ordem manual, ordenação automática, ghost, marca light).
- **Regras e grupos (userlist):** nos detalhes do plugin, regras “carregar depois de”, “requer” e “incompatível com”, escolha do grupo e criação de grupo que carrega depois de outro. Ficam em `userlist.yaml` na pasta do jogo no PeliGames, gravada pela `libloot`, e entram na ordenação do LOOT e nas mensagens.
- **Ghost:** esconde o plugin do jogo como `<nome>.ghost`, como no Vortex. Plugin de fora do PeliGames é renomeado na pasta Data; plugin de mod é renomeado na cópia do perfil e o perfil é implantado de novo. Plugin com ghost conta como desativado.
- **Marcar como light/comum** (Skyrim SE, Enderal SE, Fallout 4 e Starfield): só para plugins de mods; a marca (0x200, ou 0x100 no Starfield) é gravada no cabeçalho da cópia do perfil, nunca no arquivo do pacote nem em plugins de fora, e o perfil é implantado de novo. “Voltar ao original” desfaz.

## Página de saves

Como o `gamebryo-savegame-management` do Vortex: botão **Saves** no painel expandido (`nexus_saves.rs`, `NexusSaves.tsx`).

- **Leitura:** cabeçalho dos saves de Oblivion (`TES4SAVEGAME`), Skyrim LE/SE/VR e Enderal (`TESV_SAVEGAME`, lista de plugins no corpo comprimido em LZ4/zlib, do qual só o começo é descomprimido), Fallout 3/New Vegas (`FO3SAVEGAME`, campos separados por `|`) e Fallout 4 (`FO4_SAVEGAME`). Starfield fica de fora, como no Vortex.
- **Lista:** personagem, nível, local, tempo de jogo, data, número e plugins; os plugins ausentes ou desativados na ordem atual ficam em destaque. A miniatura é lida só ao abrir o save (PNG).
- **Ações:** restaurar os plugins do save (ativa os dele e desativa os outros na página de plugins; com ordem manual, segue a ordem do save), apagar (com o co-save `.skse`/`.nvse`…; uma cópia fica em `deleted-saves/` na pasta do jogo no PeliGames) e transferir saves de outro perfil ou dos compartilhados (copiar ou mover, sem sobrescrever).
- **Por perfil:** ver “Perfis de mods” em `NEXUS_GAME_SUPPORT.md` (saves próprios e configurações próprias).

## Ordenação LOOT

- **Ativação:** é opcional. O botão "Baixar masterlist" nas configurações do jogo baixa `masterlist.yaml` e `prelude.yaml` do branch `v0.29` (licença CC0). A `libloot` 0.29 (GPL-3.0-or-later, compatível com a licença do PeliGames) valida os arquivos antes de substituir o cache em `Mod/Nexus/loot/<repositório>`.
- **Masterlists:** o mapeamento é o mesmo do LOOT. As edições VR usam `skyrimse` e `fallout4`; Enderal e Enderal SE usam `enderal`. O Oblivion Remastered fica de fora.
- **Ordenação:** com a masterlist presente, cada implantação ordena as linhas do `plugins.txt` (as externas também), preservando o asterisco de cada linha. As cópias preparadas dos mods têm prioridade sobre a pasta `Data` do jogo.
- **Falhas:** um ciclo, um plugin ausente ou uma masterlist inválida mantêm a ordem atual e registram o motivo no log do jogo. A implantação não é bloqueada.
- **Limites:** sem visualização em grafo das regras nem histórico de ordem, que o Vortex tem. O Skyrim LE e o Enderal LE recebem as datas, mas não o `loadorder.txt` (usado só por ferramentas externas).

## Testes

- **Sintéticos:**
  - `archive_invalidation_follows_mods_and_keeps_the_managed_ini`;
  - `script_extenders_install_at_root_launch_and_are_required`;
  - `bethesda_editions_are_told_apart_by_executable_or_steam_id`;
  - `enderal_special_edition_is_told_apart_from_skyrim_special_edition`;
  - os dois testes de `nexus_bain`;
  - `masterlist_rules_reorder_listed_plugins_and_keep_their_flags`.
  - `enb_and_dinput_packages_go_to_the_game_folder`; os testes de `nexus_gamebryo` (hash TES4, BSA de redirecionamento, lista de arquivos, datas);
  - os testes de `nexus_plugins` (linhas da implantação, datas), `new_vegas_plugins_page_enables_orders_and_dates_plugins` e `userlist_rules_groups_ghost_and_light_flag` (ignorados; página completa em NV e Skyrim SE de fixture);
  - `new_vegas_archives_get_overrides_backdate_and_redirection` (ignorado; implantação completa com `XDG_CONFIG_HOME` isolado em `/tmp/peligames-*`).
- **Com dados reais:** `published_masterlists_parse_with_this_libloot` (ignorado) carrega as 8 masterlists publicadas: `PELIGAMES_LOOT_CACHE=<pasta> cargo test published_masterlists -- --ignored`.
