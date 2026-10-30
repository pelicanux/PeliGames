<p align="center">
  <img src="public/images/readme-header.svg" alt="PeliGames" width="900" />
</p>

> **0.5.2 experimental:** constant-quality capture presets with automatic codec selection, AppImage/Wayland animation fixes, artwork recovery, and improvements to data organization and reset. See the [0.5.2 release notes (Portuguese)](docs/releases/0.5.2.md). Capture, frame generation and the overlay are experimental; compatibility depends on the game, GPU, Proton and desktop environment. See the [usage and limitations guide](docs/overlay.md).

<p align="center"><a href="README.md">Português</a> · <strong>English</strong></p>

Windows game launcher for Linux with Proton, a game library, Pelinstall, Wine tools, Nexus Mods integration and the DLSSNR-AMD mod manager.

<p align="center">
  <img src="public/images/interface-preview.webp" alt="PeliGames interface with the game library grouped by source" width="1200" />
</p>

## How it works

1. **Library:** PeliGames brings together games from Steam, the native Epic Games Launcher for Linux, Heroic, Lutris, Bottles and Faugus, plus the ones you install with it.
2. **Installing Windows games and programs:** pick the installer (`.exe` or `.msi`), a name, a folder and a Proton version. PeliGames prepares the prefix, runs the installer and finds the game's executable by itself. When several executables are possible, it asks.
3. **Launchers (EA app, Battle.net, Ubisoft Connect, Epic):** games installed from inside a launcher appear under **Programs**, ready to open directly. When one of them is added to the library, it gets a profile of its own (separate prefix, Proton and mods) without duplicating the launcher's files.
4. **Nexus Mods:** in **Mods → Nexus Mods**, add the game, pick mods from the catalog (or import an archive) and click **Install**. PeliGames checks the requirements, runs the FOMOD installer when there is one and deploys the files following the game's rules, the same ones the Vortex game extensions use. Enabling, disabling and removing mods never changes the game's original files. See the [supported games](docs/supported-games.md).
5. **Problems:** **Report error**, in the DLSSNR-AMD, frame generation, performance, captures, options and advanced tabs, gathers technical settings and logs for that area. Nexus mod reports remain available. Check the filtered text with **Review submission**, then confirm to publish in the Discord support forum; no video, audio or screenshots are attached. Launcher errors also show up in the notification bell.

## Installation

Download the AppImage, RPM or DEB from [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM and DEB install the `peligames` and `pelinstall` binaries in `/opt/PeliGames`, with commands in `/usr/bin` and shortcuts in the system application folders. From a terminal, use `peligames` or `pelinstall /path/to/program.exe`.
- The AppImage contains both modules. Make it executable and open it. It runs as is, but it can also be installed for your user, as Gear Lever and AppManager do.
- The AppImage carries its own WebKitGTK (2.52) and needs glibc 2.39 or newer: Ubuntu 24.04, Linux Mint 22, Debian 13, Fedora 40 and later, Arch, CachyOS, Nobara and Bazzite. The DEB and RPM packages have the same requirement.
- To open a Windows executable in the installer: `./PeliGames.AppImage --install /path/to/program.exe` (once installed, `peligames --install /path/to/program.exe`).

### Installing the AppImage

On the first start, the welcome screen offers **Install and open**. The same button is under **Settings → Tools** for anyone who skipped it. No root or FUSE is needed:

- the AppImage's contents are extracted to `~/.local/share/PeliGames/app` and run from there, which starts faster than the compressed file;
- `~/.local/share/PeliGames/peligames` is the command shortcuts use, and `~/.local/bin/peligames` links to it for the terminal (a `peligames` file of your own there is never replaced);
- PeliGames and Pelinstall show up in the applications menu, under Games; game shortcuts and Nexus links now open the installed copy;
- the downloaded `.AppImage` file is removed after extraction.

Updates download the new AppImage, extract it alongside and swap the `app` folder. The previous version stays in `app.previous` until the new one opens, and comes back on its own if the new one fails to start.

To uninstall, delete `~/.local/share/PeliGames`, the `~/.local/bin/peligames` link and the `peligames-*.desktop` and `pelinstall-appimage.desktop` shortcuts in `~/.local/share/applications`. The launcher's data is kept apart, in `~/.config/PeliGames`.

Proton, UMU and Winetricks are downloaded when needed. Settings, tools, mods and interface data live in `~/.config/PeliGames`; delete that folder to start from scratch. Prefixes and games stay in the folders the user chose. SteamGridDB covers need an optional personal key, set in Preferences.

## Nexus Mods

Connect your personal API key in **Nexus account** and pick mods in the **Catalog**, or import ZIP, 7z and RAR archives with **Local Mod**. To receive downloads from the website, let PeliGames handle Nexus links and use **Mod Manager Download**.

Free accounts confirm each download on the Nexus Mods page; Premium accounts download directly. Follow the queue in the Nexus downloads panel and, in the mod list, check the requirements before **Install**, **Enable** or **Disable**.

Independent integration, not affiliated with Nexus Mods. Report issues to PeliGames, not to Nexus Mods.

Support depends on the game and on the package layout. The API key is stored only in the system keyring (the AppImage needs `secret-tool` and an available keyring). Use **Logs** or **Report error** to diagnose problems.

Under **Settings → Tools** you can create a ZIP backup of the games with Nexus mods installed, or restore one saved on another disk. The per-game recovery option keeps a hidden `.peligames` folder on the games' disk, so the records come back after rescanning on another distribution.

## MAKO frame generation and PeliGames overlay

Open **Library → Game details → Frame generation**. Install MAKO from **Settings → Mods → MAKO**, provide your own public Steam Lossless Scaling installation (`Lossless.dll`), and enable it for the game. Changes save automatically. Multipliers (2×–5×), adaptive target FPS, motion scale and the lightweight model can be adjusted from the launcher while a game is running with MAKO.

Enable **PeliGames overlay** before launching the game. **F8** shows/hides its draggable panel; the shortcut is configurable and is local to the focused game. Esc does not close it, and no global desktop shortcut is registered. Startup options require reopening the game. This overlay is experimental: input capture, cursor behavior and compatibility still need testing across games and compositors. Linux 32-bit graphics hosts are not supported by the overlay; Windows 32-bit executables are allowed for experimental testing with a 64-bit new-WoW64 graphics host. Flatpak Steam is supported (the overlay is copied into PeliRun's folder inside the sandbox). MAKO frame generation itself works in Flatpak through its Vulkan layer extension, but its in-game control panel and other Flatpak launchers do not load the overlay yet.

> **The overlay is meant for offline games.** It works on the Linux side without injecting a DLL into the game, but it loads libraries into the process and intercepts keyboard and mouse for its interactive panel. In online games with anti-cheat or their own protection, this may be seen as cheating and lead to account restrictions or bans. **Using it in online games is at the player's own risk**; PeliGames is not responsible for penalties applied by games or servers.

**PeliRun** exports readable per-installation IDs, such as `tomb-raider`, for Steam, Heroic, Lutris and compatible wrapper/command-prefix fields in other launchers. Copy the command shown by PeliGames. Once linked, profile changes apply on the next game launch without repeatedly restarting Steam; live changes depend on the backend. Steam's Proton selection remains in Steam → Compatibility. See [PeliRun](docs/steam-launch-runtime.md).

## DLSS 5 Neural Rendering on AMD cards (DLSSNR)

Open **Mods → DLSSNR-AMD** in a game's details to install and configure Neural Rendering. PeliGames manages the backend, models and launch options for each installation.

> Experimental projects (versions 0.0.x), not affiliated with NVIDIA or AMD. **Do not use them in online games with anti-cheat**: injected DLLs can get an account banned. The installer puts files into the game folder; **Remove** deletes only what it installed.

### Cards and backends

| Card | Backend | Authors' reference |
|---|---|---|
| RX 9000 (RDNA 4) | [DLSSNR-AMD by mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD) | Mesa 26.2 or newer and recent Proton; 0.0.8 tested with GE-Proton 11-11 and Mesa 26.2.5 |
| RX 7000 (RDNA 3) | [DLSSNR-RDNA3 by mauri870](https://github.com/mauri870/DLSSNR-RDNA3/blob/rdna3/README-RDNA3.md) | Separate RX 7000 fork; tested by its author on RX 7900 XTX with Mesa 26.2.3 |

The RDNA 4 backend uses FP8 instructions; the RDNA 3 fork adapts processing to FP16. PeliGames shows options according to the installed package's capabilities. The projects do not necessarily provide the same features.

### Prepare the model and install in a game

1. Open **Settings → Add-ons → DLSSNR-AMD** and download your GPU's backend. **Import local backend** accepts a folder containing the project's Linux packages or its extracted architecture subfolders.
2. In the setup wizard, choose **Extract from a DLL** with your `nvngx_dlssnr.dll`, or **Use existing file** with valid `.bin` models. These options become available once the backend is ready. Models are checked against SHA-256 hashes.
3. In the library, open the game's details and **Mods → DLSSNR-AMD**. Check the mod destination, select the mode, architecture and starting network, then click **Install** or **Repair**.
4. Complete **Configure in Steam** when prompted, or use the PeliRun profile in the other launcher. Then launch the game.

NVIDIA's DLL and model weights **are not included with PeliGames or the backend download**. Obtain a compatible DLL from a game that supplies it. The backend installer uses `bash` and `python3`.

| Extraction DLL | Models imported by the launcher |
|---|---|
| **310.8.0** | `dlssnr.bin` — old model |
| **310.9.16** | `dlssnr.bin` and `dlssnr-small.bin` — old and new models |

Files stay in `~/.config/PeliGames/components/Mods/DLSSNR/backend-files/<rdna3|rdna4>/<32|64>/dlssnr-amd/`. Keep both `.bin` files together when importing the 310.9.16 models. Valid cached models can be reused for later installations.

### Starting network and launcher options

With Mochizuki backend 0.0.8, **Starting network** offers:

| Option | Network | Required model |
|---|---|---|
| **New** | `CC_SMALL_FP8` | 310.9.16 |
| **Experimental** | `CC_SMALL_FP8_EXPERIMENTAL` | 310.9.16; derived weights prepared during installation |
| **Old** | `CC_SILVER_AARDWOLD` | 310.8.0 or 310.9.16 |
| **int4** | `CC_SILVER_AARDWOLD_INT4_MIXED` | Old model; derived weights prepared during installation |

The **Experimental** network may change the image. It appears only with a supporting backend. Older backends and RDNA 3 retain the selectors and options their scripts support; int4 cannot be combined with the new model.

| PeliGames setting | When it applies |
|---|---|
| **Start enabled** and **RenoDX** | With an installed mod, saved automatically for the next game launch |
| Mode, architecture, starting network and menu shortcut | When installing or repairing |
| **VORT / LumeniteFX**, on compatible ReShade routes | When installing or repairing; VORT is bundled, while LumeniteFX requires an additional download from its official project |

**Installing or repairing replaces `dlssnr-amd.ini` with backend defaults and the installation choices.** Keep a copy of custom settings before doing this. Updating the launcher's backend does not automatically update files installed in games: repair each game that should receive the update.

### Choose the mode

This table describes Mochizuki 0.0.8 routes for RX 9000. With RDNA 3 or older packages, follow the routes and controls available in the installed backend.

| Game | Launcher mode | In-game menu |
|---|---|---|
| 64-bit DirectX 11/12 or Vulkan, with DLSS, FSR or XeSS available | **OptiScaler** | **Insert**, by default, opens OptiScaler; **End** opens NR Control |
| DirectX 10/11/12 without a compatible upscaler | **ReShade** | **Home → Add-ons**, by default |
| Vulkan without a compatible upscaler | **Vulkan** | **Home → Add-ons**, by default |
| DirectX 9 | **DX9** | **Home → Add-ons**, by default |

Mochizuki 0.0.8 uses original OptiScaler with the `dlssnr-amd.asi` plugin on the OptiScaler route. Older backends may use the OptiScaler-NR fork. Enable a compatible upscaler in the game's settings for this route. The OptiScaler route does not support 32-bit games; PeliGames offers 64-bit only on RDNA 3. OpenGL is unsupported.

### In-game adjustments with Mochizuki 0.0.8

The launcher keeps preparation and startup controls. **Layers, RenoDX/wilsjo2 processing, fast or legacy passes, HUD protection, visual comparison and cost/VRAM diagnostics** are adjusted in **NR Control** on the OptiScaler route, or **ReShade → Add-ons** on the other 0.0.8 routes. See the [official 0.0.8 release notes](https://github.com/mochizuki0323/DLSSNR-AMD/releases/tag/v0.0.8-linux-preview) for details and limitations.

On the OptiScaler route, **Insert** controls upscaling and frame generation; **End** opens NR settings. Use the NR menu to switch networks during play; **Ctrl+F11** is the default switching shortcut in supporting backends. Settings are stored in `dlssnr-amd.ini` in the mod's installation folder.

### Launching through Steam and other launchers

PeliGames saves the installer's printed launch options in the game's profile, replacing previous NR options. Complete **Configure in Steam** or use the command provided by **PeliRun** once. Once linked, later profile changes apply when relaunching the game, without repeating setup or restarting Steam. Proton selection remains in Steam or the corresponding launcher. See the [PeliRun guide](docs/steam-launch-runtime.md).

Do not manually add NR Vulkan layers to the 0.0.8 OptiScaler route. ReShade routes and older backends may require them; follow the profile generated by the installer. If synchronization fails, **View log** shows the options needed for manual setup.

### Processing cost

Authors' published network-only times, in milliseconds per frame:

| GPU and network | 1080p | 1440p | 4K |
|---|---|---|---|
| RX 9070 XT — New | 3.27 ms | 5.53 ms | 12.67 ms |
| RX 9070 XT — Experimental | 2.90 ms | 4.84 ms | 11.09 ms |
| RX 7900 XTX — Old | 15 ms | 26 ms | 56 ms |
| RX 7900 XTX — Old model's int4 | 13.3 ms | 22.8 ms | 50.8 ms |

These references use different versions and measurement methods, excluding the rest of the game and the mod's full processing cost. They do not represent final FPS or guarantee the same result on another machine. Check the conditions in the [RDNA 4](https://github.com/mochizuki0323/DLSSNR-AMD#performance) and [RDNA 3](https://github.com/mauri870/DLSSNR-RDNA3/blob/rdna3/README-RDNA3.md#what-it-costs) references. Reducing the network's processing resolution can lower its cost while affecting the image.

### When something does not work

- Allow network compilation to finish on first launch; its duration depends on the backend and machine.
- Check `dlssnr-amd.log` and `OptiScaler.log` or `ReShade.log` in the mod folder. Installed files alone do not prove that NR is processing the image.
- If an option is missing, check the selected GPU, installed backend and models available in the wizard.
- Use **Report error** in the game's DLSSNR-AMD area. The report gathers settings and logs; review the filtered data before confirming submission to Discord support. **Copy a report** in the 0.0.8 backend's diagnostics also generates a local report.

## Supported games

PeliGames installs Nexus Mods mods in **215 games**, with modules based on the Vortex game extensions: Fallout, Skyrim, Starfield, Cyberpunk 2077, Baldur's Gate 3, Stardew Valley, Elden Ring, the Resident Evil and Monster Hunter families, and dozens of Unreal and Unity games.

| Completion | Games |
|---|---|
| 100% — verified in the real game | 5 |
| 90% — parity with the Vortex extension | 67 |
| 80% — some extension features still pending | 29 |
| 70% — generated from the family template | 113 |
| 60% — Vortex support not confirmed | 1 |

**[See the full list of supported games →](docs/supported-games.md)** (in Portuguese)

## Building from source

1. Install the Tauri 2 system dependencies:
   - Debian/Ubuntu: `sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`
   - Fedora: `sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel` and `sudo dnf group install c-development`
2. Install [Rust](https://rustup.rs) and [Bun](https://bun.sh).
3. Get the code and the interface dependencies:
   ```
   git clone https://github.com/pelicanux/PeliGames.git
   cd PeliGames
   bun install
   ```
4. Build the interface (it also generates the icons):
   ```
   bun run build
   ```
5. Build both executables, with the interface embedded:
   ```
   cargo build --release --bins --features custom-protocol --manifest-path src-tauri/Cargo.toml
   ```
6. The executables are in `src-tauri/target/release/`: `peligames` opens the launcher and `pelinstall /path/to/program.exe` opens the installer.

To build packages (AppImage, DEB and RPM) in `src-tauri/target/release/bundle/`, use `bun run tauri build`.

## Credits and license

PeliGames is developed by Pelicano and distributed under the [GNU GPL 3.0](LICENSE). Distributed derivative versions must provide the corresponding source code under the same license.

It includes [DLSSNR-AMD by mochizuki0323](https://github.com/mochizuki0323/DLSSNR-AMD) and [DLSSNR-RDNA3 by mauri870](https://github.com/mauri870/DLSSNR-RDNA3).

Third-party components keep their own licenses. The [full credits and notices](licenses/README.md) ship with the program and can be read in the **About** window.

## Special thanks

Thanks to the developers and communities of the projects that make gaming on Linux possible and support the tools PeliGames uses:

- [GE-Proton — GloriousEggroll](https://github.com/GloriousEggroll/proton-ge-custom)
- [Proton CachyOS — CachyOS team](https://github.com/CachyOS/proton-cachyos)
- [UMU Launcher — Open Wine Components](https://github.com/Open-Wine-Components/umu-launcher)
- [Wine](https://www.winehq.org/) and [Proton — Valve and contributors](https://github.com/ValveSoftware/Proton)
- [DXVK](https://github.com/doitsujin/dxvk)
- [VKD3D-Proton](https://github.com/HansKristian-Work/vkd3d-proton)
- [Winetricks](https://github.com/Winetricks/winetricks)
