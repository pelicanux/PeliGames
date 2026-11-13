import {expect, test} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {getUpscalerFamilies, isKnownGameInfo} from "../../src/modules/launcher/gameInfoPresentation";
import {SelectedGameOverview} from "../../src/modules/launcher/SelectedGameOverview";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
await loadLanguage("pt-BR");
test("discovery placeholders never masquerade as detected capabilities", () => {
  for (const value of [undefined, "", "Verificando...", "Não detectada", "Unknown", "Not identified", "—", "N/A", "Nenhum"]) expect(isKnownGameInfo(value)).toBe(false);
  for (const value of ["Windows", "Linux Nativo", "DirectX 12", "/games/example", "64-bits"]) expect(isKnownGameInfo(value)).toBe(true);
});
test("upscaler families use the same normalization in recommendations and overview", () => {
  expect(getUpscalerFamilies(["DLSS 3.7", "DLSS", "FSR_3", "fsr 4", "XeSS 2", "Custom"])).toEqual(["DLSS", "FSR", "XeSS", "Custom"]);
});
function render(reserved = false) {
  return renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><SelectedGameOverview
    selectedGame={{name: "Example", launcher: "PeliGames", path: "/games/example", prefix: "/prefix"}} gamePanelMode="installed"
    info={{platform: "Windows", architecture: "64-bits", graphics_api: "DirectX 12", upscalers: ["DLSS 3", "DLSS"]}}
    selectedGameRoot="/games/example" releaseDate="Desconhecido"
    copyReference={reserved ? {reserved: true, dependents: ["Example (1)"], base: null, kind: null} : null}
    directoryPickerBusy={false} onChooseCustomPath={() => {}} onDuplicate={() => {}}/>
  </I18nProvider>);
}
test("overview preserves capability, path and reserved-base controls without displaying unknown data", () => {
  const normal = render(); expect(normal).toContain("DirectX 12"); expect(normal).toContain("/games/example");
  expect(normal).not.toContain("Desconhecido"); expect(normal).not.toContain("Criar outra cópia");
  const reserved = render(true); expect(reserved).toContain("Example (1)"); expect(reserved).toContain("Criar outra cópia");
});
