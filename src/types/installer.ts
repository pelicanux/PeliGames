/**
 * Types of models that can be used for installation.
 * - "bin": An existing dlssnr.bin file.
 * - "dll": An NVIDIA DLL to extract the model from.
 */
export type ModelSource = "bin" | "dll";

/**
 * Installation route choices.
 */
export type InstallRoute = "optiscaler" | "reshade" | "vulkan" | "dx9" | "remove";

/**
 * Parameters for the installer payload sent to Rust.
 */
export interface InstallerPayload {
  gameDir: string;
  route: InstallRoute;
  bitness: "32" | "64";
  gpuArch: "rdna3" | "rdna4";
  dllPath: string | null;
  binPath: string | null;
  shortcutKey?: string;
  neuralStartup?: boolean;
  /** Optional parts on every route: faster int4 mixed network, RenoDX style. */
  model?: "auto" | "small" | "experimental" | "original";
  int4?: boolean;
  renodx?: boolean;
  /** Optional ReShade motion vectors; installed by the compatible backend. */
  lumenite?: boolean;
}
