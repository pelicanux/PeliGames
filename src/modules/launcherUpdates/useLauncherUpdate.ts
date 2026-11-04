import { useEffect, useRef, useState } from "react";
import { checkLauncherUpdate, dailyLauncherUpdate, type LauncherDownloadProgress, type LauncherUpdateInfo, type LauncherUpdateStatus } from "../../services/launcherUpdates";

export function useLauncherUpdate() {
  const [showAppUpdate, setShowAppUpdate] = useState(false);
  const [launcherDownloadProgress, setLauncherDownloadProgress] = useState<LauncherDownloadProgress | null>(null);
  const [launcherUpdateInfo, setLauncherUpdateInfo] = useState<LauncherUpdateInfo | null>(null);
  const [launcherUpdateStatus, setLauncherUpdateStatus] = useState<LauncherUpdateStatus>("idle");
  const manualUpdateCheck = useRef(false);
  const mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const checkUpdatesManually = async () => {
    if (manualUpdateCheck.current || launcherUpdateStatus === "checking") return;
    manualUpdateCheck.current = true;
    setLauncherUpdateStatus("checking");
    try {
      const info = await checkLauncherUpdate();
      if (mounted.current) {
        setLauncherUpdateInfo(info);
        setLauncherUpdateStatus(info.available ? "available" : "current");
      }
    } catch {
      if (mounted.current) setLauncherUpdateStatus("error");
    } finally {
      manualUpdateCheck.current = false;
    }
  };
  // The update channel changed in Settings → Tools: offer what it now allows.
  const recheckUpdates = useRef(checkUpdatesManually); recheckUpdates.current = checkUpdatesManually;
  useEffect(() => {
    const recheck = () => void recheckUpdates.current();
    window.addEventListener("peligamesUpdateChannelChanged", recheck);
    return () => window.removeEventListener("peligamesUpdateChannelChanged", recheck);
  }, []);
  // At most one automatic GitHub request a day; reopening reuses the last answer.
  // The title bar, the update dialog and Settings still check on request.
  useEffect(() => {
    let active = true;
    dailyLauncherUpdate().then(info => {
      if (!active) return;
      setLauncherUpdateInfo(info);
      setLauncherUpdateStatus(info.available ? "available" : "current");
    }).catch(() => { if (active) setLauncherUpdateStatus("error"); });
    return () => { active = false; };
  }, []);
  return {
    open: showAppUpdate, setOpen: setShowAppUpdate,
    info: launcherUpdateInfo, status: launcherUpdateStatus,
    downloadProgress: launcherDownloadProgress, setDownloadProgress: setLauncherDownloadProgress,
    check: checkUpdatesManually,
  };
}
