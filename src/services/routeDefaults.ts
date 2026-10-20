export function defaultShortcutForRoute(route?: string): string {
  return route === "reshade" || route === "vulkan" || route === "dx9" ? "Home" : "Insert";
}

/** The launch options line the backend printed ("Steam launch options:  …"),
 * which carries the Vulkan layers of NR Control and int4 mixed. */
export function launchOptionsFromInstallOutput(output: string): string | null {
  const lines = output.split("\n");
  // 0.0.7 prints a second complete line to permit switching to int4 in game
  // even when the starting network is FP8/original. Keep that layer available.
  const alternative = lines.findIndex(item => item.trim() === "To use CC_SILVER_AARDWOLD_INT4_MIXED in game, use this line instead (all three networks can be switched):");
  if (alternative >= 0) {
    const value = lines[alternative + 1]?.trim();
    if (value?.endsWith("%command%") && /(?:^|\s)VK_INSTANCE_LAYERS=/.test(value)
      && value.includes("VK_LAYER_DLSSNR_int4")) return value;
  }
  const line = lines.find(item => item.trimStart().startsWith("Steam launch options:"));
  const value = line?.slice(line.indexOf(":") + 1).trim();
  return value && value.endsWith("%command%") ? value : null;
}
export function steamLaunchOptionsForRoute(route: string): string {
  const overrides = route === "vulkan" || route === "dx9"
    ? "winevulkan=n,b;vulkan-1=n,b"
    : "dxgi=n,b";
  return `WINEDLLOVERRIDES="${overrides}" %command%`;
}

/** ReShade's DirectX/OpenGL/Vulkan routes share its menu; OptiScaler has its own. */
export function menuShortcutStorageKey(game: string, route: string): string {
  const menu = route === "optiscaler" ? "optiscaler" : "reshade";
  return `${game}::menu:${menu}`;
}
export function menuShortcutForRoute(keys: Record<string, string> | undefined, game: string, directory: string, route: string): string {
  return keys?.[menuShortcutStorageKey(game, route)]
    || keys?.[menuShortcutStorageKey(directory, route)]
    // Older builds saved one key per game. Keep it until each menu is customized.
    || keys?.[game] || keys?.[directory] || defaultShortcutForRoute(route);
}
