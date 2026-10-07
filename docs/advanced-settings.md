# Opções avançadas por jogo

Atualizado em 06/10/2026. Implementação própria; textos e código de terceiros não foram incorporados.

Referências: [ProtonPlus](https://github.com/Vysp3r/ProtonPlus), catálogo e validação consultados no commit `94eb7532b840d644e28b7ad5eb5f9469bf5c571e`; [Heroic](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/src/backend/launcher.ts), configuração de Wine-Wayland e HDR; [Gamescope](https://github.com/ValveSoftware/gamescope), argumentos de resolução, escala e FPS.

O catálogo mantém todas as quatro opções CachyOS e somente as opções marcadas nas demais imagens, mais HDR solicitado posteriormente. Todas começam desligadas, inclusive Gamescope. As marcações dos prints indicam inclusão, não ativação. Configurações da implementação anterior, sem `schema_version: 2`, são reiniciadas uma vez para remover as ativações padrão incorretas. Escolhas novas ficam salvas por entrada em `installation.json` e sobrevivem às varreduras.

Os seis blocos são Proton-CachyOS; Hardware e drivers; Compatibilidade Proton e Wine; Exibição e ferramentas de execução; Gamescope; Desempenho e monitoramento. Cada bloco permite retornar à seleção.

| Grupo | Opção | Comando / variável | Inicial |
|---|---|---|---|
| cachyos | DX9–11 com menor latência | `PROTON_DXVK_LOWLATENCY=1` | Desligada |
| cachyos | DX12 com menor latência | `PROTON_VKD3D_LOWLATENCY=1` | Desligada |
| cachyos | Camada Vulkan Anti-Lag 2 | `LOW_LATENCY_LAYER=1` | Desligada |
| cachyos | Reflex na camada Vulkan | `LOW_LATENCY_LAYER_REFLEX=1` | Desligada |
| hardware | Reduzir latência com Mesa | `ENABLE_LAYER_MESA_ANTI_LAG=1` | Desligada |
| hardware | Atualizar FSR para a versão 4 | `PROTON_FSR4_UPGRADE=1` | Desligada |
| hardware | Geração de quadros por aprendizado de máquina | `PROTON_MLFG_UPGRADE=1` | Desligada |
| hardware | Ajuste de geração de quadros para RDNA3 | `DXIL_SPIRV_CONFIG=wmma_rdna3_workaround` | Desligada |
| compatibility | Compatibilidade WoW64 | `PROTON_USE_WOW64=1` | Desligada |
| compatibility | Ativar integração OptiScaler | `PROTON_USE_OPTISCALER=1` | Desligada |
| compatibility | Integrar presença no Discord | `PROTON_DISCORD_BRIDGE=1` | Desligada |
| compatibility | Prioridade das DLLs do Wine | `WINEDLLOVERRIDES=dxgi=n,b` | Desligada |
| display | Executar com Wayland nativo | `PROTON_ENABLE_WAYLAND=1` | Desligada |
| display | Habilitar HDR com Wayland | `PROTON_ENABLE_HDR=1` | Desligada |
| performance | Painel de desempenho MangoHud | `mangohud` | Desligada |
| performance | Otimizações temporárias com GameMode | `gamemoderun` | Desligada |
| performance | MangoHud pelo ambiente Vulkan | `MANGOHUD=1` | Desligada |
| performance | Captura de jogo pelo OBS | `OBS_VKCAPTURE=1` | Desligada |
| performance | Prioridade maior para o processo | `PROTON_PRIORITY_HIGH=1` | Desligada |
| performance | Cache de shaders separado por jogo | `PROTON_LOCAL_SHADER_CACHE=1` | Desligada |

## Wayland e HDR

Wine-Wayland libera o controle de HDR, sem ativá-lo automaticamente. Para nossos runners Proton, HDR usa `PROTON_ENABLE_HDR=1`, seguindo o Heroic, e `DXVK_HDR=1` para exposição pelo DXGI. Desligados, esses controles removem as variáveis da execução; não prometem desativar recursos que o runner habilita internamente por padrão. DXVK implementa DX11 e VKD3D-Proton implementa DX12, compartilhando DXGI com DXVK. HDR pode funcionar nos dois, quando jogo, runner, drivers, compositor e tela oferecem suporte. Não há detecção automática da capacidade HDR do monitor ou validação completa da GPU.

## Gamescope

Modelo revisado conforme os prints do Heroic, implementado com código próprio no PeliGames. Substitui os controles anteriores de tela cheia, HDR, VRR e MangoApp por:

- Permitir aprimoramento de escala: revela método (FSR 1.0, NIS, escala inteira ou esticar imagem), largura/altura do jogo, largura/altura da saída e tipo de janela (tela cheia, sem borda ou janela).
- Habilitar limitador de FPS: revela limite de FPS ativo e sem foco.
- Captura forçada do cursor.
- Opções adicionais, sempre disponíveis.

As três opções começam desligadas e os campos adicionais vazios. Ao desligar uma opção seus valores são preservados, mas não emitidos na execução. Gamescope é utilizado quando escala, limitador, captura ou opções adicionais estão ativos. Não há botão separado de ativação. Campos numéricos e argumentos salvam ao sair do campo ou pressionar Enter.

Comandos: resolução do jogo `-w/-h`; saída `-W/-H`; FSR/NIS `-F fsr/nis`; escala inteira/esticar `-S integer/stretch`; tela cheia `-f`; sem borda `-b`; janela não adiciona flag; FPS `-r`; FPS sem foco `-o`; cursor `--force-grab-cursor`. Requer Gamescope moderno, com suporte a `-F/-S` (3.12 ou superior). Wayland adiciona `--expose-wayland`.

Opções adicionais são separadas em argumentos com aspas e escapes, sem shell. São rejeitados controles, operadores de shell, aspas incompletas, o separador `--` e flags já gerenciadas pelos campos próprios. O separador final é inserido exclusivamente pelo backend. Dimensões até 16384 e FPS até 1000; zero/vazio usa o padrão. Configurações do modelo anterior são substituídas por valores iniciais, mantendo outras escolhas avançadas.

Referência consultada: [construtor de argumentos do Heroic](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/src/backend/launcher.ts) e [interface Gamescope](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/blob/main/src/frontend/screens/Settings/components/Gamescope.tsx).

## Bloqueios

As regras compartilhadas `advancedRules.json` são verificadas na interface e no backend. Uma tentativa inválida mostra uma mensagem e não muda outras escolhas silenciosamente. Dependências e conflitos entre opções presentes no nosso catálogo, derivados das relações do ProtonPlus:

- Reflex requer a camada Vulkan de baixa latência.
- Ative FSR 4 antes de habilitar a geração ML de quadros.
- O ajuste RDNA3 requer a geração ML de quadros.
- Ative Wine-Wayland antes de habilitar HDR.
- Baixa latência DX12 e geração ML de quadros não podem ser combinadas.
- A camada Vulkan do CachyOS conflita com Anti-Lag do Mesa.
- A camada Vulkan do CachyOS conflita com a atualização FSR 4.
- Escolha apenas um modo de MangoHud: painel ou ambiente Vulkan.

Com Gamescope ativo, os dois modos de MangoHud são bloqueados. O tipo de janela é uma seleção única. DLLs aceitam apenas a sintaxe do Wine, por exemplo `dxgi=n,b;dinput8=n,b`. O backend rejeita opções desconhecidas, valores fora dos limites e ferramentas de execução ausentes. Ao salvar e iniciar, verifica se o script do runner declara as variáveis `PROTON_*` solicitadas. Isso identifica opções não oferecidas pelo runner, mas não garante suporte pelo jogo/GPU/driver. O backend monta argumentos e ambiente sem shell. O instalador não recebe configurações de execução do jogo.

Validação: build da interface; seis testes Bun; 25 testes Rust; cargo check nativo no Ubuntu. Os testes não iniciam jogos reais.

## Busca

O campo Busca ao lado do título pesquisa em todos os blocos por nome, descrição, categoria e variável, ignorando maiúsculas e acentos. Resultados comuns permitem ativar/desativar a opção com os mesmos bloqueios existentes. Para opções de Gamescope, o resultado abre sua seção. Limpar a busca restaura a navegação; Escape também limpa. Sem correspondência, a interface informa que nenhuma opção foi encontrada.

### Busca de Protons da Steam

Preferências → Protons da Steam permite habilitar a descoberta das instalações nativas e Flatpak, incluindo bibliotecas adicionais e `compatibilitytools.d`. A opção `scan_steam_protons` começa desativada e é salva automaticamente. Os runners do PeliGames e de outras pastas independentes continuam disponíveis. O rodapé do seletor mostra “Versões instaladas da Steam. Clique aqui”; o link abre e focaliza essa preferência sem descartar o formulário atual. A prévia guarda essa opção no armazenamento local do navegador; o aplicativo usa sua configuração persistente.

Verificado o link e a inclusão/remoção dos Protons da Steam no navegador; teste de varredura com pastas temporárias, build da interface e cargo check no Ubuntu aprovados.
