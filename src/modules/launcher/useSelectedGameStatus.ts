import {useEffect, useRef, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "../../components/GameGrid";

interface InstallationDetails {status: string; installed_dll: string | null; dlssnr_installed: boolean}

/** Read-only discovery for the selected installation; replies from previous games are ignored. */
export function useSelectedGameStatus(selectedGame: GameInfo | null, selectedGameDirectory: string | undefined, gameInfoRevision: number) {
  const installationRequest = useRef(0);
  const [releaseDate, setReleaseDate] = useState("Desconhecido");
  const [installStatus, setInstallStatus] = useState("Verificando...");
  const [installedDll, setInstalledDll] = useState<string | null>(null);
  const [dlssNrInstalled, setDlssNrInstalled] = useState(false);
  const refreshInstallationDetails = async (path: string) => {
    const request = ++installationRequest.current;
    try {
      const result = await invoke<InstallationDetails>("get_game_installation_details", { gameDir: path });
      if (request === installationRequest.current) { setInstallStatus(result.status); setInstalledDll(result.installed_dll); setDlssNrInstalled(result.dlssnr_installed); }
    } catch (error) {
      console.error("Could not refresh installation details:", error);
      if (request === installationRequest.current) { setInstallStatus("Erro"); setInstalledDll(null); setDlssNrInstalled(false); }
    }
  };

  useEffect(() => {
    let active = true;
    const request = ++installationRequest.current;
    if (selectedGame) {
      if (selectedGame.app_id) {
        setReleaseDate("Detectando...");
        invoke<string>("fetch_steam_release_date", { appId: selectedGame.app_id })
          .then((date: string) => { if (active) setReleaseDate(date); })
          .catch(() => { if (active) setReleaseDate("Desconhecido"); });
      } else {
        setReleaseDate("Desconhecido");
      }

      setInstallStatus("Nenhum");
      setInstalledDll(null); setDlssNrInstalled(false);
      setInstallStatus("Verificando...");
      invoke<InstallationDetails>("get_game_installation_details", { gameDir: selectedGameDirectory })
        .then(result => {
          if (active && request === installationRequest.current) { setInstallStatus(result.status); setInstalledDll(result.installed_dll); setDlssNrInstalled(result.dlssnr_installed); }
        })
        .catch(() => { if (active && request === installationRequest.current) { setInstallStatus("Erro"); setInstalledDll(null); setDlssNrInstalled(false); } });

    } else {
      setReleaseDate("Desconhecido");
      setInstallStatus("Nenhum");
      setInstalledDll(null); setDlssNrInstalled(false);
    }
    return () => { active = false; };
  }, [selectedGame?.path, selectedGameDirectory, selectedGame?.app_id, gameInfoRevision]);

  const prepareSelection = () => {
    setInstallStatus("Verificando..."); setInstalledDll(null); setDlssNrInstalled(false);
    ++installationRequest.current;
  };
  return {releaseDate, installStatus, installedDll, dlssNrInstalled, refreshInstallationDetails, prepareSelection};
}
