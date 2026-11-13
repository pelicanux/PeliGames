# DLSSNR · Mochizuki add-on

Independent PeliGames integration for [DLSSNR-AMD](https://gitlab.com/mochizuki0323/DLSSNR-AMD).
Its UI, translations, component downloads, model preparation, installation and
configuration live in this directory. The launcher loads the packaged service
and interface through the add-on SDK; it does not link this crate.

Install the integration ZIP through the store's **Import local add-on** action,
or use a published catalog package when available. Open **Manage** to download
or import the Linux backend. **Extract DLL / ZIP** accepts your compatible
`nvngx_dlssnr.dll` (310.8.0 or 310.9.16); **Import BIN** accepts previously
extracted, verified models. NVIDIA DLLs and model weights are not included.
The upstream installer requires `bash` and `python3`.

In a Windows game's details, open **Mods → DLSSNR → Mochizuki**. Choose the
executable folder, installation route, architecture and starting network, then
install. Startup, RenoDX and supported optimized-network quality settings can
be edited without reinstalling; route and starting-network changes require
repair. Repair uses fresh upstream defaults plus the selected installation
options. Standalone OptiScaler ownership blocks conflicting installations.
Use **How to use** for the game's PeliRun link and launch instructions.

The component keeps the existing managed
`components/Mods/DLSSNR/backend-files/rdna4/` location. Downloaded components are
shared between games. Removing the downloaded backend leaves installed game
copies untouched; download it again before repairing or uninstalling those
copies. Disabling/removing the integration package also preserves game files.
Older user configuration files remain readable by the launcher; the add-on
reads model caches and installed INIs directly instead of requiring the former
setup wizard.

The integration recipe is `manifest.json`; distributable entrypoints are
`bin/dlssnr-service` and `ui/index.js`. See the
[package instructions](../_infra/README.md#validate-and-package) for local ZIPs.
Build the service from `service/Cargo.toml`, build the UI with
`addons/_infra/tools/build-ui.ts`, and stage only those outputs and `LICENSE`.
Include required notices for any additional materials you choose to distribute.
The upstream backend is downloaded separately. Increment the recipe version
when changing an experimental package.

Tests use synthetic backends and temporary games, without the user's library:

```sh
CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR=/tmp/peligames-mochi-tests cargo test --manifest-path addons/dlssnr-mochizuki/core/Cargo.toml -- --test-threads=1
CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR=/tmp/peligames-mochi-tests cargo test --manifest-path addons/dlssnr-mochizuki/service/Cargo.toml -- --test-threads=1
bun test tests/dlssnr
```
