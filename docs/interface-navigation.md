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
