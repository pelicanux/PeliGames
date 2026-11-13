# GPU Screen Recorder add-on

Adds gameplay recording, instant replay and a video library to PeliGames. Its
service owns recorder processes, portal selection, audio, presets, downloads and
per-game settings. The launcher provides game identity, the in-game overlay,
automatic replay on Play and recording status for Discord.

Import the add-on ZIP from the store. Under **Manage**, download the portable
recorder and choose the video destination. No root installation is required.
Choose video and audio settings in the game's **Captures** tab, then start a
recording or replay. **Save replay** writes the buffered seconds to a video.
The desktop portal can ask which screen or window to capture.

Settings are saved automatically. Video, audio and replay-duration changes apply
to the next capture. Automatic replay starts only after clicking **Play** for a
game with that option enabled. The game's overlay must be enabled to use the
capture shortcuts inside the game. Reopen the game after changing bindings.

Closing a panel keeps recording. Disabling or removing the add-on stops its
service and finalizes the current recording. Downloaded runtime removal preserves
videos and per-game preferences. Existing component and profile paths are
retained, so the integration can use previously saved preferences.

The ZIP contains a compiled service, bundled interface, English and Portuguese
translations, manifest and licenses. The third-party recorder is downloaded
separately and its archive is checked before installation. Capture requires
compatible graphics hardware and drivers; portal capture also requires a working
PipeWire/desktop portal session.

Project: [GPU Screen Recorder](https://git.dec05eba.com/gpu-screen-recorder/).
Portable package: [pkgforge-dev](https://github.com/pkgforge-dev/gpu-screen-recorder-AppImage).
