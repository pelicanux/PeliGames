import type {ResourceScope} from "./lifecycle";

/** Styles belong to a view session, including when factory creation fails. */
export function createAddonStyles(scope: ResourceScope, addonId: string, document?: Pick<Document, "createElement" | "head">) {
  let bytes = 0;
  return Object.freeze({add(css: string) {
    scope.assertOpen();
    if (typeof css !== "string" || (bytes += css.length) > 512 * 1024) throw new Error("Add-on styles exceed the size limit");
    if (!document) return; // Static rendering does not attach document styles.
    const style = document.createElement("style");
    style.dataset.addon = addonId;
    style.textContent = css;
    document.head.appendChild(style);
    scope.onDispose(() => style.remove());
  }});
}
