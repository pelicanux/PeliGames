# OptiScaler integration

`core/` contains installation, update, removal, detection, INI editing,
OptiPatcher and channel handling. It does not depend on Tauri or a user profile.
Callers provide explicit component and game directories. This library is linked
only by the independent service, not by the launcher. Writable-copy checks and
generic PeliRun profile facilities remain in the host.

The proxy DLL catalog belongs to `core/proxies.json` and is shared with this
integration's interface. The core and service sources have no resource imports from
the launcher's frontend. Their build dependencies are the neutral add-on SDK and
configuration/archive libraries; their Cargo paths can be adjusted when developing the
integration in a separate project.

`service/` uses the add-on SDK's bounded stdio protocol. Each call receives
`protocol::ServiceCall<T>`: managed component storage from the host, plus the
request parameters below.

| Method | Request | Result |
| --- | --- | --- |
| `game.inspect` | `directory` (absolute path) | Installed state, settings and inventory |
| `game.mutate` | `directory`, `action` | Installation/removal result, saved settings, or job ID |
| `runtime.inspect` | `null` | Selected download channel and cached package inventory |
| `runtime.channel` | `channel`: `stable` or `nightly` | Updated package state |
| `runtime.remove` | `channel` | Package state after removing that downloaded channel |
| `runtime.download` | `channel` | Job ID for the official component download |
| `job.inspect` | `id` | Bounded progress, completion result or error |
| `config.inspect` | `text` | Known settings from an INI string |
| `config.patch` | `text`, `key`, `value` | Updated INI string, preserving unrelated entries |

`action.operation` accepts `install` (`proxy`, `settings`), `upgrade`, `remove`,
`update` (`key`, `value`) or `patcher` (`enabled`). The launcher checks game
writability before dispatching `game.mutate`. The integration validates file
ownership and conflicts before changing the game. Downloading and removing a
cached channel do not update or remove copies already installed in games.

Downloads and OptiPatcher acquisition run as one bounded background operation per
service. Poll `job.inspect` until `done`; terminal errors release the operation
slot. The service does not access the user's home directory to discover storage.
The manifest retains `components/OptiScaler` without moving existing caches.

`ui/` owns the interface, translations, credits and styles. Its source depends
on the UI SDK and the module's own resources, not on launcher source files.
The `preferences` and `game` routes use the selected service, shared host controls
and generic game/PeliRun facilities. Build it with the shared UI builder and stage
the result at `ui/index.js`. The integration is not yet published in the official
catalog. The launcher discovers its menus from installed contributions; its old
OptiScaler-specific commands and bundled implementation are removed.

## Configuration coordination

The shared `peligames-mod-config` library owns the INI parser, sparse editing,
atomic file replacement and directory lock. Both the launcher NR controls and
OptiScaler mutations use `.peligames-config.lock` in the game directory before
reading and writing configuration. The OS releases the lock on process exit.
The empty lock file remains so concurrent writers continue to use the same inode.
Uncooperative external writers, including an in-game menu, are not synchronized
by this lock.

Rust 1.89 or newer is required for the portable file-lock API. Run the isolated
tests from the repository root:

```sh
cargo test --manifest-path addons/_infra/shared/config/Cargo.toml
cargo test --manifest-path addons/optiscaler/core/Cargo.toml
cargo test --manifest-path addons/optiscaler/service/Cargo.toml
```

The service integration test packages its test executable, verifies it through
the registry, starts it lazily and closes it when disabled. Game fixtures stay
in the system temporary directory.
