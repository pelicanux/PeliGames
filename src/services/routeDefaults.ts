export function defaultShortcutForRoute(route?: string): string {
  return route === "reshade" || route === "vulkan" || route === "dx9" ? "Home" : "Insert";
}

/** The launch options line the backend printed ("Steam launch options:  …"),
 * which carries the Vulkan layers of NR Control and int4 mixed. */
export function launchOptionsFromInstallOutput(output: string): string | null {
  const line = output.split("\n").find(item => item.trimStart().startsWith("Steam launch options:"));
  const value = line?.slice(line.indexOf(":") + 1).trim();
  return value && value.endsWith("%command%") ? value : null;
}
export function steamLaunchOptionsForRoute(route: string): string {
  const overrides = route === "vulkan" || route === "dx9"
    ? "winevulkan=n,b;vulkan-1=n,b"
    : "dxgi=n,b";
  return `WINEDLLOVERRIDES="${overrides}" %command%`;
}
