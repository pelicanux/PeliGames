# Catálogo de instaladores do PeliGames

Receitas originais e declarativas para **Ubisoft Connect, EA app e Battle.net**. GOG GALAXY está excluído. Pesquisa revisada em 13/10/2026.

O catálogo está conectado à opção **Instalar jogo**: os três ícones abrem um assistente de três etapas. O executor usa receitas embutidas com hashes verificados, downloads HTTPS dos fornecedores, verificação Authenticode por `osslsigncode`, prefixo novo, Proton/UMU e registro do executável confirmado na categoria **Launchers**. Não consome scripts ou alterações de uma branch remota.

**Nenhum dos três instaladores foi executado ou certificado nesta implementação.** Todas as receitas conservam `validation.status: research-only`. A prévia simula o fluxo, sem downloads, instalações ou alterações da biblioteca real. `osslsigncode` e o pacote de certificados CA são pré-requisitos no sistema nativo; se faltarem ou o publicador não for confirmado, a execução é bloqueada.

Os componentes da receita são opcionais e precisam ser selecionados no assistente; ajustes históricos de compatibilidade permanecem desativados. Feche o launcher no fim da instalação para concluir a verificação. Em falha/cancelamento, os arquivos, prefixo e logs são preservados para diagnóstico. Para repetir, selecione um destino novo. Os caminhos alternativos glob da EA não são executados nesta versão; uma instalação fora do caminho conhecido retorna erro, sem registrar um executável presumido.

## Arquivos

- `index.json`: catálogo, política comum, fontes e SHA-256 de cada receita.
- `recipe.schema.json`: estrutura v1 e lista fechada de operações e ajustes.
- `recipes/*.json`: instalador oficial, candidatos a runner, componentes, alternativas e problemas documentados.
- `RESEARCH.md`: levantamento e interpretação das fontes.

O JSON separa **evidência**, **candidato a configuração** e **validação local**. A presença de um componente em uma receita comunitária não prova que todo jogo ou todo runner precise dele. Os ajustes opcionais começam desativados; instalar fontes e componentes exige consentimento e respeito às respectivas licenças.

## Validar

Na raiz do projeto:

```sh
python3 scripts/validate-launcher-recipes.py
python3 scripts/test-launcher-recipes.py
```

O validador é offline, usa a biblioteca padrão do Python e rejeita comandos livres, campos desconhecidos, referências inexistentes, travessia de diretórios, globs recursivos e downloads fora dos hosts iniciais autorizados. Ele verifica os hashes do índice. Implementa somente os recursos de JSON Schema usados neste catálogo; não substitui uma biblioteca genérica de JSON Schema.

Ao editar uma receita, revisar a alteração, aumentar `recipe_version` e atualizar seu SHA-256 e versão no índice. Ao publicar uma alteração do catálogo, aumentar `catalog_version`. O hash é dos bytes exatos do arquivo, incluindo a quebra de linha final.

## Contrato do futuro executor

1. Aceitar apenas uma versão de catálogo revisada, embutida ou vinculada a uma release/commit confiável do nosso GitHub. Os hashes no próprio índice detectam inconsistência; **não autenticam um catálogo remoto adulterado**. Não executar automaticamente alterações recebidas de uma branch mutável.
2. Validar o catálogo antes de exibi-lo ou usá-lo. Usar cache da última versão validada quando offline; o download do instalador ainda exige rede.
3. Resolver o runner selecionado para uma versão concreta e guardar versão, origem, hash do instalador, URL final e resultado das etapas. Não atualizar o runner no meio da instalação. Candidatos listados não implicam que todos já estejam disponíveis no gerenciador do PeliGames.
4. Para Proton fora da Steam, usar UMU e seu runtime. Configurar a identificação UMU somente depois de confirmar o identificador no banco oficial. Esta pesquisa não atribui GAMEIDs presumidos nem garante aplicação de protonfixes específicos.
5. Criar um prefixo exclusivo no diretório escolhido. Não sobrescrever um prefixo existente; não executar como root. Caminhos devem ser resolvidos dentro do prefixo, inclusive após symlinks. Globs devem ter busca limitada e correspondência única.
6. Baixar somente dos endereços oficiais. Validar HTTPS também nos redirecionamentos, auditar os hosts finais/CDNs antes de permitir execução, impor limite de tamanho e rejeitar respostas HTML. Verificar MSI/PE e assinatura Authenticode, incluindo **identidade esperada do publicador**, não apenas presença de uma assinatura. Os publicadores/certificados ainda precisam ser auditados antes de promover as receitas. Um hash calculado após baixar serve para auditoria, não prova autenticidade sozinho.
7. Executar MSI via `msiexec` ou EXE pelo runner selecionado, com argumentos separados e sem shell. Deixar o usuário aceitar a licença e concluir o instalador. Aplicar componentes apenas no prefixo selecionado e pelo mecanismo compatível com seu runner; não misturar Wine do sistema com um prefixo Proton inadvertidamente.
8. Ajustes de configuração são identificadores fechados, não caminhos ou scripts remotos. Mapear no código para arquivos conhecidos; editar por merge, com backup. Não apagar diretórios, caches, bibliotecas ou configurações automaticamente. Ajustes gráficos dependem do diagnóstico; não reduzir globalmente DXVK nem desabilitar a segurança do sistema.
9. Só registrar o launcher após confirmar o executável correto, seu formato e a conclusão do instalador. Verificar login e atualização em uma etapa de teste separada. Encerrar somente processos associados ao prefixo, sem `killall` global.

## Promoção de uma receita

Testar em prefixo novo, registrar distribuição, GPU/driver, runner/runtime exatos, checksum/assinatura do instalador e componentes realmente aplicados. Verificar instalação, login, atualização, reinício do launcher e pelo menos um jogo compatível. Publicar limitações conhecidas. Um teste bem-sucedido não prova suporte a todos os jogos, anticheats ou atualizações futuras.

As fontes do Lutris foram consultadas para identificar fatos e decisões de compatibilidade. Não foram importados scripts inteiros, comandos de remoção de arquivos nem o catálogo de terceiros. Os instaladores proprietários continuam hospedados pelos respectivos fornecedores.
