export const nexusArchiveFormats = ["zip", "7z", "rar", "fbmod", "fbpack", "daimod", "package", "ts4script", "dll", "pak", "mem", "dl_bin", "stream", "gpu_resources"];
export function supportedNexusFile(name: string): boolean {
  const extension = name.split(".").pop()?.toLowerCase();
  return Boolean(extension && nexusArchiveFormats.includes(extension)) || /^[a-f0-9]{16}\.patch_\d+(\.(stream|gpu_resources))?$/i.test(name);
}
