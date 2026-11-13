# AFMF add-on

This independent add-on provides per-game controls for [afmf-linux](https://github.com/serialexperimentslainnnn/afmf-linux).
Its Rust service and interface are loaded from an installed package; the launcher
does not link the AFMF core or bundle its interface. The host supplies shared
controls, translations, game persistence and PeliRun facilities through the SDK.

## Use

Enable the AFMF add-on in the store, then open a game → Frame generation → AFMF.
Download the runtime in **Manage**, enable it in **Prepare**, and choose settings
in **Adjustments**. Settings save automatically for the next game launch. **How
to use** provides the PeliRun command for launching through another launcher.
The settings page also manages the shared runtime and installed versions.

AFMF generates one intermediate frame per real frame (2×). It requires a 64-bit
Vulkan graphics process, directly or through DXVK/VKD3D. A 32-bit Windows game
needs a Proton supporting the new WoW64 mode. Flatpak launchers are currently
unsupported. Disable other frame generators, including the game's own generator.
Enabling AFMF releases the other configured frame-generation integration.

Runtime versions can be selected and removed individually. Settings and other
versions are preserved. Removing an active version selects another compatible
installed version when available. Loaded native libraries cannot be removed
until their games close. If no runtime is available, the native launch entrypoint
starts the game without AFMF. A cached PeliRun command bypasses a disabled or
removed add-on.

The component storage retains `components/FrameGeneration/afmf/versions`, the
current-version pointer and legacy `afmf-<version>` directories. Existing game
settings are read through the generic host migration port and saved under the
integration's identifier. The original runtime manifest remains unchanged;
launch preparation creates a private manifest with an absolute library path.

## Service contract

Each stdio request uses `protocol::ServiceCall<T>` with the managed component
directory assigned by the host.

| Method | Request | Result |
| --- | --- | --- |
| `config.defaults` | `null` | Default settings |
| `config.validate` | Settings | Validated settings |
| `game.support` | Executable bitness, Windows and Flatpak flags | Advisory availability and status code |
| `launch.prepare` | Settings, runtime version, Gamescope flag and presentation inputs | Environment changes |
| `runtime.inspect` | `null` | Installed and available runtime versions |
| `runtime.check` | `force` boolean | Catalog-check job ID |
| `runtime.download` | Optional `version` | Download job ID |
| `runtime.select` | `version` | Updated runtime state |
| `runtime.remove` | `version` | Updated runtime state |
| `job.inspect` | `id` | Progress, completion result or error |

Downloads verify SHA-256, enforce archive limits and publish validated runtime
files through a staging directory. Catalog entries require official release URLs
and checksums. Runtime operations use a shared directory lock. The service keeps
one bounded background job; poll until completion before starting another.

The `native-launch` capability declares the service's second entrypoint:
`--peligames-launch <LaunchContext JSON> -- <game command...>`. PeliRun verifies
the selected package and dispatches to it. The service composes the AFMF,
Steam/mod and MangoHUD presentation chain, then replaces itself with the original
game command. It preserves arguments, exit status and the game's environment;
it does not execute a shell or require the launcher's UI process.

## Develop and package

`core/` owns configuration, compatibility, version management and downloads.
`service/` owns stdio dispatch and native launch. `ui/` owns its controls,
English/Portuguese catalogs and styles. Dependencies shared with other add-ons
live under `_infra/`; no module source imports the launcher's application code.
The archive dependency enables static XZ support without libarchive conversion.

```sh
CARGO_BUILD_JOBS=8 cargo test --manifest-path addons/afmf/core/Cargo.toml
CARGO_BUILD_JOBS=8 cargo test --manifest-path addons/afmf/service/Cargo.toml
```

Build the service on the supported Linux baseline. Stage it at
`bin/afmf-service` and build `ui/src/index.tsx` with `_infra/tools/build-ui.ts`
into `ui/index.js`. Include applicable license notices and `LICENSE`, then use
the SDK package tool with `manifest.json`. Only add a catalog artifact after
publishing and verifying the actual archive. No AFMF package is currently
published in the bundled catalog.

The afmf-linux runtime retains its own license and notices. The integration's
source uses the repository license.
