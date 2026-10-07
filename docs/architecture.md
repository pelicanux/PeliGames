# Arquitetura modular do PeliGames e Pelinstall

Status: desenho para implementação. O código atual ainda é a base do launcher de mods. Este documento estabelece o destino da migração, não descreve funcionalidades já disponíveis.

## Objetivo e decisão inicial

Permitir que diferentes pessoas desenvolvam runners, instalação, biblioteca, aparência e integrações ao mesmo tempo, com o mínimo de alterações em arquivos compartilhados. Cada módulo terá uma responsabilidade, contratos públicos e verificação própria.

Começar com uma aplicação modular no mesmo repositório e processo. Não é necessário criar serviços de rede ou um sistema de carregamento de código externo para separar responsabilidades. Novos recursos entrarão como módulos e adaptadores registrados na composição da aplicação. Plugins distribuídos independentemente poderão ser avaliados depois, quando houver necessidade concreta de uma API pública versionada e de um modelo de permissões.

## Fronteiras

| Módulo | Possui | Não possui |
| --- | --- | --- |
| `library` | Entradas, cadastro, consulta, atualização, capas associadas e esquema da biblioteca. | Download de runners ou execução de processos. |
| `runners` | Catálogo, provedores, versões locais, download, instalação e validação dos runners. | Etapas do Pelinstall ou dados privados da biblioteca. |
| `execution` | Preparação dos prefixos, estratégias Wine/Proton, processos, argumentos, ambiente e cancelamento de execução. | Catálogo de downloads ou componentes de tela. |
| `pelinstall` | Sessão de instalação, validação das etapas, revisão, andamento e confirmação do executável final. | Implementação dos downloads ou acesso direto ao armazenamento da biblioteca. |
| `desktop` | Argumentos de entrada, atalhos e adaptadores de integração com gerenciadores de arquivos. | Regras de instalação ou seleção de runners. |
| `appearance` | Tema selecionado, tokens, variantes visuais e preferências de aparência. | Estado de jogos ou execução. |
| `sources` | Adaptadores de descoberta e metadados, normalizados antes de entrar na biblioteca. | Alteração direta dos arquivos da biblioteca. |
| `settings` | Preferências gerais e APIs de persistência de preferências por módulo. | Uma configuração global que exponha o estado interno de todos os módulos. |
| `updates` | Atualização do próprio aplicativo, quando houver distribuição do PeliGames. | Atualização de runners, que pertence a `runners`. |
| `mods` | Wizard de configuração dos mods, backends, modelos, instalação, reparação e remoção por jogo. | Instalação de jogos/programas ou catálogo de runners. |

`ui` será um conjunto de componentes reutilizáveis, e não um módulo de negócio. Um botão, modal ou campo não deverá depender de runners, jogos ou do assistente.

## Estrutura prevista

```text
src/
  app/                    # Bootstrap, navegação e composição dos módulos
  modules/
    library/
      index.ts            # API pública do módulo
      contracts.ts        # Tipos expostos e serviços requeridos
      components/
      hooks/
      services/
      tests/
    runners/              # Mesma convenção, conforme a necessidade
    execution/
    pelinstall/
    desktop/
    appearance/
    sources/
    settings/
    updates/
  platform/
    tauri/                # Transporte de comandos/eventos e adaptadores nativos
  ui/                     # Componentes, estilos base e acessibilidade
  shared/                 # Apenas tipos/utilitários pequenos e neutros

src-tauri/src/
  app/                    # Composição dos serviços e registro de comandos
  modules/
    library/
      mod.rs              # API pública
      commands.rs         # Entrada Tauri, sem regras de negócio
      service.rs          # Casos de uso
      repository.rs       # Persistência privada da biblioteca
    runners/
      providers/          # GE, CachyOS, local e futuras fontes
    execution/
      strategies/         # Inicialização específica de Wine/Proton
    pelinstall/
    desktop/
      integrations/       # Adaptadores por gerenciador de arquivos
    sources/
    settings/
    updates/
  platform/               # Processos, arquivos, HTTP e caminhos da aplicação

docs/modules/             # Contratos e guias de cada módulo, quando implementado
```

As subpastas serão criadas conforme houver código real. Um módulo pequeno pode começar com poucos arquivos; a regra é ter fronteira clara, sem produzir estruturas vazias. Separar crates Rust ou pacotes de frontend será uma decisão futura, se trouxer benefício de compilação, distribuição ou reutilização.

## Regras de dependência

1. Consumidores usam a API pública (`index.ts` ou exports de `mod.rs`). Não importam componentes, repositórios ou arquivos internos de outro módulo.
2. A composição em `app` fornece as dependências aos serviços. Módulos não acessam um registro global de serviços para buscar dependências escondidas.
3. O frontend não decide comandos de processo, arquivos de configuração ou variáveis Wine/Proton. Usa serviços tipados, implementados pelo transporte Tauri. Comandos nativos validam novamente os dados recebidos.
4. APIs Tauri ficam em `platform/tauri`; componentes de tela não espalham chamadas `invoke` e listeners pelo projeto. O código Rust que registra comandos pertence à composição, e não concentra os casos de uso.
5. Cada módulo é responsável pelo seu estado, esquema persistido e migrações. Outro módulo só modifica esses dados por operações públicas. Pelinstall registra o resultado usando a API de `library`.
6. Dependências entre módulos formam um grafo sem ciclos. Uma dependência circular exige rever a responsabilidade ou extrair um contrato pequeno, não esconder o ciclo em `shared`.
7. `shared` não depende de módulos e não vira depósito de regras de negócio. Tipos como `RunnerDescriptor` pertencem à API de runners; tipos como `LibraryEntry` pertencem à biblioteca.
8. Eventos notificam mudanças e progresso. Comandos com resposta explícita executam operações que precisam de confirmação. Evitar fluxos críticos baseados em cadeias implícitas de eventos.

Fluxo previsto: `desktop` entrega uma solicitação ao aplicativo, que abre `pelinstall`. O assistente consulta `runners`, usa `execution` para preparar o prefixo e iniciar o instalador, e usa `library` para registrar a entrada confirmada. `runners` e `execution` não dependem do assistente. O Pelinstall poderá usar esses mesmos serviços em uma janela própria.

## Contratos entre módulos

Definir os contratos antes de implementar cada recurso compartilhado. Os nomes abaixo são exemplos de responsabilidades, sujeitos à definição tipada durante a implementação.

| Serviço público | Operações esperadas |
| --- | --- |
| Biblioteca | Listar, consultar, adicionar e atualizar entradas. |
| Runners | Listar versões locais, consultar releases, instalar uma release e validar um runner. |
| Execução | Preparar um prefixo, iniciar um processo, consultar resultado e solicitar cancelamento. |
| Pelinstall | Criar sessão, revisar plano, iniciar instalação e concluir com executável confirmado. |
| Aparência | Listar temas, aplicar tema e persistir a escolha. |

Os contratos devem especificar dados de entrada/saída, erros identificáveis, capacidades opcionais e comportamento ao cancelar. Operações demoradas devolvem um identificador de operação. Eventos de progresso, logs e conclusão carregam esse identificador, para não misturar duas instalações ou downloads simultâneos.

Interfaces TypeScript e tipos serializados Rust deverão seguir o mesmo contrato, incluindo convenção de nomes e campos opcionais. Mudanças na fronteira Tauri exigem verificar a compatibilidade dos dois lados. A geração automática de tipos poderá ser adotada após avaliação das ferramentas; até lá, usar verificações de serialização dos contratos críticos.

## Pontos de expansão

**Provedores de runners:** cada provedor traduz sua fonte para o contrato comum de releases. Download e instalação utilizam serviços compartilhados do módulo runners. Particularidades de formato ou preparação pertencem ao provedor; particularidades de lançamento pertencem às estratégias de execução. O Pelinstall consulta capacidades e versões disponíveis, sem uma lista fixa de marcas no assistente.

**Temas:** componentes consomem tokens de cor, tipografia, espaçamento, bordas e movimento. Um tema fornece valores e recursos visuais declarativos, sem executar código de instalação. O escopo de estilos de cada módulo evita colisões. Trocar o tema não deverá exigir editar componentes de negócio. O visual existente será preservado como tema padrão, e novos temas serão opcionais.

## Preservação visual durante a modularização

Reaproveitar integralmente o layout, as cores, os efeitos e as animações do launcher atual. A extração para módulos preservará os valores e comportamentos existentes antes de introduzir opções de personalização. Os fluxos de mods e seu wizard serão preservados para compor a seção de mods. Nesta etapa o wizard foi restaurado com o comportamento inicial original; sua transferência para a seção de mods será feita posteriormente.

Referências atuais para a migração:

| Código atual | Comportamento a preservar |
| --- | --- |
| `src/App.css` e estilos dos componentes | Paleta, tipografia, layout, fundos, efeitos e estados visuais. |
| `src/components/GameGrid.tsx` | Grid/lista, efeitos dos cards e transição da miniatura da capa. |
| `src/App.tsx` | Transição compartilhada da capa e entrada/saída do painel selecionado. |
| `src/hooks/useAnimatedDetailsHeight.ts` | Medição do conteúdo e ajuste animado da altura do painel. |
| `src/components/AmbientBackground.tsx` | Fundo ambiente associado à capa. |
| `src/components/EffectsContext.ts` e modo desempenho | Preferência de efeitos aplicada aos componentes. |
| Demais componentes e diálogos | Animações de abertura/fechamento, feedback e interação existentes. |

Tokens visuais e presets genéricos de movimento poderão ficar em `ui`; coordenação da seleção de capas e estado do painel continuará no módulo da biblioteca. Preservar o vínculo entre a capa da biblioteca e a capa do painel, hoje realizado por `layoutId`, ao reorganizar componentes. Manter também a medição do conteúdo separado do contêiner animado, para não introduzir oscilações de altura.

Antes de migrar esses componentes, registrar capturas e vídeos curtos do comportamento atual como referência. Depois, comparar seleção, troca e fechamento de jogos, efeitos dos cards, grid/lista, fundos, menus e diálogos, tanto no modo com efeitos quanto no modo desempenho. Capturas estáticas verificam o layout; gravações e interação verificam as animações. Pelinstall e novas telas deverão usar essa mesma linguagem visual.

**Integrações desktop:** cada gerenciador de arquivos terá um adaptador que produz a mesma solicitação de instalação. Diferenças do menu de contexto ficam no adaptador.

**Fontes de jogos e metadados:** descoberta e arte usam interfaces separadas. Um novo adaptador entrega dados normalizados; importação e resolução de duplicatas continuam sob responsabilidade da biblioteca.

**Novos módulos:** declarar responsabilidade, API pública, dependências e ponto de entrada. Registrar na composição da aplicação. Criar documentação e verificações proporcionais ao recurso. Um novo recurso não deverá depender de alterar estados internos de módulos existentes.

## Trabalho simultâneo

- Cada módulo terá um guia curto com finalidade, API pública, dependências, instruções de verificação e extensões previstas. Exemplos e serviços falsos permitirão trabalhar na interface enquanto o backend é desenvolvido.
- A responsabilidade por módulos pode ser distribuída entre colaboradores, sem bloquear contribuições de outras pessoas. Mudanças que alteram contratos exigem coordenação com os consumidores afetados.
- Preferir alterações concentradas em um módulo. Separar mudanças de contratos das grandes alterações de interface ou reorganizações de arquivos.
- Navegação, bootstrap e registros centrais devem ser pequenos e declarativos, para reduzir conflitos. Textos de interface e estilos próprios serão mantidos próximos do módulo, com catálogo de idiomas composto pela aplicação.
- Documentar decisões que mudam fronteiras ou persistência. Não exigir um novo documento para escolhas locais simples.
- A futura CI deverá verificar tipos e compilação, dependências proibidas e testes relevantes. Verificações de arquitetura deverão impedir imports internos e ciclos; a regra não ficará apenas neste documento.

## Verificação e migração

Testar regras de negócio com adaptadores controlados, contratos com casos de serialização e integrações reais nos pontos de processo, persistência e download. Para Wine/Proton e menu de contexto, manter um roteiro de validação em Linux e registrar o ambiente testado.

Migrar por etapas: composição e fronteiras iniciais; biblioteca; runners e execução; Pelinstall; aparência e integrações. A aparência pode ser desenvolvida em paralelo quando os tokens e componentes básicos estiverem definidos. Reaproveitar funções úteis do legado por adaptadores e removê-los conforme cada módulo assuma sua responsabilidade. Não mover todo o código de uma vez apenas para renomear pastas.

Considerar a modularização de um módulo concluída quando sua API está documentada, seu estado é privado, suas dependências são explícitas, não há ciclos e suas regras podem ser verificadas sem iniciar toda a interface. O projeto permanece em migração até isso se aplicar aos módulos que compõem o produto.
