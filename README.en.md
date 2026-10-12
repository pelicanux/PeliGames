<p align="center">
  <img src="public/images/readme-header.svg" alt="PeliGames" width="900" />
</p>

<p align="center"><a href="README.md">Português</a> · <strong>English</strong></p>

Windows game launcher for Linux with Proton, a game library, Pelinstall, Wine tools, Nexus Mods integration and the DLSSNR-AMD mod manager.

<p align="center">
  <img src="public/images/interface-preview.webp" alt="PeliGames interface with the game library grouped by source" width="1200" />
</p>

## How it works

1. **Library:** PeliGames brings together games from Steam, Heroic, Lutris, Bottles and Faugus, plus the ones you install with it.
2. **Installing Windows games and programs:** pick the installer (`.exe` or `.msi`), a name, a folder and a Proton version. PeliGames prepares the prefix, runs the installer and finds the game's executable by itself. When several executables are possible, it asks.
3. **Launchers (EA app, Battle.net, Ubisoft Connect, Epic):** games installed from inside a launcher appear under **Programs**, ready to open directly. When one of them is added to the library, it gets a profile of its own (separate prefix, Proton and mods) without duplicating the launcher's files.
4. **Nexus Mods:** in **Mods → Nexus Mods**, add the game, pick mods from the catalog (or import an archive) and click **Install**. PeliGames checks the requirements, runs the FOMOD installer when there is one and deploys the files following the game's rules, the same ones the Vortex game extensions use. Enabling, disabling and removing mods never changes the game's original files. See the [supported games](docs/supported-games.md).
5. **Problems:** **Report error** builds a report for the game and can send it straight to the PeliGames support forum on Discord. Launcher errors also show up in the notification bell.

## Installation

Download the AppImage, RPM or DEB from [Releases](https://github.com/pelicanux/PeliGames/releases).

- RPM and DEB install the `peligames` and `pelinstall` binaries in `/opt/PeliGames`, with commands in `/usr/bin` and shortcuts in the system application folders. From a terminal, use `peligames` or `pelinstall /path/to/program.exe`.
- The AppImage contains both modules. Make it executable and open it. The first start registers the launcher and "Open with Pelinstall" for the user. Keep the AppImage in the same place so the shortcuts keep working.
- To open a Windows executable in the installer: `./PeliGames.AppImage --install /path/to/program.exe`.

Proton, UMU and Winetricks are downloaded when needed. Settings, tools, mods and interface data live in `~/.config/PeliGames`; delete that folder to start from scratch. Prefixes and games stay in the folders the user chose. SteamGridDB covers need an optional personal key, set in Preferences.

## Nexus Mods

Connect your personal API key in **Nexus account** and pick mods in the **Catalog**, or import ZIP, 7z and RAR archives with **Local Mod**. To receive downloads from the website, let PeliGames handle Nexus links and use **Mod Manager Download**.

Free accounts confirm each download on the Nexus Mods page; Premium accounts download directly. Follow the queue in the Nexus downloads panel and, in the mod list, check the requirements before **Install**, **Enable** or **Disable**.

Independent integration, not affiliated with Nexus Mods. Report issues to PeliGames, not to Nexus Mods.

Support depends on the game and on the package layout. The API key is stored only in the system keyring (the AppImage needs `secret-tool` and an available keyring). Use **Logs** or **Report error** to diagnose problems.

Under **Settings → Tools** you can create a ZIP backup of the games with Nexus mods installed, or restore one saved on another disk. The per-game recovery option keeps a hidden `.peligames` folder on the games' disk, so the records come back after rescanning on another distribution.

## Supported games

PeliGames installs Nexus Mods mods in **202 games**, with modules based on the Vortex game extensions: Fallout, Skyrim, Starfield, Cyberpunk 2077, Baldur's Gate 3, Stardew Valley, Elden Ring, the Resident Evil and Monster Hunter families, and dozens of Unreal and Unity games.

| Completion | Games |
|---|---|
| 100% — verified in the real game | 5 |
| 90% — parity with the Vortex extension | 67 |
| 80% — some extension features still pending | 29 |
| 70% — generated from the family template | 100 |
| 60% — Vortex support not confirmed | 1 |

**[See the full list of supported games →](docs/supported-games.md)** (in Portuguese)

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
