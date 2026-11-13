import {OptiScalerConnection} from "./host";
import {test,expect} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {OptiScalerSoftwareStatus} from "../../addons/optiscaler/ui/src/OptiScalerSoftwareStatus";
import {createUpscalerPreview} from "./preview";

test("component inventory labels files and integration without inventing FSR versions or claiming runtime activity",async()=>{
 await loadLanguage("pt-BR");
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><OptiScalerSoftwareStatus scope="game" items={[{id:"optiscaler",present:true,integrated:false,version:"v0.9.4"},{id:"fsr",present:true,integrated:false,version:null},{id:"fakenvapi",present:false,integrated:true,version:null},{id:"dlss",present:false,integrated:false,version:null}]}/></OptiScalerConnection></I18nProvider>);
 expect(html).toContain("Componentes neste jogo");expect(html).toContain("v0.9.4");expect(html).toContain("Arquivos detectados");expect(html).toContain("Não detectado");expect(html).toContain("Integrado ao OptiScaler");expect(html).not.toContain("4.1.2");
});
test("preview keeps installed versions distinct from downloaded channels until explicitly updating the game",()=>{
 const preview=createUpscalerPreview(()=>false);
 preview.install("/preview/game","dxgi.dll",{});
 const installedVersion=()=>preview.state("/preview/game").software?.find(item=>item.id==="optiscaler")?.version;
 expect(installedVersion()).toBe("v0.9.4");
 expect(preview.state("/preview/game").settings["FSR.Fsr4ForceEnableInt8"]).toBe("auto");
 preview.setChannel("nightly");preview.download();
 expect(preview.runtimeState().version).toBe("10.0.0-pre1");expect(installedVersion()).toBe("v0.9.4");
 preview.upgrade("/preview/game");expect(installedVersion()).toBe("10.0.0-pre1");
 expect(preview.state("/preview/game").settings["FSR.Fsr4ForceEnableInt8"]).toBeUndefined();
 expect(preview.state("/preview/game").settings["FSR.Fsr4GpuSupport"]).toBe("auto");
});

test("removing a downloaded channel preserves installed games and the other channel",()=>{
 const preview=createUpscalerPreview(()=>false);
 preview.install("/preview/game","dxgi.dll",{});
 preview.setChannel("nightly");preview.download();
 expect(()=>preview.removeDownload("stable")).toThrow("channel changed");
 preview.removeDownload("nightly");
 expect(preview.runtimeState().ready).toBe(false);
 expect(preview.runtimeState().directory).toBeNull();
 expect(preview.state("/preview/game").installed).toBe(true);
 expect(preview.state("/preview/game").software?.[0].version).toBe("v0.9.4");
 preview.setChannel("stable");expect(preview.runtimeState().ready).toBe(true);
 expect(preview.runtimeState().directory).toContain("official");
});
