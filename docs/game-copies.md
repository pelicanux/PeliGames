# Criar cópias de jogos e prefixos

Na biblioteca PeliGames, clique com o botão direito na capa e escolha **Criar cópia…**. Informe um nome, selecione a pasta de destino e escolha o método. Leia a explicação e marque a confirmação para liberar **Criar cópia**. A pasta deve ser nova e ficar fora da instalação original.

- **Cópia completa:** copia o jogo e o prefixo. É independente da origem e ocupa o espaço correspondente.
- **Reflink:** cria uma cópia independente, compartilhando blocos do disco até serem alterados. Exige suporte a reflink no mesmo sistema de arquivos. A instalação original continua disponível e pode ser removida sem afetar a cópia.
- **Overlay:** utiliza a instalação original como base e guarda as alterações de cada cópia separadamente. Requer `fuse-overlayfs`, `fusermount3` e acesso a `/dev/fuse`.

A interface oferece cópia completa e o método econômico disponível: reflink quando suportado; caso contrário, overlay. Para duplicar as alterações de um overlay existente, use cópia completa ou reflink. Crie novos overlays pela instalação de referência.

Nas novas cópias, a pasta escolhida é o próprio prefixo: `drive_c`, `dosdevices`, registros do Wine, `installation.json`, `copy.json` e logs ficam no mesmo nível, sem uma subpasta visível chamada `prefix`. No overlay, as pastas do Wine apontam para a montagem privada em `.peligames-overlay`; os registros ficam diretamente na pasta da cópia. Preserve a pasta oculta ao transportar ou fazer backup da cópia. Cópias antigas continuam usando sua estrutura original.

## Base dos overlays

Ao criar o primeiro overlay, a instalação original fica **reservada como base**, no mesmo caminho. O PeliGames impede executá-la, alterar suas configurações, abrir ferramentas Wine ou remover seu registro e seus arquivos enquanto existirem cópias vinculadas. Nos detalhes do jogo, **Prefixo de referência** permite localizar essa pasta e a indicação **Base reservada** mostra quais cópias a utilizam.

Não execute, atualize, mova nem altere a base por ferramentas externas. A reserva é controlada pelo launcher; não é uma proteção contra alterações feitas diretamente no sistema de arquivos.

Use **Desinstalar** na cópia para remover seu prefixo e as alterações exclusivas dela. A base e as outras cópias são preservadas. Um overlay não pode ser desinstalado mantendo seu prefixo, nem apenas retirado da biblioteca: isso deixaria seus arquivos dependentes da base. Ao remover o último overlay, a base volta a ficar disponível, no estado anterior às alterações das cópias. Essas alterações não são incorporadas à base.

## Contas e execução

Feche o jogo e as ferramentas Wine do prefixo antes de copiá-lo. A operação mostra andamento e permite cancelar, removendo a cópia incompleta quando consegue desmontá-la com segurança. Se uma desmontagem falhar, a pasta é preservada com uma mensagem de erro.

Cada cópia recebe identidade e prefixo próprios. Inicie cada uma pela respectiva entrada da biblioteca. O launcher acompanha sessões de prefixos diferentes e oferece uma ação de encerramento para cada instância. O mesmo prefixo permanece protegido contra duas sessões gerenciadas simultâneas.

As informações locais de login existentes são copiadas. Troque de conta em cada instância quando necessário; o jogo ou seu servidor podem restringir acessos simultâneos. Arquivos gravados fora do prefixo, pastas externas ligadas por links e serviços de rede podem continuar compartilhados: cópias não são sandboxes.

Comandos personalizados de inicialização e wrappers são limpos para não apontarem à instalação original; configure-os novamente se necessário. Cada cópia mantém seu próprio prefixo; um executável externo continua compartilhado, conforme explicado abaixo.

O recurso atende instalações registradas no PeliGames. Instalações gerenciadas por Steam, Heroic ou outras lojas não são duplicadas por este recurso.

## Recuperar uma biblioteca existente

Depois de reinstalar o PeliGames, use **Biblioteca → Localizar biblioteca** e selecione a pasta que contém suas instalações. Confira os resultados e clique em **Importar selecionados**. Use **Incluir outra pasta** se houver instalações ou bases em outros discos. Selecione também a base das cópias overlay.

O reconhecimento usa `installation.json` e, nas cópias, `copy.json`. Preserve esses arquivos junto de toda a instalação. Não é necessário exportar um cadastro separado: nomes, executáveis, prefixos e vínculos dos perfis ficam nos registros locais. A busca não procura jogos de outros launchers nem registra cada `.exe` como um jogo diferente.

- **Overlay:** preserve a base e as pastas completas de cada perfil, incluindo `upper`. Uma pasta de visualização montada não substitui esses arquivos. A base precisa estar disponível para recuperar a cópia.
- **Reflink e cópia completa:** preserve a pasta completa da própria cópia; a origem não é necessária para executá-la.
- **Mudança de disco ou pasta:** os registros encontrados permitem atualizar os locais da biblioteca e as referências entre base e cópias. Inclua os dois locais na mesma busca. Não mova pastas durante a importação.

Instalações já cadastradas são identificadas e não são duplicadas. Registros incompletos ou uma base ausente são informados antes da importação. A busca é limitada a seis níveis de subpastas; selecione uma pasta mais específica se necessário. Instalações no antigo formato `.peligames` devem ser atualizadas pelo launcher compatível antes da recuperação.

O Proton antigo pode precisar ser instalado ou substituído nos detalhes do jogo. Wrappers e comandos personalizados de inicialização são desativados na recuperação; configure novamente as integrações desejadas no sistema novo. Feche os jogos e ferramentas Wine antes de importar.

**Esses registros não são um backup dos jogos ou dos saves.** Formatar um disco sem preservar os arquivos elimina também as instalações e as alterações dos perfis. Para transportar reflinks mantendo a economia de espaço, o método de backup também precisa preservar o compartilhamento de blocos; uma cópia convencional pode ocupar o tamanho completo.


## Executável fora do prefixo

Se o executável estiver em outra pasta ou disco, a operação copia somente o prefixo Wine. A nova entrada usa o mesmo executável externo. Reflink é verificado entre o disco do prefixo original e o destino; a partição do executável não precisa oferecer reflink.

Registro do Windows, componentes e arquivos dentro de cada prefixo ficam separados. Configurações, atualizações e logs gravados na pasta externa do jogo permanecem compartilhados. Não mova nem apague essa pasta enquanto as cópias forem usadas. Desinstalar uma cópia pelo PeliGames preserva os arquivos externos. No modo overlay, a dependência da base do prefixo também permanece.
