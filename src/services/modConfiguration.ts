import type { AppConfig } from "../types/appConfig";

/** Keep the configured-state check and installer payload on the same file types. */
export function modelSourcePaths(config: Pick<AppConfig, "dll_version"> | null) {
  const path = typeof config?.dll_version === "string" ? config.dll_version.trim() : "";
  return {
    dllPath: /\.(dll|zip)$/i.test(path) ? path : null,
    binPath: /\.bin$/i.test(path) ? path : null,
  };
}

/** Existing saved wizard configurations count; a preferences-only default does not. */
export function hasDlssnrConfiguration(config: AppConfig | null): boolean {
  if (!config || !["AMDNR", "OptiScaler"].includes(config.backend)) return false;
  const source = modelSourcePaths(config);
  return Boolean(source.binPath || source.dllPath);
}
