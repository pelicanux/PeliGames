# Pesquisa de launchers Windows para o PeliGames

Data da revisão: **13/10/2026**. Escopo: Ubisoft Connect, EA app e Battle.net. GOG GALAXY não faz parte deste catálogo.

Este levantamento documenta sintomas recorrentes e configurações utilizadas em projetos de compatibilidade. Relatos não equivalem a uma causa confirmada, e versões novas dos clientes podem invalidar ajustes antigos. As receitas são experimentais; ainda não houve instalação ou teste de login no PeliGames.

## Runners e requisitos comuns

**GE-Proton com UMU é o candidato inicial**, pois o próprio projeto documenta esse caminho para uso fora da Steam e sua integração com Lutris e Heroic. Isso é evidência de adoção, não um ranking estatístico de popularidade. A pesquisa não encontrou dados comparáveis que permitam afirmar qual runner tem mais usuários em cada launcher. Executar diretamente o Wine interno do GE ignora o ambiente suportado pelo projeto. [GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom/blob/master/README.md), [UMU](https://github.com/Open-Wine-Components/umu-launcher/blob/main/README.md).

Verificar drivers gráficos e Vulkan antes de instalar. O suporte de 32 bits deve ser avaliado conforme os aplicativos e o runner escolhidos; o runtime não elimina a necessidade de um driver de GPU funcional no host. As instruções de pacotes variam por distribuição e arquitetura, então não incluí um comando universal de instalação. [Guia de drivers do Lutris](https://github.com/lutris/docs/blob/master/InstallingDrivers.md).

O catálogo propõe prefixo exclusivo de 64 bits, perfil Windows 10 e preferência por ext4/Btrfs como política inicial do PeliGames. Essas escolhas precisam ser validadas por launcher. Dependências de jogos individuais, inclusive anticheat, ficam separadas da instalação do cliente.

## Resultado por launcher

| Launcher | Problemas documentados | Componentes e ajustes candidatos | Runner a avaliar |
| --- | --- | --- | --- |
| Ubisoft Connect | Instalador falha e o executável não é criado; regressão relatada no GE-Proton10-34. | `corefonts`, `d3dcompiler_43`; opção de desativar overlay. | GE + UMU para execução; Wine Staging como alternativa documentada para instalação. |
| EA app | Janela preta, instalação incompleta e executável em diretório versionado. | `corefonts`, `physx` como candidatos da receita consultada; descoberta limitada de caminhos alternativos. | Comparar GE + UMU com DW-Proton + UMU. |
| Battle.net | Update Agent travado; login sem controles; problemas de rede/DNS. | Verificação de Vulkan; opção de desativar aceleração de hardware e streaming do cliente. | GE-Proton + UMU. |

### Ubisoft Connect

Há relato confirmado por colaborador do Lutris de instalador que falha com GE, além de relatos de instalação com Wine Staging seguida de execução com GE. Isso justifica oferecer uma alternativa por etapa, **não mudar automaticamente o runner do usuário**. O teste deve registrar a versão concreta; o exemplo GE-Proton10-34 é histórico e não significa que todo GE atual esteja afetado. [GE-Proton #527](https://github.com/GloriousEggroll/proton-ge-custom/issues/527), [Lutris #6658](https://github.com/lutris/lutris/issues/6658).

O instalador oficial, `corefonts`, `d3dcompiler_43` e desativação do overlay aparecem na receita comunitária. O ajuste de overlay deve preservar os demais valores da configuração. [Receita Ubisoft do Lutris](https://lutris.net/games/install/5228/view).

### EA app

A receita consultada usa **DW-Proton**, adiciona fontes e PhysX e trata uma mudança de caminho causada por diretórios versionados. Nosso JSON permite descobrir o executável nesses caminhos sem reproduzir o procedimento de apagar/mover pastas. A necessidade de PhysX deve ser avaliada; não será classificada como requisito comprovado de todo jogo. [Receita EA do Lutris](https://lutris.net/games/install/36644/view), [projeto DW-Proton](https://github.com/human-sketch/DWproton).

Há relatos de executável ausente após instalação e janela preta. Eles são úteis para diagnóstico, mas não comprovam uma solução universal. [Instalação incompleta](https://forums.lutris.net/t/ea-app-not-installing/22481), [janela preta](https://forums.lutris.net/t/black-screen-issue-when-launching-ea-app/23831).

### Battle.net

O guia relaciona travamentos do agente à configuração gráfica e cita problemas de DNS, filtragem de rede e renderização do login. O catálogo orienta verificar essas condições e oferece desativação da aceleração do cliente; não altera `/etc/hosts`, firewall ou cache por conta própria. As recomendações antigas de versões Wine não se tornam padrões novos. [Guia Battle.net do Lutris](https://github.com/lutris/docs/blob/master/Battle.Net.md).

A receita atual fornece o instalador oficial e opções de aceleração/streaming. Nenhuma lista extra de DLLs foi inventada para o Battle.net. O aplicativo deve ser fechado ao concluir o instalador; processos auxiliares precisam ser tratados por prefixo. [Receita Battle.net do Lutris](https://lutris.net/games/install/37360/view).

## O que os JSONs já permitem preparar

Catálogo com três entradas, versões próprias, links oficiais, dependências com fontes, alternativas desligadas, sintomas e orientação de diagnóstico. O índice inclui hashes de integridade e o esquema impede receitas com comandos livres. A pesquisa preserva a distinção entre cliente Windows e integrações de conta do Lutris: falhas de sincronização da biblioteca de terceiros não viraram supostas dependências do instalador.

O próximo trabalho de produto é conectar esses dados ao assistente existente e implementar o executor restrito. Antes de habilitar instalação automática, ainda é necessário auditar redirecionamentos e publicadores das assinaturas dos três instaladores, validar runners disponíveis e realizar os testes registrados em `installers/README.md`. A validação offline dos JSONs não substitui esses testes.
