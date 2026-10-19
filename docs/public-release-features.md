# Recursos da publicação 0.4.4

A publicação usa `release-features.json` com `experimental_frame_generation: false`. O mesmo arquivo é consumido pela interface, pelo launcher Rust, pelo build das bibliotecas incorporadas e pelo PeliRun. Alterar o arquivo para `true` restaura as builds locais; não modifica os pacotes já gerados ou o commit publicado.

- A aba de geração de quadros fica visível, desabilitada, com balão Em desenvolvimento.
- Gerenciamento MAKO e gravação do atalho ficam indisponíveis, inclusive pela API; nenhum registro/migração de atalho é iniciado nessa build.
- O build escreve payloads vazios do overlay, sem compilar ou incorporar as bibliotecas executáveis experimentais.
- Wrappers antigos gerenciados pelo PeliGames são ignorados na construção de comandos e pelo auxiliar PeliRun da publicação. Camadas/variáveis experimentais são filtradas; DLSSNR, MangoHUD, overlay Steam e os demais argumentos são preservados.
- Ao iniciar o launcher público, auxiliares e comandos de perfis Steam existentes são atualizados atomicamente. A configuração de origem do perfil permanece salva, permitindo reutilização em uma build local habilitada após salvar o perfil de novo.
- Desativar estes recursos no launcher não remove mods, runtimes ou configurações do disco e não impede programas independentes que o usuário execute por conta própria.

O teste de integração executa o PeliRun real com um wrapper antigo que criaria um arquivo marcador: ele não executa o wrapper, mantém DLSSNR e inicia o comando original. Há testes isolados do filtro para as duas políticas. `--verify-ui` expõe `experimental_frame_generation` e `overlay_payloads_available` para verificar cada pacote sem abrir interface ou inicializar o perfil do usuário.

Depois de publicar o commit e os três pacotes, somente o arquivo de recursos será restaurado localmente para `true`, sem commit/push, conforme solicitado. Os pacotes públicos continuam restritos. Os pacotes locais de teste precisam ser recompilados para incorporar essa restauração.
