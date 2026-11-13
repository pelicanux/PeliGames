import {expect,test} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {SelectedGameOverview} from "../../src/modules/launcher/SelectedGameOverview";
import {GameConfigurationChips} from "../../src/components/modern/GameConfigurationChips";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {normalizeAdvanced} from "../../src/services/advancedSettings";
import type {GameInfo} from "../../src/components/GameGrid";
import type {GameAnalysisResult} from "../../src/hooks/useGameAnalysis";
await loadLanguage("pt-BR");
const game:GameInfo={name:"Example",path:"/fixture/game",directory:"/fixture/game",prefix:"/fixture/prefix",executable:"/fixture/game/bin/game.exe",launcher:"PeliGames"};
test("overview keeps core game paths while omitted modules cannot expose legacy status or destinations",()=>{
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><SelectedGameOverview
  selectedGame={game} gamePanelMode="mods" info={{platform:"Windows",graphics_api:"DirectX 12",architecture:"64 bits",upscalers:[]} as unknown as GameAnalysisResult}
  selectedGameRoot={game.directory} releaseDate="" copyReference={null}
  directoryPickerBusy={false}
  onChooseCustomPath={()=>{}} onDuplicate={()=>{}} showModInformation={false}/></I18nProvider>);
 for(const path of ["/fixture/game","/fixture/prefix","/fixture/game/bin"])expect(html).toContain(path);
 expect(html).not.toContain("hidden-proxy.dll");expect(html).not.toContain("hidden-nr-destination");expect(html).not.toContain("DLSSNR");
 expect(html).toContain("Windows");expect(html).toContain("DirectX 12");
});
test("the integrated configuration chips omit module indicators in external mode but retain core monitoring",()=>{
 const advanced={...normalizeAdvanced(),afmf:{enabled:true},mangohud_enabled:true,overlay_enabled:true};
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><GameConfigurationChips gameKey={game.path} advanced={advanced}/></I18nProvider>);
 expect(html).toContain("MangoHUD");expect(html).toContain("Overlay");expect(html).not.toContain("AFMF");
});
