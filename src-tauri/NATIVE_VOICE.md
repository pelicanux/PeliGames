# Native voice integration


The local probe now has a separate Rust microphone path (`matrix_voice.rs`).
It uses pinned LiveKit 0.9.5, CPAL 0.15.3 and nnnoiseless 0.5.2 (BSD-3-Clause,
Rust implementation derived from RNNoise). The latter includes its model; it
is not Krisp and does not require a hosted denoising service.

The explicit 48 kHz mono / 10 ms pipeline supports `off`, `standard` (WebRTC
APM noise suppression), and `rnnoise`, exclusively. Source-side processing is
disabled to avoid double filtering. AEC and AGC are deliberately not enabled:
AEC requires a correctly timed playback reference, which this probe does not
have. Samples feed a native LiveKit audio track but are not published.

The probe opens only the default microphone after its start button is clicked,
reports RMS before/after processing at 10 Hz, and stops after 30 seconds or on
window destruction. Capture callbacks use a bounded five-frame queue and drop
frames rather than blocking. No recording, credentials, or audio payloads are
sent to the frontend. Devices without 48 kHz support receive an explicit error;
resampling and device selection remain future work.

Build dependency: ALSA development headers (`libasound2-dev`) in the Linux build
environment. CPAL in this version uses ALSA; the desktop may route its default
ALSA device through PipeWire/PulseAudio. Do not describe this as direct native
PipeWire support or universal Linux compatibility.

Still pending: MatrixRTC authorization/membership/key management, production room
integration, multi-participant audio mixing and AEC reference, device UI,
persisted production voice preferences, and screen capture. The browser probe
cannot run Rust; use the compiled application with `--matrix-rtc-test`.

LiveKit 0.9.5 / webrtc-sys 0.3.49 additionally requires Clang >= 21 for its
bundled hermetic libc++ ABI. Build with `CXX=clang++-21` in the prepared Ubuntu
container. The default Jammy Clang 14 is insufficient. This is a build-time
requirement, not an end-user compiler dependency.

## References

- LiveKit SDK (Apache-2.0): https://github.com/livekit/rust-sdks
- nnnoiseless (BSD-3-Clause): https://github.com/jneem/nnnoiseless
- CPAL (Apache-2.0): https://github.com/RustAudio/cpal

## Two-session local call

Run `python3 scripts/test-native-call.py` after building. The script verifies the
pinned official LiveKit server archive, starts it on loopback ports 17880 (HTTP),
17881 (RTC TCP), 17882 (RTC UDP), and opens two standalone windows. Credentials
are randomly generated, valid for one hour, stored in a private temporary
directory, and removed when both windows close. No external Matrix room is used.

Click Entrar na chamada in each window. Both start muted. Use headphones, enable
only A's microphone and listen through B; then mute A and enable B. The noise
mode is selected before joining; leave and rejoin to change it. Volume controls
received audio. This test has no acoustic echo cancellation. Each process uses
the system's default input/output at 48 kHz; it supports one remote microphone.

An ignored Rust integration test publishes and receives non-silent synthetic
Opus audio in both directions, without opening microphone/speaker devices.
Execute it with `python3 scripts/test-native-call.py --check PATH_TO_RUST_TEST_BINARY`.
The script shuts down the server and removes temporary credentials on exit.

## Matrix room integration: readiness stage

`matrix_chat_voice_readiness` validates the invoking main window, current Matrix
account and actual joined membership for the selected room, checks encryption,
and reads public discovery/version capabilities. It never issues LiveKit
credentials, publishes RTC membership or opens a microphone. The panel explicitly
labels this as preparation; `calling_enabled` remains false.

On 2026-10-14 the configured homeserver advertised legacy LiveKit discovery but
not org.matrix.msc4143 or io.element.msc4502. Absence of these flags does not
prove calls are impossible: it means legacy compatibility must be handled.
The standalone authorization API does not universally enforce Matrix membership;
a client-side joined check is not a server-side security boundary. Before enabling
production calls, implement MatrixRTC membership/key distribution and compatible
media encryption, or deploy authorization that enforces room membership. Never
reuse the local test's unencrypted room model for private/premium Matrix calls.

Reference: https://github.com/element-hq/lk-jwt-service#-room-membership-on-the-standalone-endpoints
