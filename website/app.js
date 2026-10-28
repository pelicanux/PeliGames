const screens = {
  library: ["Biblioteca de jogos do PeliGames", "Seus jogos organizados, dos favoritos à próxima descoberta."],
  nexus: ["Catálogo Nexus Mods no PeliGames", "Encontre e gerencie mods em jogos compatíveis, sem sair do launcher."],
  dlssnr: ["Configuração do DLSSNR-AMD por jogo", "Escolha as opções de upscaling compatíveis com seu hardware e seu jogo."],
  monitoring: ["Configurações de monitoramento do MangoHUD", "Acompanhe o desempenho com o MangoHUD configurado do seu jeito."],
  themes: ["Personalização da aparência do PeliGames", "Sua biblioteca com suas cores, transparência e personalidade."]
};
const tabs = [...document.querySelectorAll('[role="tab"]')];
const shot = document.querySelector('#gallery-shot');
const panel = document.querySelector('#gallery-panel');
function selectTab(tab, focus = false) {
  tabs.forEach(item => { item.setAttribute('aria-selected', String(item === tab)); item.tabIndex = item === tab ? 0 : -1; });
  const [alt, description] = screens[tab.dataset.screen];
  shot.src = `assets/${tab.dataset.screen}.jpg`;
  shot.alt = alt;
  panel.setAttribute('aria-labelledby', tab.id);
  document.querySelector('#gallery-description').textContent = description;
  if (focus) tab.focus();
}
tabs.forEach((tab, index) => {
  tab.addEventListener('click', () => selectTab(tab));
  tab.addEventListener('keydown', event => {
    let target;
    if (event.key === 'ArrowRight') target = tabs[(index + 1) % tabs.length];
    if (event.key === 'ArrowLeft') target = tabs[(index - 1 + tabs.length) % tabs.length];
    if (event.key === 'Home') target = tabs[0];
    if (event.key === 'End') target = tabs.at(-1);
    if (target) { event.preventDefault(); selectTab(target, true); }
  });
});
const viewer = document.querySelector('#image-viewer');
document.querySelector('.gallery-image').addEventListener('click', () => {
  viewer.querySelector('img').src = shot.src;
  viewer.querySelector('img').alt = shot.alt;
  viewer.showModal();
  document.body.classList.add('viewer-open');
});
viewer.addEventListener('close', () => document.body.classList.remove('viewer-open'));
viewer.addEventListener('click', event => { if (event.target === viewer) viewer.close(); });
// Release metadata is generated on deployment, including public experimental releases.
// Existing download links remain usable when JavaScript or this request is unavailable.
fetch('release.json').then(response => {
  if (!response.ok) throw new Error('Release metadata unavailable');
  return response.json();
}).then(release => {
  const trusted = value => typeof value === 'string' && value.startsWith('https://github.com/pelicanux/PeliGames/releases/');
  if (!trusted(release.url) || typeof release.tag !== 'string') return;
  document.querySelectorAll('[data-release-label]').forEach(item => { item.textContent = `${release.tag}${release.prerelease ? ' experimental' : ''}`; });
  document.querySelectorAll('[data-release-link]').forEach(item => { item.href = release.url; });
  for (const item of document.querySelectorAll('[data-download]')) {
    const kind = item.dataset.download, asset = release.assets?.[kind];
    item.href = trusted(asset?.url) ? asset.url : release.url;
    const size = document.querySelector(`[data-size="${kind}"]`);
    size.textContent = Number.isFinite(asset?.size) ? `${Math.ceil(asset.size / 1048576)} MB` : 'Ver release';
  }
}).catch(() => { /* Keep the working HTML download links. */ });
