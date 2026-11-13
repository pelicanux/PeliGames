# PeliGames UI SDK

The UI contract in `index.d.ts` uses the launcher's React instance and existing
controls. It is a type-only SDK: add-ons do not bundle React, React DOM or a second
design system. Start with `../template/ui/src/index.tsx`.

Export `createAddon(host)` from one self-contained ES module. Return an instance
with `apiVersion: 1`, `id`, `version`, `views` keyed by the manifest's declared routes,
and a `dispose()` method. The host provides `addon.id/version` so the template uses
the same identity as its package manifest. Do not put runtime imports of the SDK,
React or `react/jsx-runtime` in the output. The sample uses classic JSX compilation
with `React.createElement` bound from `host.React`.

```sh
bunx tsc --noEmit -p addons/_infra/template/ui/tsconfig.json
bunx tsc -p addons/_infra/template/ui/tsconfig.json
```

Stage `addons/_infra/template/ui/dist/index.js` at `ui/index.js` in the package payload.
The package must include its license, all notices and its service executable when
declared. The manifest at `addons/_infra/template/manifest.json` is the identity source
for both service and UI. Compilation is a contributor step; users download ready
packages and do not need TypeScript or Rust installed.

## Shared controls and language

`host.ui` supplies Categories, Row, Switch, Choice, Button and Icon. They render the
same controls as integrated PeliGames menus, including their keyboard behavior,
theme and switch design. Views receive `context` and `disabled`. Context identifies
the contribution target/route and may include the selected game. Do not copy host
CSS or create another React root. The host mounts the returned component inside
its own error boundary.

Use `host.i18n.createTranslator` for the add-on's messages. It uses the same
translation engine and language selection as the launcher, including interpolation,
plural rules and locale fallback:

```ts
const t = host.i18n.createTranslator({
  defaultLanguage: "en",
  resources: {
    en: {install: "Install", files_one: "{{count}} file", files_other: "{{count}} files"},
    "pt-BR": {install: "Instalar", files_one: "{{count}} arquivo", files_other: "{{count}} arquivos"},
  },
});
t("install");
t("files", {count: 3});
```

The package owns these catalogs; bundle them into its self-contained UI module.
Adding an add-on translation does not require editing the launcher's catalogs.
Messages missing in the selected locale fall back to `defaultLanguage`, whose
catalog must be present. A region such as `pt-BR` can also use a declared `pt`
catalog. Each catalog is isolated from host messages and other add-ons, and is
released with the view. Calls after disposal are refused. Catalogs accept up to
32 locales, 10,000 message entries in total and 1 MiB of key/text characters.

The host recreates the instance when language, route or game changes, so the
interface follows a launcher language change without a separate language selector.
`host.language` and `host.i18n.language` expose that resolved locale. Keep service
results structured so the UI can translate status/error codes. Keep contribution
titles concise; repository credits are exposed alongside the contribution heading.

## Lifecycle and services

Use `host.lifecycle.timeout`, `interval`, `listen` and `onDispose` for resources
owned by the view. React effects should also return their cleanup function. The
scope's AbortSignal is aborted immediately on close, invalidation or failure;
tracked timers/listeners are removed even if custom disposal fails or hangs.
Asynchronous callbacks must check `signal.aborted` before updating local state.
Callbacks scheduled through the lifecycle API are stopped if they throw.

Call `host.service.call(method, params)` to use the selected add-on service. The
host automatically tracks in-flight calls as busy, pins them to the verified
version/dependencies and rejects results after disposal. `setBusy` supports other
pending work; always clear it in a `finally` block. Never expose credentials in
view props, module source or service parameters.

Multiple views of the same selection share the service. Closing one preserves it
for the others; closing the last view releases its worker. A new view waits for
an earlier teardown of the same selection before issuing calls. The old selection
cannot stop a newly selected worker. Mutations emit `addon-store-changed`; open
views discard their instances when their own version or a dependency changes.

Factories/native load/import have bounded waits. A factory that resolves after
the view closes is disposed and cannot reactivate its host APIs. Rendering failures
are caught by the host boundary; initialization failures show a retry action.
Tracked cleanup is immediate; custom asynchronous cleanup is given a bounded
wait. Do not start persistent background jobs from a view's lifecycle: those need
an explicit task lifecycle beyond the view's service lease.

## Loading boundary

`addon_ui_contributions` returns metadata without reading or importing module code.
`addon_ui_load` accepts only an add-on ID and declared target/route, verifies its
selected package/dependencies and returns UTF-8 source limited to 2 MiB. The exact
returned bytes must match the registered file checksum. The frontend imports that
source through a temporary Blob URL and revokes the URL after import.

Only reviewed catalog packages may be distributed this way. JavaScript running in
the host WebView has the privileges of that context: the SDK is a lifecycle/design
contract, not an OS or JavaScript sandbox. It cannot interrupt synchronous infinite
JavaScript or clean resources created by bypassing the lifecycle contract. Unmount
releases the instance, listeners, timers and service lease; ECMAScript module-cache
retention is controlled by the WebView and is not a guarantee that all compiled
code memory is immediately returned.

## Game and file facilities

Game contexts carry an opaque `id`, the game root and, when resolved, the
`executableDirectory`. A binary-folder integration must wait for that directory;
do not silently substitute the game root. `host.game.chooseExecutableDirectory`
opens the launcher's existing chooser for the selected game.

Use `host.game.synchronizeLaunch` to save the integration's launch settings through
the existing PeliRun and launcher paths. Use `saveMenuShortcut` and `refresh` for
the selected game's preferences and library refresh. The host rejects a stale
game selection. `host.ui.LaunchProfile` provides the shared external-launcher
instructions without exposing a complete library record to the integration.

`host.files.openDirectory` opens an absolute directory. `host.ui.Path`, `Progress`
and `Projects` use the same path, download and credit controls as the launcher.
`host.styles.add` attaches module-owned CSS until the view is disposed; do not
copy global styles or leave document-wide nodes behind.

`host.ui.Choice` accepts `appearance="compact"` for the shared flat selector
with a selected-item checkmark. This material belongs to the host; integrations
do not need their own copies of its menu styles.

For integrations that prepare native launches, `host.game.launch.read(gameId)`
returns only the integration's own saved payload, execution availability,
executable probe and relevant core flags. `save(gameId, data)` persists that
payload through the game's existing profile facility and preserves other core
settings. Active game sessions cannot be edited. Enabling a frame-generation
integration disables competing configured generators while preserving their
tuning values. Pair this facility with the recipe's `native-launch` capability
and native service entrypoint; the UI never constructs executable launch code.

Translation catalogs belong to the add-on. Use `host.i18n.createTranslator` and
shared controls so changing the host language also changes loaded module views.

## Native service request context

The native host wraps each service call in `protocol::ServiceCall<T>`:
`{host: {component_directory}, request: <UI parameters>}`. The storage directory
comes from the selected recipe, never from the caller. A recipe can retain an
existing relative `components/...` directory through `component_directory`;
otherwise it receives `components/addons/<id>`. Integrations must not discover
configuration by reading the user's home directory.

File-changing game operations use the method `game.mutate` with an absolute
`request.directory`. The native host requires the `game-files` capability and
checks that the directory is writable, including overlay base protection, before
dispatch. Services remain responsible for validating their own actions, paths,
file ownership and configuration. These checks complement package review; they
do not turn native services into a sandbox.

Long downloads should return a bounded job identifier and expose progress through
short requests. Keep the view busy until its operation ends. Closing the last view
stops ordinary services. A reviewed `background-service` capability retains the
worker until the add-on is disabled, removed or the application exits, so closing
a recording panel does not interrupt its capture. Do not retain an unbounded job history.

## Local file selection and machine context

`host.files.chooseFile(label, extensions)` and `chooseDirectory(label)` open the
host's native picker when available. A cancelled picker returns `null`.
`host.files.machine()` provides detected GPU model/PCI addresses and Steam library
roots for component discovery. Treat detection as optional: keep manual selection
available when it fails. These methods reject replies after the view closes.

`host.game.launch.read(gameId)` includes the game's stable key and display name,
launcher constraints and the current integration's opaque settings. Old managed
wrapper envelopes are projected into the same contract without changing saved
profiles. The host owns activation and exclusive categories; the service owns
its feature settings and configuration files.


`host.ui.Shortcut` records a keyboard binding using the host control. Validate
bindings and collisions in the service before saving them. `host.files.openFile`
opens a local file with its associated application; a module must first validate
that the selected file belongs to the managed content it is presenting.
