import type { AppConfig } from "../components/SetupWizard";

/** Existing saved wizard configurations also count; a preferences-only default does not. */
export function hasDlssnrConfiguration(config: AppConfig | null): boolean {
  return Boolean(config && ["AMDNR", "OptiScaler"].includes(config.backend)
    && /\.(bin|dll)$/i.test(config.dll_version.trim()));
}
