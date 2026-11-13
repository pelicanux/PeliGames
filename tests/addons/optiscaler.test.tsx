import {expect, test} from "bun:test";
import {mkdtemp, mkdir, cp, copyFile, rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {renderToStaticMarkup} from "react-dom/server";
import {buildAddonUi} from "../../addons/_infra/tools/build-ui";
import type {AddonHost} from "../../addons/_infra/ui-sdk";
import {importAddonModule} from "../../src/modules/addons/moduleImport";
import {createAddonI18n} from "../../src/modules/addons/i18n";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {addonDesign} from "../../src/modules/addons/sharedDesign";
import {loadLanguage} from "../../src/i18n/engine";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {checkMessages} from "../../scripts/i18n/catalogs";
import {messages} from "../../addons/optiscaler/ui/src/messages";

test("OptiScaler catalogs stay complete in their own package", () => {
  for (const [locale, catalog] of Object.entries(messages.resources)) {
    expect(checkMessages({...messages.resources.en}, {...catalog}, locale, false, locale)).toEqual([]);
  }
});
test("the real OptiScaler UI builds outside the launcher, shares React, and loads in both languages", async () => {
  const root = await mkdtemp(join(tmpdir(), "peligames-optiscaler-ui-test-"));
  const source = join(root, "addons/optiscaler"), infrastructure = join(root, "addons/_infra");
  const scopes: ResourceScope[] = [];
  try {
    await mkdir(join(source, "core"), {recursive: true});
    await mkdir(infrastructure, {recursive: true});
    await cp("addons/optiscaler/ui", join(source, "ui"), {recursive: true});
    await cp("addons/_infra/ui-sdk", join(infrastructure, "ui-sdk"), {recursive: true});
    await copyFile("addons/optiscaler/core/proxies.json", join(source, "core/proxies.json"));
    const code = await buildAddonUi(join(source, "ui/src/index.tsx"));
    expect(code).not.toContain("react-dom");
    expect(code).not.toContain("node_modules/react/");
    expect(code).not.toContain("/home/");
    expect(code).not.toContain(root);
    const module = await importAddonModule({source: code} as Parameters<typeof importAddonModule>[0]);
    for (const [language, label] of [["en", "Download"], ["pt-BR", "Baixar"]]) {
      await loadLanguage(language);
      const scope = new ResourceScope(() => {}); scopes.push(scope);
      const css: string[] = [], requests: string[] = [];
      const host: AddonHost = {
        ...addonDesign, apiVersion: 1, addon: {id: "optiscaler", version: "0.1.0"}, language,
        i18n: createAddonI18n(language, scope), lifecycle: scope, styles: {add: value => css.push(value)},
        service: {async call<T>(method: string) {requests.push(method); return null as T}}, setBusy() {},
      };
      const instance = await module.createAddon(host);
      expect(requests).toEqual([]); // Rendering metadata/factories never starts the service.
      expect(css.some(text => text.includes(".upscaler-panel"))).toBe(true);
      const View = instance.views.preferences;
      const html = renderToStaticMarkup(<I18nProvider initialPreference={language as "en" | "pt-BR"}><View context={{target: "settings", route: "preferences"}} disabled={false}/></I18nProvider>);
      expect(html).toContain(label); expect(html).toContain("mod-option-switch--rail");
      expect(html).toContain("https://github.com/optiscaler/OptiScaler");
      const Game = instance.views.game;
      const native = renderToStaticMarkup(<Game context={{target:"game-mods",route:"game",game:{id:"fixture",directory:"/fixture",native:true}}} disabled={false}/>);
      expect(native).toContain(language === "en" ? "Windows game" : "jogo Windows");
      const unresolved=renderToStaticMarkup(<Game context={{target:"game-mods",route:"game",game:{id:"fixture",directory:"/wrong-root"}}} disabled={false}/>);
      expect(unresolved).toContain(language === "en" ? "Choose the executable folder" : "Escolher pasta do executável");
      expect(requests).toEqual([]);
      await instance.dispose(); await scope.close();
    }
  } finally {
    await Promise.all(scopes.map(scope => scope.close()));
    await rm(root, {recursive: true, force: true});
  }
});
