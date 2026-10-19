import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { I18nProvider } from "../src/i18n/I18nContext";
import { ModernDlssPanel } from "../src/components/modern/ModernDlssPanel";
import type { ComponentProps } from "react";

const noop = () => {};
function render(overrides: Partial<ComponentProps<typeof ModernDlssPanel>> = {}) {
  return renderToStaticMarkup(<I18nProvider><ModernDlssPanel
    route="reshade" onRoute={noop} bitness="64" onBitness={noop} gpuArch="rdna4"
    busy={false} startup={true} startupDisabled={false} onStartup={noop} renodx={false} onRenodx={noop}
    model="small" modelAvailable={true} unifiedNetworks={true} modelStatus="ready" onModel={noop}
    int4={false} onInt4={noop} lumeniteAvailable={true} lumenite={false} onLumenite={noop}
    actions={null} shortcut={null} feedback={null} {...overrides}
  /></I18nProvider>);
}

test("motion vector selection is limited to supported ReShade routes", () => {
  for (const route of ["reshade", "vulkan", "dx9"] as const) {
    expect(render({route})).toContain('aria-label="Fonte dos vetores de movimento"');
  }
  for (const route of ["optiscaler", "remove"] as const) expect(render({route})).not.toContain("LumeniteFX");
  expect(render({lumeniteAvailable:false})).not.toContain("LumeniteFX");
});

test("pills expose the selected provider and cannot change during an operation", () => {
  expect(render()).toMatch(/aria-pressed="true"[^>]*>VORT<\/button>/);
  expect(render({lumenite:true})).toMatch(/aria-pressed="true"[^>]*>LumeniteFX<\/button>/);
  expect(render({busy:true})).toMatch(/disabled=""[^>]*>LumeniteFX<\/button>/);
});
