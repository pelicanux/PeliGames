# Camada de overlay PeliGames

Camada Vulkan em Rust para os controles MAKO dentro do jogo. É experimental, começa oculta e usa F8 por padrão para abrir e fechar. O painel pode ser arrastado pelo cabeçalho.

Para ativação, requisitos e limites, consulte o [guia de uso](../../docs/in-game-overlay.md). A camada não usa uma janela Tauri dentro do jogo.

## Compilar e verificar o módulo

As dependências estão fixadas no `Cargo.lock`. Com o toolchain Rust e as dependências disponíveis:

```sh
cargo test --locked --manifest-path tools/game-overlay/Cargo.toml
```

Os testes automatizados não substituem a validação gráfica com jogo, Proton e compositor. O build do aplicativo incorpora o módulo, o runner e o audit shim em alvos separados. `PELIGAMES_OVERLAY_BUNDLE` permite fornecer um conjunto previamente compilado da mesma arquitetura.

Os [avisos de terceiros](NOTICES.md) devem acompanhar a distribuição.
