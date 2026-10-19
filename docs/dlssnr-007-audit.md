# Auditoria DLSSNR-AMD 0.0.7 — 19/10/2026

## Origem e escopo

- Mochizuki: [release v0.0.7](https://github.com/mochizuki0323/DLSSNR-AMD/releases/tag/v0.0.7), publicado em 19/10/2026 às 08:38:47 (America/Fortaleza), sem marca prerelease na API.
- [Comparação com v0.0.6-linux-preview](https://github.com/mochizuki0323/DLSSNR-AMD/compare/v0.0.6-linux-preview...v0.0.7), release apontando ao commit `cc27501`.
- [Instalador Linux auditado](https://github.com/mochizuki0323/DLSSNR-AMD/blob/v0.0.7/linux/package/install.sh). Não se aplicam ao launcher Linux as novas opções de ACO específicas de Windows.
- Instalação local consultada somente para leitura: marcador `v0.0.6-linux-preview-rdna4`. Nenhuma pasta de backend/modelo/jogo do usuário foi atualizada nesta auditoria.

## Contratos conferidos

| Área | Resultado e adaptação |
| --- | --- |
| Download | Release conjunta tem tar.gz Linux x86_64/i686 e zip Windows preview. O seletor existente já aceita os dois primeiros e rejeita o terceiro. Acrescentada regressão específica 0.0.7. |
| Versão | Continua tag do pacote em `backend-files/rdna4/version.txt`, com `-rdna4`, separada dos nomes de rede. Nenhuma substituição por nome de modelo ou Preview genérico. |
| Modelos | `dlssnr.bin` SHA-256 `2b41c888cf4155b8958c665ba64018ab0bd25c85fc71a2b6db86d0d04d1f7fbd` e `dlssnr-small.bin` SHA-256 `7e966e2639dbec6596aa0bac3b8761137d2c40ca8cb4aca30df7ca10043c6e33` permanecem iguais. Extração/cache validado existente compatível. int4 continua gerado pelo backend e validado por ele. |
| CLI | `--model` seleciona uma das três redes pelos nomes oficiais. Os aliases antigos existem, mas sua ordem importa: o fluxo anterior enviava `--int4` seguido de `--old-model`, sobrescrevendo int4 no 0.0.7. A integração nova usa um único `--model NOME`, sem aliases conflitantes. |
| Compatibilidade | Detecção pelo conteúdo do script, não pelo número de versão. Backends antigos/RDNA3 continuam com seus aliases e flags de instalação. A escolha de modelo sem pesos continua sendo recusada antes de executar o instalador. |
| Interface | 0.0.7 usa Rede inicial com pílulas Novo / Antigo / int4 e nomes oficiais nos balões. Remove a chave redundante int4 apenas nesse fluxo. O texto explica troca pelo menu NR/Ctrl+F11; backends antigos conservam instruções de Reparar. |
| Inicialização | Script imprime linha principal e, quando há pesos int4 e outra rede inicia, uma segunda linha completa com sua camada Vulkan. O parser antigo descartava essa alternativa. Agora prefere a alternativa oficial válida para permitir alternância durante o jogo, sem alterar a rede inicial. A sincronização existente com PeliRun conserva opções do usuário e a linha fixa da Steam. |
| Rotas | OptiScaler e nomes `reshade-feeder`, `reshade-feeder-vulkan`, `reshade-feeder-dx9` continuam com o mesmo contrato. |
| Configuração | Residency (auto/resident/offload/minimal), OSD, Engine data, log/atalhos e demais ajustes já são oferecidos pelos menus do backend. Não são reimplementados no launcher. Não se apagam nem reescrevem essas seções ao atualizar a integração. |
| LumeniteFX | Alternativa opcional de vetores para ReShade via `--lumenite`, download separado de um terceiro; não é requisito do 0.0.7. VORT continua padrão. Não foi introduzido novo download/redistribuição desse projeto nesta etapa. |

## Mudanças do backend que chegam ao reparar o mod

Troca de redes com preparação em segundo plano; gerenciamento de memória compartilhada e descarregamento dos pesos; diagnóstico de dados efetivamente recebidos da engine; grade Natural/Cinematic após a rede; amostragem dos vetores revisada; correções de resolução dinâmica, VRAM, texturas typeless e pontos/brilhos indevidos. Instalar o backend no launcher não substitui sozinho os arquivos já copiados nos jogos: é necessário Reparar / reinstalar a instalação desejada.

## Verificação

- 12 testes Rust passaram usando os módulos reais de CLI, modelos e seleção de assets em uma bancada isolada em RAM. Um dos testes usa o parser Bash do próprio instalador v0.0.7 para validar as três redes com RenoDX; executa apenas o trecho de argumentos, sem varredura, download ou escrita em jogo.
- Dois testes TypeScript passaram para a linha alternativa 0.0.7, preservação da linha antiga e rejeição de saída incompleta.
- TypeScript verificado com `tsc --noEmit`.
- Após autorização, binário e AppImage foram compilados no Ubuntu 24.04 com 12 tarefas. Os 14 testes desta integração e os 56 do overlay passaram novamente; o probe release validou 28 símbolos. Os payloads incorporados e a interface dos dois executáveis foram conferidos. Hashes e fontes estão em `Release/build-validation.json`. Temporários em RAM removidos.
- A instalação local do backend e os arquivos dos jogos não foram alterados. Falta o teste gráfico do DLSSNR; atualize o backend para 0.0.7 pela interface e repare a instalação desejada para testar.

## Rechecagem do release e da interface — 19/10/2026

A API oficial `/releases/latest` ainda retorna `v0.0.7`, publicado às 08:38:47 e com notas atualizadas às 14:25:22 (America/Fortaleza). Foram relidas as notas atuais e o `linux/package/install.sh` do tag, além de `InstallExtras`, `extra_arguments`, `get_neural_model_status` e `ModernDlssPanel` locais. Esta rechecagem não instalou dependências, não alterou jogos e não gerou pacotes.

- A escolha de rede inicial Novo / Antigo / int4 e seu mapeamento para `--model` já existem no launcher. RenoDX e Começar ligado também estão expostos.
- A opção de instalação ausente é `--lumenite` / `--no-lumenite`. A CLI também aceita um ZIP local após `--lumenite`. O instalador recusa essa opção na rota OptiScaler; aplica-se às três rotas ReShade. Sem a opção, o instalador não interativo usa VORT. Não há campo LumeniteFX em `InstallExtras` nem controle correspondente na interface.
- Para expor esse recurso, a proposta é um seletor compacto **Vetores de movimento: VORT / LumeniteFX**, visível nas rotas ReShade/Vulkan/DX9 e condicionado à capacidade do script instalado. Instalar/Reparar com LumeniteFX faria o download separado da revisão fixada pelo backend e instalaria seus arquivos. Alterar apenas o INI não substitui essa preparação. Após isso, o menu Add-ons permite alternar; o backend volta a VORT quando LumeniteFX não pode ser usado. A interface ainda não foi alterada nesta rechecagem.
- Residency, OSD, Engine data, Style Grade, Motion Without Depth, Resize History e ajustes de log/atalhos não criam requisitos novos de instalação: são oferecidos/configurados pelo backend. Não é obrigatório duplicar esses controles no launcher.
- As alterações ACO Mode descritas sob **Windows** não justificam uma nova chave no launcher Linux.

## Integração LumeniteFX no launcher — 19/10/2026

Implementado o seletor VORT / LumeniteFX no painel de opções por jogo, somente nas rotas ReShade/Vulkan/DX9 e quando o script oferece as duas flags. A escolha segue pelo payload TypeScript e `InstallExtras` até `--lumenite` / `--no-lumenite`; solicitações incompatíveis também são recusadas no Rust. Backends antigos preservam seu fluxo. VORT é o padrão em instalações novas; ao abrir uma instalação existente, a seleção é lida de `[DlssNr] MotionVectors` no INI do destino, com leitura limitada a 1 MiB. A seleção feita na interface permanece separada por diretório enquanto o launcher está aberto. Instalar/Reparar aplica a escolha; não há download ao apenas selecionar a pílula.

Passaram 14 testes de contratos Rust, incluindo o parser Bash real do release com LumeniteFX ligado/desligado nas três rotas (sem instalar nem baixar); um teste do leitor de INI; dois testes de renderização do seletor e três regressões de opções de inicialização. `tsc --noEmit` e `cargo check --lib --tests` passaram. O controle visual do navegador não respondeu nesta rodada, portanto não foi realizada inspeção por captura de tela. Nenhum backend/jogo real foi alterado e nenhum pacote foi gerado.
