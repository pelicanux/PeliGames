import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { installationFolderName } from "../services/installationPaths";

export function useInstallationLocation(confirmedName: string, open: boolean) {
  const [root, setRoot] = useState("");
  const [custom, setCustom] = useState<string | null>(null);
  const [selected, setSelected] = useState(false);
  const [error, setError] = useState(false);
  useEffect(() => {
    if (!open) return;
    let active = true;
    setError(false);
    invoke<string>("prepare_default_installation_directory").then(path => {
      if (active) setRoot(path);
    }).catch(() => { if (active) setError(true); });
    return () => { active = false; };
  }, [open]);
  const suggested = root && confirmedName ? `${root}/${installationFolderName(confirmedName)}` : root;
  const selectDefault = async () => {
    const folder = confirmedName ? installationFolderName(confirmedName) : undefined;
    try {
      const path = await invoke<string>("prepare_default_installation_directory", { folder });
      setRoot(folder ? path.slice(0, path.lastIndexOf("/")) : path);
      setCustom(null); setSelected(true); setError(false);
    } catch (error) { setError(true); throw error; }
  };
  return { directory: custom ?? suggested, selected, selectDefault,
    setDirectory: (path: string) => { setCustom(path); setSelected(true); },
    reset: () => { setCustom(null); setSelected(false); }, error: error && custom === null };
}
