# Inicialização persistente de jogos Steam

O PeliGames fornece um auxiliar em Rust, `pelirun`, separado da interface.
Uma linha fixa nas opções de inicialização chama esse auxiliar e entrega a ele o
comando completo da Steam. A cada início do jogo, ele lê o JSON atualizado e
executa os wrappers, as variáveis e os argumentos configurados no launcher.
O PeliGames pode estar fechado durante essa execução.

## Configuração pelo usuário

1. Salve as opções desejadas nos detalhes do jogo.
2. Em **Configurar na Steam**, copie a linha gerada.
3. Cole em **Propriedades → Geral → Opções de inicialização**, preservando seus
   argumentos adicionais. Feche a tela de propriedades para gravar.
4. Use **Já adicionei · verificar** no PeliGames.

A linha tem este formato, com o caminho real daquela instalação:

```text
'/caminho/Steam/config/peligames-launcher/pelirun' --app 203160 -- %command%
```

Esse vínculo é feito uma vez por jogo e conta Steam. As próximas alterações de
MangoHUD, variáveis, argumentos, wrappers e seleção do componente MAKO atualizam
o JSON, sem editar novamente a linha e sem reiniciar a Steam. Uma cópia do
auxiliar atende a todos os perfis daquela instalação.

Ao migrar um comando antigo, a aplicação automática reconhece o script anterior
e as linhas geradas que consegue confirmar. Se encontrar um wrapper antigo que
não pode remover com segurança, preserva a linha e orienta a substituição manual
uma vez, evitando carregar duas cópias do gerador de quadros.

A alternativa de aplicação automática continua disponível e encerra a Steam
normalmente antes de editar seus arquivos. Ela serve para estabelecer o vínculo
ou mudar o Proton; não é necessária para atualizar perfis já vinculados.

## Próxima execução e alterações durante o jogo

| Alteração | Quando passa a valer |
| --- | --- |
| Ativar/remover um wrapper, camada Vulkan ou variável de inicialização | Ao reabrir o jogo |
| Instalar/reparar os arquivos de um mod | Segue o instalador e os backups existentes; o jogo precisa recarregar os arquivos |
| Ajustes do MAKO com recarga de configuração | Seguem a recarga oferecida pelo MAKO |
| Opções do MangoHUD com recarga suportada | Seguem o comportamento do MangoHUD |
| Versão do Proton | Continua sob controle de **Compatibilidade**, na Steam |

O auxiliar mantém o comando original da Steam, incluindo seu runtime e Proton.
Ele não muda o Proton selecionado pela Steam durante a partida. O mini painel
experimental e a camada Vulkan são componentes separados desse auxiliar.

## Arquivos e isolamento

Dentro de `Steam/config/peligames-launcher/`:

- `pelirun`: executável estável, incorporado ao pacote do PeliGames;
- `profiles/<AppID>.json`: comando com marcador, alterações do ambiente,
  argumentos adicionais e informações para atualizar o perfil.

As instalações nativa e Flatpak usam seus próprios diretórios. Os requisitos de
extensão e acesso aos perfis/DLL do MAKO no Flatpak continuam sendo verificados
pelo fluxo de preparação existente. O auxiliar não concede acesso ao host nem
contorna o isolamento do Flatpak. O funcionamento completo dentro de cada
runtime ainda precisa de teste real.

O JSON é substituído por renomeação atômica e usa permissões `0600`; diretórios
novos usam `0700`. O leitor verifica versão, AppID, tamanho, proprietário e tipo
do arquivo. O comando original é tratado como uma lista de argumentos, inclusive
caminhos com espaços. O auxiliar não usa `sh -c` para interpretar o perfil.
Variáveis desabilitadas podem ser removidas explicitamente do ambiente herdado.
Outras variáveis da Steam são mantidas.

O estado dos mods continua pertencendo ao gerenciador e aos backups existentes.
O auxiliar recebe suas exigências de inicialização; não replica o instalador de
mods nem modifica a conta Steam a cada execução.

Na redefinição, somente `profiles/` entra no mesmo registro de recuperação das
preferências. O executável permanece acessível à linha já salva na Steam. Sem
um JSON, ele executa o comando original; um perfil inválido gera um erro claro.
Se a redefinição for restaurada, os perfis anteriores também são restaurados.

## Compilação e verificação

O alvo principal Linux incorpora uma compilação estática do auxiliar para não
depender da versão de glibc do runtime da Steam. O `build.rs` compila somente o
pequeno pacote `src-tauri/launch-runtime`, em diretório separado. Ele exige o alvo
Rust `x86_64-unknown-linux-musl` (ou `aarch64-unknown-linux-musl` em ARM64) e suas
dependências já baixadas. É possível fornecer um executável estático previamente
construído pela variável `PELIGAMES_LAUNCH_HELPER`; o ELF é validado antes de ser
incorporado.

Foram adicionados testes para o contrato de perfis, atualização atômica,
preservação do comando/argumentos da Steam, remoção de variáveis, rejeição de
perfis inválidos e execução normal após redefinição. Os testes de integração do
auxiliar usam apenas comandos pequenos em pastas temporárias.

Validação realizada em 19/10/2026:

- Compilação da interface de produção e dos binários Linux, com o `pelirun`
  estático incorporado; geração do AppImage.
- Seis testes do auxiliar no alvo musl, 39 testes da integração Steam e oito
  testes de redefinição/recuperação passaram. Os diagnósticos locais ignorados
  por padrão foram executados separadamente para o jogo selecionado.
- Os dois binários e o AppImage passaram em `--verify-ui`, com os recursos da
  interface de produção incorporados.
- O auxiliar executou dentro do runtime Steam Linux. No Tomb Raider (203160),
  duas execuções receberam os perfis com MangoHUD ativado e desativado, mantendo
  o mesmo processo Steam e a mesma linha de inicialização. Na segunda execução,
  o jogo recebeu `MANGOHUD=0` e não carregou `libMangoHud`.
- A verificação Rust confirmou a linha salva na conta ativa e o Proton
  selecionado. Ao concluir, o perfil anterior de monitoramento foi restaurado
  e os marcadores temporários do teste foram removidos.

A validação real acima cobre a Steam nativa. O teste completo com Steam Flatpak
continua pendente; os testes de isolamento das instalações não substituem essa
execução dentro do sandbox.

## Referências da arquitetura

- [MangoHUD: peligames-run](https://github.com/flightlessmango/mangohud/blob/main/peligames-run.lpr):
  referência do fluxo de executável fixo e configuração externa. A implementação
  do PeliGames é em Rust e usa seu próprio contrato; não incorpora código Pascal.
- [Valve: runtimes Steam Linux](https://github.com/ValveSoftware/steam-runtime/blob/master/doc/reporting-steamlinuxruntime-bugs.md).
- [Flatpak: permissões de sandbox](https://docs.flatpak.org/en/latest/sandbox-permissions.html).
