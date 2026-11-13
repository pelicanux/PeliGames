# MAKO add-on

Adds MAKO runtime management and per-game frame generation to PeliGames. The
integration runs in its own service and contributes translated settings, game
controls and an overview summary. PeliGames keeps the launch contract and its
own overlay; this package owns MAKO configuration, version discovery, downloads,
profile generation, live adjustments and the MangoHUD layer adapter.

Download the component under **Manage**, select your `Lossless.dll` from the
public Steam release of Lossless Scaling, then enable MAKO for a game. The DLL
is not included. Select an installed runtime version to switch versions for that
game. Only one frame generator can be enabled per game.

The component and existing managed profiles retain their established paths.
Legacy PeliRun profiles route through the selected add-on. Removing or disabling
the add-on bypasses its launch integration. Runtime removal preserves game files,
profiles and the DLL; referenced or mapped runtimes cannot be deleted.

Reloadable settings can change while a game is running. Startup settings require
the game to close. Requires Vulkan and compatible graphics hardware. Flatpak
launchers require a matching MAKO extension and access to the profiles and DLL;
the support check shows the required user commands without changing permissions.

The distributable is a ZIP with the compiled service, bundled UI, translations,
manifest and license. Local experimental ZIPs can be imported from the add-on
store. The runtime download is separate from the add-on package.

Project: [MAKO](https://github.com/eugeniosegala/MAKO).
