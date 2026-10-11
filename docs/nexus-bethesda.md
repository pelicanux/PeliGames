# Jogos Bethesda nos módulos Nexus

Atualizado em 10 de outubro de 2026.

Suporte aos jogos Creation Engine e Gamebryo seguindo as extensões públicas do Vortex. Nenhum destes recursos foi testado em jogo real; `runtime_verified` continua `false`.

## Script extender

O campo `script_extender` (SKSE64, SKSE, F4SE, xNVSE, FOSE, xOBSE, SFSE, SKSEVR e F4SEVR) faz quatro coisas:

- **Raiz do jogo:** o loader (`skse64_loader.exe` etc.) e as DLLs com o prefixo do extender (`skse64_*.dll`) vão para a raiz, também dentro de um invólucro de até uma pasta. O código-fonte do pacote não é implantado.
- **Iniciar com mods:** com o loader instalado, o jogo é aberto por ele (`mod_launch_executable`).
- **Requisito:** plugins na pasta do extender (`Data/SKSE/Plugins` etc.) exigem o extender, com link para a página Nexus.
- **Logs:** os logs do extender em `Documentos/My Games/...` entram nos logs do loader.

As páginas Nexus foram conferidas pelo nome na API pública. Quando o extender fica em outro jogo Nexus (Enderal usa os do Skyrim), o campo `domain` aponta para lá e a página vira só um link: ela não confirma a identidade de um download do jogo.

## Ajustes de INI (arquivos soltos)

O campo `ini_settings` grava `[Archive] bInvalidateOlderFiles=1` e `sResourceDataDirsFinal=` no mesmo INI que o `gamebryo-archive-invalidation` do Vortex usa:

- **INI principal:** Skyrim.ini (Skyrim e Skyrim SE), Fallout4.ini, Fallout.ini (Fallout 3 e New Vegas), Oblivion.ini e Enderal.ini.
- **INI Custom:** StarfieldCustom.ini, SkyrimVR.ini e Fallout4Custom.ini (Fallout 4 VR) são criados se não existirem.
- **INI que o jogo cria:** se ainda não existe, a instalação para e pede para iniciar o jogo uma vez.
- **Quando grava:** só com mods instalados. Depois que o arquivo passa a ser gerenciado, ele continua nos perfis seguintes, sem mudanças quando não há mais mods; assim, a remoção do último mod não apaga o INI do jogador. O arquivo é lido do prefixo a cada implantação, então edições manuais são preservadas.

O redirecionamento de BSA e o ajuste de data dos BSA, que o Vortex também faz para Oblivion e Fallout 3/NV, não foram implementados.

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

## Ordenação LOOT

- **Ativação:** é opcional. O botão "Baixar masterlist" nas configurações do jogo baixa `masterlist.yaml` e `prelude.yaml` do branch `v0.29` (licença CC0). A `libloot` 0.29 (GPL-3.0-or-later, compatível com a licença do PeliGames) valida os arquivos antes de substituir o cache em `Mod/Nexus/loot/<repositório>`.
- **Masterlists:** o mapeamento é o mesmo do LOOT. As edições VR usam `skyrimse` e `fallout4`; Enderal e Enderal SE usam `enderal`. O Oblivion Remastered fica de fora.
- **Ordenação:** com a masterlist presente, cada implantação ordena as linhas do `plugins.txt` (as externas também), preservando o asterisco de cada linha. As cópias preparadas dos mods têm prioridade sobre a pasta `Data` do jogo.
- **Falhas:** um ciclo, um plugin ausente ou uma masterlist inválida mantêm a ordem atual e registram o motivo no log do jogo. A implantação não é bloqueada.
- **Limites:** não há userlist, mensagens do LOOT nem interface de ordem manual. O Skyrim LE e o Enderal LE não recebem `loadorder.txt` nem ajuste de datas.

## Testes

- **Sintéticos:**
  - `archive_invalidation_follows_mods_and_keeps_the_managed_ini`;
  - `script_extenders_install_at_root_launch_and_are_required`;
  - `bethesda_editions_are_told_apart_by_executable_or_steam_id`;
  - `enderal_special_edition_is_told_apart_from_skyrim_special_edition`;
  - os dois testes de `nexus_bain`;
  - `masterlist_rules_reorder_listed_plugins_and_keep_their_flags`.
- **Com dados reais:** `published_masterlists_parse_with_this_libloot` (ignorado) carrega as 8 masterlists publicadas: `PELIGAMES_LOOT_CACHE=<pasta> cargo test published_masterlists -- --ignored`.
