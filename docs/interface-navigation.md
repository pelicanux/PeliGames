# Retorno de atalhos nas configurações

Ao abrir uma configuração por um atalho dentro dos detalhes de um jogo (por exemplo, **Opções → Versão do Proton → Outra… → Gerenciar versões**), a configuração é um destino temporário. Clicar em **Biblioteca** no menu lateral ou fechar as configurações deve retornar ao mesmo jogo, na mesma biblioteca e na mesma aba que estava selecionada.

Essa regra também se aplica a outros atalhos para configurações quando houver uma tela de origem a preservar, como conexões de contas. O destino temporário não deve redefinir a aba para Visão geral, DLSS ou Mods. Downloads, salvamentos e diálogos que bloqueiem a navegação continuam respeitados.

A seleção de aba pertence ao estado da tela principal em `src/App.tsx`, e é passada a `ModernGameDetails`. Assim ela sobrevive à desmontagem temporária do painel durante as configurações. Fechar os detalhes ou mudar de jogo/biblioteca inicia um novo contexto de abas; essa lembrança é da sessão atual e não é salva no disco.

## Verificação

1. Abrir um jogo da Steam na biblioteca e selecionar **Opções**.
2. Abrir **Outra… → Gerenciar versões** no seletor de Proton.
3. Confirmar a página do Gerenciador de Proton, com downloads e integração Steam.
4. Clicar no ícone **Biblioteca**: o mesmo jogo deve voltar na aba **Opções**.
5. Repetir a partir de outro contexto aplicável, como os detalhes da biblioteca Nexus.
6. Fechar os detalhes e abrir outro jogo: o jogo novo deve iniciar na sua aba padrão.

## Comparação de benchmarks do jogo

Na aba **Desempenho**, **Comparar testes deste jogo** fica no cartão de monitoramento, ao lado de **Configurar visual**. A comparação substitui o conteúdo do painel de detalhes existente, sem abrir outro diálogo. **Voltar ao desempenho** restaura o mesmo jogo, a aba Desempenho e a rolagem anterior, com o foco no botão de comparação. O X continua fechando os detalhes. Fechar o painel ou mudar de jogo/biblioteca descarta essa navegação temporária.

A comparação continua selecionando automaticamente o penúltimo registro como A e o mais recente como B, com seleção manual e atualização da dupla disponíveis.

**Salvar comparação** exporta um PNG completo com identificação dos testes, métricas e gráfico, independente do tamanho ou da rolagem do painel. Requer dois registros analisados. No aplicativo, o destino é escolhido pelo diálogo nativo de salvamento; na prévia web, a imagem é baixada pelo navegador. Cancelar o diálogo não gera mensagem de sucesso.

As listas de testes A e B também oferecem **Selecionar registro manualmente…**, mesmo sem registros detectados. Essa opção exibe um botão de pasta com lupa para escolher o CSV completo em qualquer pasta (por exemplo, uma gravação da Steam). Cada arquivo é validado e carregado automaticamente, sem ser movido ou copiado para a biblioteca; os dados escolhidos ficam na sessão da comparação e participam da exportação PNG. Atualizar a lista mantém as escolhas manuais; **Comparar os dois últimos testes** volta à seleção automática. O arquivo `_summary.csv` não é aceito porque não contém as amostras necessárias ao gráfico.

Os nomes exibidos dos testes podem ser editados, com **Teste A** e **Teste B** como padrão. São rótulos da sessão da comparação, mantidos ao trocar os registros dentro dela e incluídos no PNG; não renomeiam os CSVs. A apresentação usa A à esquerda, métricas no centro e B à direita, com barras na mesma escala em cada linha, resumo de fluidez e grupos de hardware e captura. O gráfico de frametime possui eixos, legenda com os nomes e leitura ao passar o mouse. As curvas mostram médias por trecho e progresso normalizado, sem sincronização automática de cenas. Mínimos por amostragem não são apresentados como percentis calculados sobre cada quadro. Utilização de CPU/GPU é comparada em pontos percentuais e não recebe classificação de melhor/pior.

Referências de apresentação e interpretação: [recursos de comparação do CapFrameX](https://www.capframex.com/features) e [explicação de métricas de desempenho](https://www.capframex.com/blog/post/Explanation%20of%20different%20performance%20metrics). A inspiração visual não implica equivalência entre a captura amostrada do MangoHUD e a captura por quadro de outras ferramentas.


### Arquivos arrastados para o launcher

- Um arquivo ou pasta por vez abre a escolha Instalar / Adicionar / Instalar mod; soltar não executa arquivos.
- Instalar e Adicionar pedem o nome digitado pelo usuário antes da busca de executáveis. Não deduzir o nome automaticamente.
- Pastas são consultadas somente após Avançar, com limite de profundidade/itens, sem seguir links simbólicos internos. Mesmo havendo um único executável, a lista é exibida sem seleção automática e o usuário confirma qual usar.
- O assistente reutiliza o componente Pelinstall em modo incorporado: não redimensiona, arrasta, minimiza ou destrói a janela principal. Os comandos de instalar/adicionar, extração de ícone, prefixo, Proton, atalhos e confirmação continuam compartilhados.
- Mods locais pedem o jogo de destino já associado à biblioteca Nexus; o pacote é importado para revisão antes da instalação.
- Novas importações aguardam diálogos e operações em andamento. A prévia usa `&drop=folder` para simular uma pasta; arrastar caminhos reais requer o aplicativo nativo.

- Ao adicionar uma pasta, os filtros e a classificação de executáveis são compartilhados com a biblioteca instalada (nome informado, estrutura Unity/Unreal, tamanho e penalização de ferramentas). O nome do registro continua sendo o digitado pelo usuário.
- A descoberta de Adicionar usa a mesma busca somente de leitura da instalação, limitada à pasta fornecida. Não prepara nem faz inventário anterior/posterior do prefixo, inclusive quando um prefixo existente será reutilizado. Pastas de pré-requisitos, redistribuíveis e ferramentas conhecidas são excluídas; um Shipping continua disponível quando for o executável real do jogo.
- Um candidato claramente superior é marcado Recomendado, sem ser selecionado automaticamente; em caso de empate, não há recomendação forçada. A lista e a confirmação são obrigatórias mesmo com um único candidato, tanto ao adicionar quanto na descoberta após a instalação. A busca incompleta não gera recomendação. Instalar preserva os setups; um executável arrastado diretamente já foi escolhido pelo usuário e segue selecionado para o assistente, sem exigir a lista. A distinção entre arquivo e pasta é informada pelo backend, sem deduzi-la pela extensão ou pelo número de candidatos.

- Instalar por arraste chama o mesmo `run_game_installer` do Pelinstall: prepara o prefixo, registra o inventário de executáveis anterior (`prefix_baseline`), executa o instalador e entrega esse inventário à descoberta pós-instalação. Isso evita confundir programas já presentes no prefixo com o programa recém-instalado; não se trata de uma cópia integral do prefixo. Manter este caminho compartilhado ao evoluir o assistente incorporado. O teste `scan_offers_only_what_the_installer_created` cobre o comparativo e a reinstalação no mesmo prefixo.

### Epic nativa: integração experimental de execução

- Jogos Windows da Epic usam um adaptador no ponto de entrada do runtime Proton, com original preservado por hash. Não usar `additionalCommands` como wrapper: o cliente passa esse conteúdo aos argumentos do executável Windows.
- Proton, variáveis, argumentos e wrappers são salvos por jogo. O adaptador lê o mapa em cada execução, inclusive quando a Epic já está aberta; não aplicar esses ajustes ao processo do cliente nem exigir que ele seja reiniciado. Nome, executável e prefixo continuam controlados pela Epic.
- A identificação combina o compatdata derivado do `artifactId` com o executável detectado. Jogos sem configuração usam o runtime original. Escolher o runtime da própria Epic não deve gerar recursão.
- Backups do Proton ficam junto ao runtime e o mapa privado em `.peligames-runtime` na pasta de dados da Epic. Atualizações do runtime são reconhecidas na próxima aplicação pelo PeliGames. Se o mapa/adaptador auxiliar desaparecer, o original permanece utilizável. Logs não devem conter argumentos de autenticação nem valores das variáveis.
- Teste real sem recompilar: Epic 0.97.0 permaneceu com os mesmos processos; FEZ iniciou com GE-Proton11-7 e uma variável de teste exclusiva. O arquivo original GE-Proton10-34 foi restaurado após a prova. Esse teste valida o mecanismo, não garante compatibilidade de todo jogo/Proton.

### Pastas padrão e remoção da biblioteca

- Ao escolher a pasta padrão no PeliGames ou Pelinstall, o backend reserva uma pasta nova com criação exclusiva: `Among Us`, `Among Us (2)`, etc. Pastas e arquivos existentes nunca são reutilizados nessa escolha. A interface mantém o caminho efetivamente reservado até concluir ou cancelar o fluxo.
- Adicionar novamente o mesmo executável no mesmo prefixo retorna o registro existente. Prefixos diferentes continuam sendo instalações independentes.
- “Remover da biblioteca” para jogos PeliGames persiste a remoção no manifesto, sem excluir o prefixo, os jogos, os mods, os saves ou os perfis. Executáveis descobertos recebem uma exclusão persistente para a varredura; registros adicionados são removidos pelo identificador individual. Outros cartões que compartilham o prefixo permanecem.

### Redefinição e armazenamento persistente de mods Nexus

- O gerenciamento Nexus usa `<pasta-do-jogo>/.peligames/mods/<id>/` para pacotes, perfis, `deployment.json`, originais substituídos e `game.json`. O índice privado em `$XDG_DATA_HOME/.peligames-mod-library/catalog.json` fica fora dos alvos de redefinição. Não contém credenciais de contas.
- Instalações antigas são migradas antes de preparar a redefinição. Arquivos usam hardlinks no mesmo filesystem quando possível; entre discos, são copiados e verificados com SHA-256. Os dados centrais antigos só desaparecem junto da limpeza posterior ao reinício validado. A migração pode exigir espaço temporário entre discos e falha sem autorizar a limpeza se a cópia não for concluída.
- Por padrão, redefinir preserva os mods e recupera seu gerenciamento pelo índice persistente. Adicionar/escanear a pasta também reconhece o registro próprio do jogo sem procurar recursivamente pelos discos.
- A confirmação oferece uma única opção, desmarcada, para remover todos os mods Nexus dos jogos listados e restaurar os originais preservados. Não remove resíduos antigos sem registros. As transações dos jogos permanecem reversíveis até salvar a alteração de toda a biblioteca.
- O plano mostra jogos, caminhos e dispositivos registrados. Discos indisponíveis bloqueiam a remoção em lote; também bloqueiam a preservação quando os dados antigos ainda precisam ser migrados. Registros já portáteis continuam mapeados mesmo com o disco desconectado. Alterações na biblioteca exigem revisar a confirmação novamente.
- Pacotes e originais pertencem à instalação do jogo; não são cache descartável do launcher. Dados de mods DLSSNR não são removidos pela opção explicitamente rotulada como Nexus.

### Preservação opcional de contas na redefinição

- A confirmação oferece “Preservar contas conectadas”, desmarcada por padrão e independente da remoção dos mods. Marcá-la mantém Matrix, Discord e Nexus Mods sem revogar sessões ou apagar os itens do chaveiro (incluindo Nexus OAuth e a chave de criptografia Matrix RTC).
- O helper, após encerrar o processo, copia apenas os armazenamentos de sessão permitidos: navegador Nexus e bancos WebKit/IndexedDB/cookies. Preferências JSON e localStorage do launcher não são copiadas. Não exporta segredos do chaveiro para JSON. Diretórios e arquivos copiados recebem permissões privadas; links e arquivos especiais interrompem a operação.
- Os originais continuam nas pastas renomeadas até a validação da nova inicialização. Uma interrupção com preservação exige restaurar o perfil anterior antes de tentar novamente, evitando confirmar uma sessão copiada parcialmente. Com a opção desmarcada, mantém-se a desconexão e remoção das credenciais locais.

- Na confirmação de redefinição, a lista de jogos tem altura limitada a 220px (ou 28% da janela em telas menores), rolagem interna e foco por teclado. A lista não deve expandir o diálogo conforme o número de jogos.

### Preservar jogos do Pelinstall ao redefinir

- Caixa independente “Preservar jogos instalados ou adicionados pelo Pelinstall”, desmarcada por padrão. Preserva `installed-destinations.json` no novo perfil; manifestos, prefixos e arquivos permanecem nas pastas externas originais. Não mantém as demais preferências nem reinstala runners apagados pela redefinição.
- A confirmação lista os destinos e avisa sobre instalações indisponíveis. O índice mantém discos desconectados; a biblioteca verifica os manifestos/executáveis quando as pastas voltam a estar disponíveis. Não há varredura recursiva dos discos.
- Antes de preparar e antes de reiniciar, verifica se destinos, executáveis ou prefixos conhecidos estão dentro dos alvos da redefinição; nesse caso bloqueia e orienta mover a instalação. A cópia do índice ocorre após encerrar o processo, mantendo os originais no backup até a validação. Não modifica nem exclui jogos externos com a opção desmarcada; apenas deixa de preservar seu índice.

### Biblioteca unificada e abas por instalação

- A navegação lateral contém uma biblioteca única. Nexus e DLSSNR ficam nas abas dos detalhes: Visão geral, Mods, DLSSNR, Geração de quadros, Desempenho, Opções e Avançado. Ferramentas específicas aparecem quando o módulo as oferece. Atalhos antigos e downloads continuam abrindo o jogo, sem criar outra biblioteca.
- A descoberta principal usa os manifestos dos launchers e registros do Pelinstall. Pastas manuais continuam sendo percorridas como coleções de subpastas, sem aplicar a heurística do DLSSNR ao diretório principal. Registros Nexus antigos são incorporados à apresentação, sem duplicar instalações já encontradas.
- `sameGameInstallation` associa por identidade externa ou registro Pelinstall antes de comparar caminhos; não associa por título. Prefixos diferentes não podem compartilhar a configuração. Uma raiz e sua pasta de executável podem representar a mesma instalação, respeitando limites de caminho.
- Identidades Nexus legadas vindas do scanner podem conter apenas o ID da loja, enquanto a biblioteca principal usa `launcher:native/flatpak:loja:id` (Steam não inclui loja). A comparação normaliza esses formatos sem misturar lojas, launchers ou prefixos diferentes. Preparar uma instalação já cadastrada reutiliza seu registro, UUID, perfil e mods; não gera uma segunda entrada nem um erro de duplicidade. A preparação da aba é vinculada ao jogo e ao estado do cadastro, para que uma tentativa antiga com erro não contamine outro jogo ou um registro já recuperado.
- Mods faz inspeção específica apenas ao abrir a aba. O módulo deve confirmar suporte; sem suporte, explica a indisponibilidade mantendo a aba visível. Mods locais não exigem login. Catálogo/coleções mantêm a verificação de conta e a notificação com atalho para Conexões.
- DLSSNR resolve o destino somente quando solicitado, em uma tarefa de backend, e reutiliza o resultado por instalação/caminho/revisão. `GameModules/<hash>.json` guarda destinos manuais sem alterar o diretório raiz, o identificador da instalação ou a biblioteca principal. A seleção manual fica dentro da instalação identificada. Jogos Linux nativos não recebem o instalador de DLLs Windows.
- A Visão geral identifica a origem da raiz e distingue prefixo, pasta do executável e destino DLSSNR. Caminhos possuem Abrir/Copiar. Destino ainda não pesquisado não é apresentado como detectado.

### Geração de quadros: integrações estáveis verificadas em 18/10/2026

- MAKO Renderer **4.5.0**, pacote oficial `https://github.com/eugeniosegala/MAKO/releases/download/render-v4.5.0/MAKO-Renderer-v4.5.0-linux.tar.xz`, SHA-256 `4dd3e70743a08a0f2f0e6006d35d1e08dfcf751932678a6217cbdcf87c784305`.
- O MAKO oferece multiplicador fixo e geração adaptativa. Não há promessa de um vencedor universal de desempenho. Não usar builds git, release candidates ou versões experimentais automaticamente.
- Download sob demanda, com limite de tamanho, checksum de referência nas versões iniciais e quando publicado no catálogo oficial, extração sem caminhos externos/links/arquivos especiais e validação do payload antes da ativação. Runtimes ficam em `PeliGames/FrameGeneration/`, fora das pastas dos jogos e sem registro Vulkan global.
- Perfil TOML e wrapper exclusivos por instalação e conteúdo da configuração, aplicados pelo pipeline de wrappers já existente (Steam, Epic, Pelinstall e execução Nexus). Configurações são imutáveis para que as alterações pendentes e Reverter da Steam preservem o comportamento anterior. Nenhuma DLL proprietária é fornecida, extraída ou copiada pelo instalador desses runtimes.
- MAKO usa `Lossless.dll` da ramificação pública padrão. Busca nas bibliotecas Steam, com seleção manual disponível. A troca de backend exige reiniciar o jogo; a interface salva para a próxima execução.
- Somente um backend é escolhido. O wrapper desabilita a camada concorrente para aquele processo. O MAKO usa SDR nesta integração; HDR permanece indisponível porque o upstream o marca como experimental. Bloqueios locais incluem MLFG, baixa latência DX12/camada Vulkan e captura Vulkan OBS, inclusive variáveis equivalentes em opções manuais. Alterações avançadas posteriores também respeitam esses bloqueios.
- OptiScaler/ReShade não são automaticamente proibidos: a documentação do MAKO considera a compatibilidade dependente do jogo e exige desligar seu Frame Generation. Upscaling ou DLSSNR isoladamente não comprovam conflito. A interface exige confirmação de que os outros geradores foram desligados; não presume conseguir ler o estado interno de todo jogo/mod.
- Requer Vulkan (direto ou via DXVK/VKD3D). OpenGL/WineD3D não são cobertos por essa ativação. Flatpak exige preparação explícita da extensão correspondente: Preparar / verificar Flatpak identifica a ABI e verifica o sandbox antes de liberar a ativação. O script do host não é usado como substituto da extensão. MangoHUD pode mostrar os quadros reais, e a contagem de saída não representa redução proporcional de latência.
- MangoHUD com MAKO no host: o inicializador do fornecedor isola as camadas implícitas, portanto executar `mangohud` sozinho não basta. O PeliGames valida e copia somente os manifestos conhecidos do MangoHUD (32/64 bits) para `FrameGeneration/layers/mangohud`, resolve bibliotecas relativas e preserva as barreiras de arquitetura. Após `mako-launch`, um estágio de execução organiza MAKO → MangoHUD → overlay Steam, sem reabrir a descoberta global de camadas implícitas nem alterar o runtime baixado. A ativação acompanha a exibição ou o limitador do MangoHUD naquele jogo; desativá-los preserva a cadeia do fornecedor. Shaders vkBasalt do MAKO e MangoHUD não podem ser combinados. A extensão Flatpak mantém seu caminho separado; esta integração de camadas do host não se aplica a ela.
- Wrappers nativos já existentes são atualizados no mesmo caminho ao salvar as opções do jogo/monitoramento. A linha fixa do `pelirun` na Steam é preservada. A ativação da camada exige reabrir o jogo; não exige reiniciar a Steam.
- Na versão 4.5.0, o transporte SDR ordenado do MAKO força `VK_PRESENT_MODE_FIFO_KHR`. A saída pode ficar limitada à taxa de atualização do monitor mesmo com o limitador do launcher zerado; `pacing=none` não desativa essa política. O contador Steam sozinho não comprova a geração de quadros. Para separar quadros reais/gerados, os diagnósticos oficiais `MAKO_PRESENT_DIAGNOSTICS=1` registram `base_fps`, `observed_output_fps` e `generated_presented`; não inferir funcionamento apenas pelo teto de FPS.
- Referências primárias: [MAKO releases](https://github.com/eugeniosegala/MAKO/releases), [combinação de camadas MAKO](https://github.com/eugeniosegala/MAKO/blob/main/engine/docs/LAYER-CHAINING.md).

- O workspace Nexus é carregado uma vez como metadados para a biblioteca unificada. A atualização periódica fica ativa somente nas abas Mods/Ferramentas ou enquanto há execução Nexus em andamento; a unificação não deve criar uma varredura geral contínua em segundo plano.

### Histórico e atualização de geração de quadros

- Catálogos oficiais: releases GitHub MAKO com tag `render-v`, sem draft/prerelease e com o asset Linux esperado. Respostas limitadas a 2 MiB, timeout de 20 s e cache local de seis horas; botão para forçar consulta. URLs de pacotes são restritas ao endereço oficial derivado da versão semântica.
- Baselines compatíveis: MAKO 4.5.0. Apenas versões estáveis da mesma versão maior e iguais/superiores à baseline podem ser instaladas. Mudanças maiores são sinalizadas para adaptação do launcher; a verificação não é uma garantia de compatibilidade em jogos.
- Organização nova: `FrameGeneration/mako/versions/<versão>` e `current.json` com o padrão para novos perfis. Diretórios legados `mako-4.5.0` continuam válidos e não são movidos, preservando wrappers existentes.
- Cada perfil persiste `runtime_version`; o caminho do wrapper aponta para esse runtime imutável. Atualizar altera o padrão de novos perfis, sem reescrever os perfis de outros jogos. Para usar uma versão nova/anterior, selecione em **Versões e backups** e salve para este jogo.
- Downloads não substituem runtimes válidos. Extração em staging, validação e publicação atômica precedem a mudança do padrão. O SHA-256 é comparado com o checksum de referência se disponível; sem referência, o pacote continua vindo do HTTPS oficial, tem payload validado e seu hash calculado é registrado, sem alegar autenticação por checksum independente.
- Histórico mostra atual/backups e os vínculos com perfis. Exclusão exige confirmação na própria interface, nunca remove a atual ou um runtime referenciado. A verificação é conservadora e inclui snapshots de perfis, pois alterações pendentes/Reverter da Steam podem ainda apontar para eles. Instalar, salvar perfil e excluir compartilham o bloqueio de manutenção.

### Busca global de preferências

- A busca reúne controles de todas as categorias, sem depender da página aberta. Resultados são agrupados por módulo/categoria; o botão Abrir continua disponível para navegar à página de origem.
- O catálogo completo do MangoHUD participa da busca. Resultados como FPS expõem os próprios controles (exibição, métricas Low, cores, limitador, lista de limites e atalhos), usando a mesma validação e salvamento automático do editor. Alterar o limite único limpa a lista de limites concorrente, conforme o modo simples.
- Busca ignora acentos e exige todas as palavras, com aliases de hardware, FPS, atalhos e recuperação. A seção dá contexto, mas não injeta termos específicos de outros controles (por exemplo, buscar limite FPS não inclui a paleta de cores).
- Demais categorias reaproveitam seus componentes e handlers; somente os `data-preference-id` correspondentes e seu contexto de títulos ficam visíveis. Conteúdo assíncrono é filtrado em inserções de DOM. Não há varredura periódica de toda a interface nem nova implementação dos fluxos de salvar/restaurar.
- Ao buscar no MangoHUD, prévia animada e seu timer gráfico ficam suspensos. Fora da página de monitoramento, o editor de busca só monta quando houver resultados no catálogo.

### Módulos nas preferências

- Configuração de mods organiza DLSSNR-AMD, Nexus Mods, MAKO em pílulas com indicador deslizante, navegação por setas e respeito à redução de movimento.
- MAKO reutilizam o instalador dos detalhes do jogo para downloads, progresso, destino, atualizações e versões/backups. A instalação é compartilhada; escolher DLL/versão e ativar continua sendo uma configuração de cada jogo. O contexto global não salva perfis de jogos.
- A busca global inclui o gerenciamento dos dois runtimes e abre a aba correspondente.

- Os painéis DLSSNR-AMD e DLSSNR-RDNA3 expõem diretamente baixar/atualizar, selecionar pasta local, abrir a pasta e apagar o backend. Reutilizam o atualizador existente em modo compacto, com progresso restrito ao download ativo, bloqueio de operações simultâneas e confirmação antes de excluir; não exigem entrar em Gerenciar backend.

### Organização dos controles de mods

- Preferências mantêm as pílulas e usam superfícies independentes diretamente no fundo da página, com ações compactas e sem um painel externo envolvendo os módulos. DLSSNR mostra um painel por arquitetura; cada ação explica brevemente o que baixa, importa, abre ou remove.
- MAKO nas preferências seguem preparação do componente → ativação nos jogos. Instalação, destino, progresso e versões continuam usando os comandos existentes. Instalar não ativa nenhum jogo automaticamente.
- Geração de quadros nos detalhes segue a organização do DLSSNR: gerador/DLL/multiplicador e salvar na coluna esquerda; ajustes e confirmação de compatibilidade à direita. Em janelas menores as colunas se empilham. Sem mudanças nos bloqueios de compatibilidade, persistência ou lançamento.

### Conexões

- Nexus Mods, Discord e Matrix usam pílulas com navegação por teclado. Conta e ações ficam em um painel; acesso, benefícios ou preferências ficam em outro painel irmão, diretamente no fundo da página.
- Os componentes permanecem montados ao trocar de aba para preservar formulários e autorizações em andamento. Isso mantém somente os fluxos ativos; o JSX de layout substituído e os estilos exclusivos antigos foram removidos.
- A busca global reúne os controles dos serviços correspondentes e o atalho Abrir seleciona a pílula de destino. A conexão Nexus mantém OAuth e chave pessoal com entrada protegida, sem persistir a chave no armazenamento do navegador.
- Validação visual usa contas simuladas da prévia; não autentica, revoga sessões ou altera o chaveiro real.

### Progresso de componentes e Proton

- Proton-GE e CachyOS usam painéis irmãos com versão instalada/disponível, instalar/atualizar, busca de atualização, cancelamento e remoção confirmada. Não há painel externo envolvendo os dois. A integração com as versões da Steam permanece separada e mantém sua confirmação.
- Proton e geração de quadros compartilham a apresentação de progresso: fase, porcentagem quando o total é conhecido, bytes e velocidade durante a transferência, com barra verde. Total desconhecido fica indeterminado, sem porcentagem inventada. Verificação e extração têm rótulos próprios e não exibem velocidade de download.
- Proton usa os bytes por segundo fornecidos pelo backend. MAKO medem os incrementos de bytes recebidos no canal, desconsiderando conexão e extração. Os números da prévia são simulados; não executam instalações reais.
- Estilos exclusivos dos antigos botões/progresso Proton foram removidos. O material de progresso ainda usado pelo instalador MangoHUD foi preservado.

### Telas de monitoramento MangoHUD

- Monitoramento separa as pílulas Instalação do MangoHUD, Monitoramento do MangoHUD e Comparação de benchmarks. O atalho de instalação solicitado pelo aviso de MangoHUD ausente abre a primeira, em vez de expandir uma seção dentro do editor.
- O editor tem telas exclusivas Simples e Avançado: os presets, ajustes visuais e atalhos ficam no Simples; o catálogo completo com busca própria fica no Avançado. Prévia e perfil são compartilhados, com salvamento automático e navegação por teclado.
- Trocar para Instalação preserva o estado do editor e pausa sua animação de prévia. A instalação mantém as operações existentes, autorização administrativa e barra de progresso compartilhada. Os antigos detalhes expansíveis e seus estilos exclusivos foram removidos.

- Comparação de benchmarks ocupa a própria aba, sem moldura externa e sem a prévia do HUD. Mantém os modos Simples/Detalhado, escolha automática ou manual de registros, nomes dos testes e exportação da imagem. Trocar entre monitoramento e comparação preserva os ajustes e os testes selecionados; o atalho de gravação acompanha a configuração compartilhada.

- Nos detalhes do jogo, geração de quadros mostra apenas Instalar quando o componente selecionado estiver ausente. O botão abre Configurações → Mods na pílula MAKO e informa a ausência em uma notificação clicável para o mesmo destino. Download, busca de atualização e exclusão de backups ficam nas configurações; a escolha da versão por jogo permanece nos detalhes. Ao concluir a instalação, o perfil aberto reconhece o componente sem substituir os ajustes em edição.

### Overlay PeliGames de geração de quadros

- A chave **Overlay PeliGames** é por jogo e desativada por padrão, inclusive ao ler perfis antigos. Salvar a chave atualiza o wrapper já vinculado ao PeliRun. A camada só será carregada na próxima execução do jogo; não se injeta em processos já abertos nem se exige outra linha da Steam.
- A camada Vulkan em Rust desenha apenas um painel pequeno, translúcido e arrastável sobre os quadros do jogo. Começa oculto; F8 (ou a tecla gravada) mostra/oculta o mesmo painel. Não há WebKit/Tauri no jogo, splash do launcher, janela de desktop, barra de título, botões de fechar/fixar ou fechamento por Esc. Esc continua disponível ao jogo.
- O atalho é tratado pela entrada do próprio processo gráfico, somente quando ele está em foco. Não registra uma combinação no KDE/GNOME nem pede autorização do portal. A inicialização migra somente a ação KDE antiga `com.pelicano.peligames / frame-overlay`; não altera outros atalhos. Combinações reservadas pelo próprio sistema podem continuar sendo interceptadas pelo compositor.
- O gravador preserva o desenho compacto: tecla e Padrão juntos, ajuda embaixo alinhada à direita. A preferência é compartilhada entre os jogos e vale ao reabri-los. MangoHUD e o overlay verificam conflitos internos nos dois sentidos, usando o mesmo mapeamento físico de teclas do renderer/runner.
- A camada só aceita o perfil MAKO habilitado com `overlay_enabled=true`. Não precisa descobrir o jogo por uma varredura de processos nem oferecer uma lista de sessões: os controles já pertencem ao processo que os desenha. Não há mais fallback para uma janela externa nem canal de alternância global.
- Controles compatíveis são escritos atomicamente no TOML ativo com debounce de 200 ms. DLL, backend, runtime e opções da Steam não são alterados durante a partida. Um watchdog libera a captura se os quadros ou o desenho do painel pararem. A posição pode ser arrastada dentro da imagem do jogo.
- O executável do launcher incorpora o runner e as duas bibliotecas Rust. São materializados sob `FrameGeneration/overlay/<hash-do-conjunto>/` somente ao ativar a opção, preservando versões já referenciadas. O runner entra depois do isolamento do MAKO e dos post-processadores gerenciados; não restaura camadas implícitas globais. Falha de preparação deixa o comando/ambiente original do jogo prosseguir.
- **Estado experimental:** esta integração foi compilada e passou nos testes isolados de Rust e do carregador; ainda aguarda validação gráfica. A UI só permite optar quando confirma executável de 64 bits no host. 32 bits, Flatpak, HDR e OpenGL não são suportados nesta etapa. Wayland com listeners C e Wine/Proton XWayland são os adaptadores iniciais; KDE/GNOME, bloqueio de mouse e coexistência com MAKO exigem teste real antes de declarar suporte.
- Os testes escritos cobrem opt-in de perfis antigos, reaproveitamento do vínculo, arquitetura, tecla personalizada, posse de entrada, ordem MAKO/MangoHUD/overlay e preservação dos controles. Após a autorização de compilação, passaram 27 testes da camada/runner e 16 da bancada de encadeamento/compatibilidade, além das verificações isoladas do carregador. O teste gráfico em jogo continua pendente. Pesquisa e matriz de validação em [in-game-overlay.md](in-game-overlay.md).

- Na geração de quadros, ajustes de multiplicador, escala e desempenho reutilizam o perfil já vinculado quando backend, versão, DLL e ativação não mudam. O salvamento atualiza o TOML sem reenviar opções à Steam. Primeira ativação, desativação ou troca de componente continuam pelo fluxo de inicialização; pendências anteriores da Steam não são apagadas por um ajuste do perfil.

- Somente MAKO é oferecido em Geração de quadros e Configurações → Mods. Downloads e descoberta de versões do LSFG-VK foram removidos. Perfis de outros geradores são lidos apenas para avisar que o vínculo deve ser substituído ou desativado; não são convertidos nem executados pelo mini painel. Arquivos antigos existentes no disco não são excluídos por esta mudança. Variáveis de desativação de camadas concorrentes são mantidas para evitar conflitos.

- A revisão completa de Steam, Flatpak e launchers externos está em [mako-integration.md](mako-integration.md). Salvar uma alteração de ativação na Steam abre o comando exato e oferece aplicação manual verificada ou aplicação automática com reinício da Steam. A confirmação manual preserva o perfil. O multiplicador é limitado a 2–5 e o FPS alvo adaptativo a 10–1000, de acordo com a baseline MAKO 4.5.0.
