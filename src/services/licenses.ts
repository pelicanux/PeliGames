import launcherLicense from '../../LICENSE?raw';
import backendLicense from '../../licenses/DLSSNR-AMD-LICENSE.txt?raw';
import rdna3License from '../../licenses/DLSSNR-RDNA3-LICENSE.txt?raw';
import frontendNotices from '../../licenses/Frontend-Dependencies.txt?raw';
import rustNotices from '../../licenses/Rust-Dependencies.txt?raw';
import fontLicense from '../../public/fonts/jersey-10/OFL.txt?raw';
import nexusIconLicense from '../../licenses/upstream/Vortex-Nexus-Icon/LICENSE.md?raw';
import nativeNotices from '../../licenses/AppImage-Native-Notices.txt?raw';
import thirdPartyNotices from '../../licenses/DLSSNR-AMD-THIRD-PARTY.md?raw';

// Original notices are bundled verbatim so they remain available offline.
export const creditLicenses = [
  { id: 'launcher', name: 'PeliGames', label: 'MIT · Pelicano', text: launcherLicense, url: 'https://github.com/pelicanux/PeliGames/blob/main/LICENSE' },
  { id: 'backend', name: 'DLSSNR-AMD', label: 'MIT · mochizuki0323', text: backendLicense, url: 'https://github.com/mochizuki0323/DLSSNR-AMD/blob/main/LICENSE' },
  { id: 'rdna3', name: 'DLSSNR-RDNA3', label: 'MIT · mochizuki0323 · mauri870', text: rdna3License, url: 'https://github.com/mauri870/DLSSNR-RDNA3/blob/main/LICENSE' },
  { id: 'thirdParty', name: '', label: 'DLSSNR-AMD', text: thirdPartyNotices, url: 'https://github.com/mochizuki0323/DLSSNR-AMD/blob/main/THIRD_PARTY.md' },
  { id: 'frontend', name: 'React · Motion · Tauri APIs', label: 'MIT · Apache-2.0', text: frontendNotices, url: 'https://github.com/pelicanux/PeliGames/blob/main/licenses/Frontend-Dependencies.txt' },
  { id: 'rust', name: 'Rust', label: 'Licenças das dependências', text: rustNotices, url: 'https://github.com/pelicanux/PeliGames/blob/main/licenses/Rust-Dependencies.txt' },
  { id: 'font', name: 'Jersey 10', label: 'OFL-1.1 · The Soft Type Project Authors', text: fontLicense, url: 'https://github.com/scfried/soft-type-jersey/blob/main/OFL.txt' },
  { id: 'nexusIcon', name: 'Nexus Mods · SVG', label: 'Vortex · GPL-3.0', text: nexusIconLicense, url: 'https://github.com/Nexus-Mods/Vortex/blob/master/LICENSE.md' },
  { id: 'native', name: 'AppImage · Linux', label: 'Avisos dos componentes nativos', text: nativeNotices, url: 'https://github.com/pelicanux/PeliGames/blob/main/licenses/AppImage-Native-Notices.txt' },
] as const;
