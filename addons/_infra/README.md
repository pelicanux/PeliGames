# Add-on package kit

The kit defines the package format, dependency resolution, version registry and
stdio protocol for independently distributed PeliGames integrations. It also
provides an offline validator and packager. Third-party mods and their licenses
remain separate from the PeliGames integration that manages them.

Services start on their first explicit request, after package verification and a
versioned handshake. Installed UI contributions appear in Settings → Add-ons;
their code loads only when opened. The existing integrated features continue to
use their current paths. Reviewed packages are published in the official catalog.

## Source layout

Each integration has its own directory under `addons/<id>/`. Shared development
and host infrastructure lives under `addons/_infra/`:

- `sdk/`: Rust contracts, package tooling and service lifecycle.
- `ui-sdk/`: interface and translation contracts.
- `shared/`: neutral libraries shared with the host.
- `template/`: starting point for a new integration.
- `config/`: reviewed catalog and manifest schema.
- `tools/`: interface build tooling.

Only actual integrations belong next to `_infra/`; infrastructure directories
are not installable add-ons.

## Create an integration

1. Copy `template/` to a new add-on directory.
2. Change the identifier, version, project URL, author and summary in `manifest.json`.
3. Implement the service using `sdk` with `default-features = false`. The example
   demonstrates initialization, a request, error handling and shutdown.
4. Implement the interface using the [UI SDK](ui-sdk/README.md). Build the service
   for each declared platform and compile the interface. Stage **only distributable output**
   in a separate payload directory, including its `LICENSE` and all required
   third-party notices. Do not include source caches, credentials or development files.
5. Validate the manifest and package the staged directory using the commands below.

The manifest uses `schema_version: 1` and `host_api: 1`. Its version is canonical
SemVer, without build metadata. Use readable lowercase IDs such as `my-addon`.
The `host_version` and dependency values are SemVer requirements. A package declares
its supported OS/architecture pairs and must contain each declared entrypoint.
Interface contributions declare a target, unique route and title. An optional
`descriptions` map supplies card descriptions by language tag (for example, `en`
and `pt-BR`). Keep these messages in the integration recipe; the host selects its
current language, then the base language, English or the original title. They require a
compiled `.js` entrypoint; source TypeScript is not a distributable entrypoint.
[`manifest.schema.json`](config/manifest.schema.json) provides editor completion and
structural validation. The Rust SDK also validates SemVer compatibility, dependency
graphs and package paths; use its validator before submitting an integration.

`capabilities` declares integration needs for review. It **does not provide an OS
sandbox**. Do not pass authentication secrets to add-on interfaces or manifests.

## Develop outside the launcher

An add-on can live in its own source project, with its own UI, service, tests and
release versions. Copy the template into that project, then point the service's
`peligames-addon-kit` dependency and the UI's type-only SDK import at the SDK copy
used by your development environment. These are build-time contracts, not imports
of launcher internals. Keep resources used by a service inside its own project;
do not reference files under the launcher's `src/` or `src-tauri/` directories.

The distributable package contains the compiled UI and service executable. The
host reads the manifest and connects them through the generic UI/service APIs.
The service owns feature-specific logic; it is not linked into the launcher just
to register a new integration. The UI uses the host's controls and theme so it
appears alongside the application's existing menus.

Installing or updating that package does not require rebuilding the launcher,
provided its declared host API and version requirements are supported. Closing
and disabling follow the lifecycle described below; removing a package removes
its integration files, separately from any mods previously installed in games.

## Validate and package

From the repository root, with Rust installed:

```sh
cargo run --manifest-path addons/_infra/sdk/Cargo.toml -- validate-manifest addons/_infra/template/manifest.json
cargo run --manifest-path addons/_infra/sdk/Cargo.toml -- validate-catalog addons/_infra/config/catalog.json
cargo run --manifest-path addons/_infra/sdk/Cargo.toml -- pack manifest.json /absolute/path/to/payload example-0.1.0-linux-x86_64.zip
```

`pack` packages existing output; it does not compile a service or interface. The
output ZIP must not already exist. The command prints the exact archive size and
SHA-256 digest for a reviewed catalog entry. To compile the example separately:

```sh
cargo build --release --manifest-path addons/_infra/template/service/Cargo.toml
```

Stage the resulting executable as `bin/example-service` and include the license
text. Compile the sample UI with `bunx tsc -p addons/_infra/template/ui/tsconfig.json`
and stage its `dist/index.js` as `ui/index.js`. Keep build output outside the payload
directory. Each platform needs its own archive, checksum and byte count. For a
service-only integration, set `entrypoints.ui` to null and clear `contributions`.

## Catalog and installation

`catalog.json` contains reviewed manifests and platform artifacts. Each artifact
declares `url`, `sha256` and `bytes`. Downloads are restricted to the official
PeliGames GitHub Releases repository. A checksum provided by an arbitrary download
source does not establish trust. Publishing an artifact requires review of its
contents, licenses, dependencies and capabilities before adding it to the catalog.

The host resolves compatibility and the complete dependency selection before any
download. It verifies the archive, checks every declared file and stages a new
version separately. The selected versions change together after preparation.
Previously installed versions remain available for explicit rollback. Different
bytes cannot silently replace an already installed version number.

The [OptiScaler integration](../optiscaler/README.md) owns its interface,
translations, installation, settings and downloads. The launcher discovers its
menus from package contributions and calls its independent service through the
generic protocol. It is available in the official catalog for Linux x86_64.

The [Mochizuki integration](../dlssnr-mochizuki/README.md) owns NR backend downloads,
model preparation, game installation and configuration. It uses the same generic
UI, service, translation and local-package contracts.

The shared storage map places integration packages under `addons/installed/<id>/<version>`
and the authoritative selection in `settings/addons.json` within the PeliGames
configuration directory. These paths are distinct from downloaded third-party
components, game files and user profiles. SDK callers supply explicit paths and
serialize registry operations; the native host performs that serialization.

Disabling/removing an integration does not uninstall mods from games. Removal of
an enabled version, a referenced version or a dependency still needed by an enabled
add-on is refused. Host consumers can pin versions for profiles/jobs. The manager
can restore an interrupted removal before committing a new operation. No service
is started by package installation itself.

## Service protocol

Use one JSON-RPC 2.0 object per line on stdin/stdout. Send diagnostics to stderr.
`peligames_addon_kit::service::serve` provides the bounded protocol loop,
handshake, response envelopes and shutdown handling for service implementations.
The maximum message length is 1 MiB. A service must initialize before handling
feature requests and terminate on `shutdown` or EOF. A service loader must verify the returned
`host_api`, `addon_id` and `addon_version` against the selected package.

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"host_api":1}}
```

```json
{"jsonrpc":"2.0","id":1,"result":{"host_api":1,"addon_id":"example-service","addon_version":"0.1.0"}}
```

An interface/service loader must verify the registered receipt and payload through
the SDK immediately before loading or executing an entrypoint. An `enabled` registry
flag means the package is selected for use; it does not mean a process is running.

## Host service lifecycle

Enable the SDK's `runtime` feature in the launcher. Service executables need only
the protocol/manifest API and should keep `default-features = false`.

The native UI can call the generic commands without registering feature-specific
Tauri commands for each add-on:

```ts
import { invoke } from "@tauri-apps/api/core";

const result = await invoke("addon_service_call", {
  id: "example-service", method: "echo", params: { value: 42 },
});
await invoke("addon_service_stop", { id: "example-service" });
const statuses = await invoke("addon_service_status");
```

`initialize` and `shutdown` are reserved to the host. The first request validates
the selected package and its dependency closure, starts the executable directly
without a shell, and verifies its declared API/identity/version. Later requests
reuse that worker. Dependencies are verified without automatically starting their
services. Querying status or installing a package does not start any service.

Each integration has its own serialized request stream and bounded queue (eight
waiting requests by default). Startup/verification has a ten-second deadline;
requests have a fifteen-second deadline. Both request and response messages are
limited to 1 MiB. Cancelling a caller does not abandon a partially read response.
A valid service error is returned to the caller while keeping the session usable.
Malformed output, timeout, unexpected exit or a mismatched handshake stops that
worker. The host does not automatically restart a failed worker: explicitly stop
it before retrying. Other integrations keep their independent queues.

Close a service when its feature finishes using it. The launcher also stops
affected workers when their selected version/dependencies change, when disabled,
and on normal application exit. Graceful shutdown has a one-second deadline
(eight seconds for declared background services),
followed by forced termination and a bounded reap. Unix workers get their own
process group so descendants cannot keep the integration alive. Windows currently
terminates the direct child; equivalent descendant cleanup needs a Windows job
object implementation before enabling Windows packages with subprocesses.

SDK embedders must serialize submission with registry mutations. Submit under that
lock, release it before awaiting `Pending::wait`, and call `reconcile` after changing
selection. Disable and reconcile before removing package files. The Tauri host
implements this coordination. The runtime is created on demand; it is not started
by the application's welcome/library screens.

Workers receive a limited desktop environment, with arbitrary credential variables
and AppImage loader hooks omitted. This does not restrict their filesystem access
or create a sandbox. Stderr is drained with bounded buffers and discarded; it is
not automatically appended to launcher reports. Status exposes identity, version,
phase, PID and error category. Request parameters are not logged by the runtime.

## Test

### Native launch integrations

A reviewed recipe declaring `native-launch` must provide a service entrypoint.
PeliRun verifies the selected package and its dependency closure, then invokes
that service with `--peligames-launch <LaunchContext JSON> -- <original argv...>`.
The context contains the managed component directory, opaque integration
settings, monitoring/Gamescope flags and enabled core option identifiers.
PeliRun replaces the context's storage path with the one declared by the recipe.
Services must preserve literal arguments and use process replacement for the
game, without introducing a shell. Native launch does not start a stdio worker
or depend on the launcher remaining open. Disabled or removed packages bypass
this entrypoint; modified or incompatible active packages fail verification.

The host persists module data in `addon_launch`, bounded to 16 integrations and
32 KiB per payload. Module-specific validation belongs to its service. Legacy
top-level values can be read by identifier without linking their schema into
the host. Package selection changes refresh exported launch profiles.

`jobs` supplies bounded operation tracking and `protocol::TransferProgress` for
services that download components. `files` supplies managed directory checks
and a Linux loaded-library removal guard. These are neutral facilities shared
by integrations; they do not implement any particular mod's behavior.

```sh
CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR=/tmp/peligames-addon-tests cargo test --manifest-path addons/_infra/sdk/Cargo.toml
CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR=/tmp/peligames-addon-tests cargo test --manifest-path addons/_infra/sdk/Cargo.toml --features runtime -- --test-threads=1
CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR=/tmp/peligames-addon-tests cargo test --manifest-path addons/_infra/template/service/Cargo.toml
```

Tests use temporary fixture profiles and never discover the user's configuration.

## Store categories and catalog updates

Use `category` (`mods`, `frame-generation` or `captures`) to place an integration
in the store. Independent backends can share a readable `group`, such as `dlssnr`.
Declare `game-mods`, `game-frame-generation`, `game-captures` or `game-overview`
contributions for the game interface. A game tab is available when at least one
enabled integration contributes to it. Performance and the core game settings
remain part of the launcher.

The store refreshes its reviewed catalog from this repository's official
`addons/_infra/config/catalog.json`. Publishing a compatible integration and its
catalog entry does not require a new launcher release. Each artifact must still
come from the configured official release origin and match its declared size and
SHA-256. The host validates and atomically caches the catalog; an offline request
preserves the last valid cache. A damaged cache falls back to the bundled catalog
and leaves installed packages manageable.

## Build a self-contained interface

```sh
bun addons/_infra/tools/build-ui.ts addons/optiscaler/ui/src/index.tsx OUTPUT.js
```

The builder compiles one ESM factory, binds React to the host instance, and includes
module-owned CSS and translation catalogs. Styles are attached to the view's
lifecycle and removed when it closes. Renderer imports are refused. The output
path must not already exist; stage the result at the UI entrypoint declared by
the recipe before packaging it. Users receive this compiled output, not a frontend
build environment.

## Install a local experiment

Open the add-on store in the left sidebar and choose **Import local add-on**.
Select the ZIP produced by `pack`, review its name, version and author, then
choose **Import and enable**. No GitHub repository, release or catalog entry is
required. A mod's upstream ZIP is not an add-on package: stage the integration's
compiled UI/service and package its manifest with the SDK first.

Local packages are marked **Local · experimental**. They use the same compatibility,
file-inventory, checksum and extraction checks as catalog packages. Reviewing a
file does not execute its service; installation also checks that the ZIP has not
changed since selection. Enable, disable, rollback and removal use the same store.
Dependencies must already be installed in compatible versions. Increment the
manifest version when changing an experiment: an installed version cannot be
replaced with different contents. Local import does not upload the ZIP anywhere.

The MAKO integration is maintained in [`../mako/`](../mako/README.md). Its runtime,
profiles and tuning logic are owned by that package; the launcher provides only
shared activation, file selection, translations and verified launch dispatch.


## Background services

Declare `background-service` for integrations that must keep an explicit user
operation running after their panel closes, such as recording or replay. Workers
remain lazy: installing the package does not start them. Closing the last view
releases its UI resources while retaining the worker. Disabling, removing or
changing the selected package, and exiting PeliGames, stop the worker. It has up
to eight seconds to finalize its own child processes before forced cleanup.
Do not rely on a view's timers to manage a recording's lifetime.

## Publish a release

Build each service and the SDK packager in a compatible Linux environment, using a
shared Cargo target directory. Remap build paths so binaries contain no personal
source paths. Generate third-party Rust notices before staging a release. Then run:

```sh
bun addons/_infra/tools/package-release.ts TARGET_DIR OUTPUT_DIR addons-v0.1.0
```

The tool stages only compiled services, self-contained interfaces and license
notices. It packages Linux x86_64 artifacts and validates the generated catalog.
Test each service handshake and inspect every archive before publishing. Upload
the ZIPs to the matching GitHub release first; publish its catalog only after the
assets are downloadable and their byte counts and SHA-256 digests match.
