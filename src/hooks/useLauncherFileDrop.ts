import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { notifyInfo, notifyError } from "../services/notifications";

export function useLauncherFileDrop(blocked: boolean, onDrop: (path: string) => void) {
  const [hovering, setHovering] = useState(false);
  const current = useRef({blocked, onDrop}); current.current = {blocked, onDrop};
  const demoStarted=useRef(false);
  useEffect(()=>{
    if (!blocked && !demoStarted.current && "__PELI_UI_PREVIEW__" in window && new URLSearchParams(location.search).get("drop")==="folder") {
      demoStarted.current=true; onDrop("/preview/Jogo instalado");
    }
  },[blocked,onDrop]);
  useEffect(() => {
    let active = true; let stop: (() => void) | undefined;
    const receive = (paths: string[]) => {
      setHovering(false);
      if (current.current.blocked) { notifyInfo("Arrastar arquivos", "Conclua ou feche a janela atual antes de importar outro arquivo."); return; }
      if (paths.length !== 1) { notifyInfo("Arrastar arquivos", "Arraste um arquivo ou uma pasta por vez."); return; }
      current.current.onDrop(paths[0]);
    };
    if (!("__PELI_UI_PREVIEW__" in window)) {
      getCurrentWebview().onDragDropEvent(event => {
        if (!active) return;
        if (event.payload.type === "enter" || event.payload.type === "over") setHovering(!current.current.blocked);
        else if (event.payload.type === "drop") receive(event.payload.paths);
        else setHovering(false);
      }).then(unlisten => {if(active)stop=unlisten;else unlisten();}).catch(error => notifyError("Arrastar arquivos", error));
    }
    const over = (event: DragEvent) => { if (event.dataTransfer?.types.includes("Files")) {event.preventDefault(); if(event.dataTransfer)event.dataTransfer.dropEffect="copy";} };
    const drop = (event: DragEvent) => {
      if (!event.dataTransfer?.types.includes("Files")) return;
      event.preventDefault(); setHovering(false);
      if ("__PELI_UI_PREVIEW__" in window) notifyInfo("Arrastar arquivos", "Para arrastar arquivos reais do computador, use o aplicativo PeliGames. Esta é uma prévia no navegador.");
    };
    window.addEventListener("dragover",over);window.addEventListener("drop",drop);
    return () => {active=false;stop?.();window.removeEventListener("dragover",over);window.removeEventListener("drop",drop);};
  }, []);
  return hovering;
}
