import { paginateLicenseNotices } from './licensePages';

import launcherLicense from '../../LICENSE?raw';
import fontLicense from '../../public/fonts/jersey-10/OFL.txt?raw';
import nexusIconLicense from '../../licenses/upstream/Vortex-Nexus-Icon/LICENSE.md?raw';
import overlayNotices from '../../licenses/PeliGames-Overlay-NOTICES.md?raw';
import webrtcNotices from '../../licenses/LiveKit-WebRTC-Third-Party.md?raw';
import muslNotice from '../../licenses/Musl-COPYRIGHT.txt?raw';
import nativeNotices from '../../licenses/AppImage-Native-Notices.txt?raw';

// Dependency notices are generated while packaging (scripts/license-notices.sh).
const generated = import.meta.glob<string>('../../licenses/generated/*.txt', { query: '?raw', import: 'default', eager: true });
const dependencyNotices = (file: string) => generated[`../../licenses/generated/${file}`]
  ?? `Gerado ao empacotar o PeliGames: licenses/generated/${file}`;

// Original notices of everything shipped with the launcher, bundled verbatim so they remain available offline.
const notices = [
  { name: 'PeliGames', label: 'GPL-3.0-only · © 2026 Pelicano', text: launcherLicense },
  { name: 'React · Motion · Tauri APIs', label: 'Interface', text: dependencyNotices('Frontend-Dependencies.txt') },
  { name: 'Rust', label: 'Bibliotecas do launcher, PeliRun e overlay', text: dependencyNotices('Rust-Dependencies.txt') },
  { name: 'Overlay', label: 'Atribuições do código adaptado', text: overlayNotices },
  { name: 'WebRTC · LiveKit', label: 'Transmissão de tela', text: webrtcNotices },
  { name: 'musl libc', label: 'MIT · auxiliar PeliRun', text: muslNotice },
  { name: 'Jersey 10', label: 'OFL-1.1 · The Soft Type Project Authors', text: fontLicense },
  { name: 'Nexus Mods · SVG', label: 'Vortex · GPL-3.0', text: nexusIconLicense },
  { name: 'AppImage · Linux', label: 'Avisos dos componentes nativos', text: nativeNotices },
];

// One document keeps the notices accessible without listing technical components in About.
const bundledLicenseNotices = notices
  .map(item => `${item.name}\n${item.label}\n\n${item.text}`)
  .join("\n\n────────────────────────────────────────\n\n");

export const licensePages = paginateLicenseNotices(bundledLicenseNotices);
