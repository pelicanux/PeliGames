import {useEffect, useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {APPIMAGE_PROMPT_KEY} from "../../components/AppImageInstallPrompt";
import {bootSplashGone} from "../../components/BootSplash";
export function useLauncherWelcome(initialGamePath?: string) {
  const [showWelcome, setShowWelcome] = useState(() => {
    try {return !initialGamePath && localStorage.getItem("peligames_welcome_completed") !== "1";} catch {return !initialGamePath;}
  });
  const [appImageTarget, setAppImageTarget] = useState("");
  useEffect(() => {
    const promptPreview = import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window && new URLSearchParams(location.search).get("appimagePrompt") === "1";
    if (("__PELI_UI_PREVIEW__" in window && !promptPreview) || initialGamePath) return;
    if (!promptPreview) {try {if (localStorage.getItem(APPIMAGE_PROMPT_KEY) === "1") return;} catch { /* Ask for this session. */ }}
    let active = true;
    void invoke<{appimage: boolean; installed: boolean; target: string}>("appimage_install_status")
      .then(async status => {if (!active || !status.appimage || status.installed) return; await bootSplashGone(); if (active) setAppImageTarget(status.target);}).catch(() => {});
    return () => {active = false;};
  }, [initialGamePath]);
  return {showWelcome, setShowWelcome, appImageTarget, setAppImageTarget};
}
