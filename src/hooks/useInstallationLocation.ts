import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { installationFolderName } from "../services/installationPaths";

export function useInstallationLocation(confirmedName: string, open: boolean) {
  const reservations=useRef(new Set<string>());
  const pending=useRef(new Set<Promise<string>>());
  const mounted=useRef(true);
  const selectionRevision=useRef(0);
  const openRef=useRef(open);openRef.current=open;
  const release=async()=>{
    await Promise.allSettled([...pending.current]);
    const paths=[...reservations.current];reservations.current.clear();
    await Promise.all(paths.map(path=>invoke("release_default_installation_directory",{path}).catch(console.error)));
  };
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;void release();};},[]);
  useEffect(()=>{if(!open)void release();},[open]);
  const [root, setRoot] = useState("");
  const [custom, setCustom] = useState<string | null>(null);
  const [reserved, setReserved] = useState<{ folder: string; path: string } | null>(null);
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
  const folderName = confirmedName ? installationFolderName(confirmedName) : "";
  const suggested = reserved?.folder === folderName ? reserved.path : root && folderName ? `${root}/${folderName}` : root;
  const selectDefault = async () => {
    const revision=++selectionRevision.current;
    const folder = confirmedName ? installationFolderName(confirmedName) : undefined;
    try {
      let path:string;
      if(reserved?.folder === (folder || "") && reservations.current.has(reserved.path))path=reserved.path;
      else {
        const request=invoke<string>("prepare_default_installation_directory",{folder});pending.current.add(request);
        try{path=await request;if(folder)reservations.current.add(path);}finally{pending.current.delete(request);}
      }
      if(folder)reservations.current.add(path);
      if(!mounted.current||!openRef.current||revision!==selectionRevision.current){
        reservations.current.delete(path);
        await invoke("release_default_installation_directory",{path});
        throw new Error("Seleção de pasta cancelada.");
      }
      for(const previous of reservations.current){if(previous!==path){reservations.current.delete(previous);void invoke("release_default_installation_directory",{path:previous}).catch(console.error);}}
      setReserved({ folder: folder || "", path });
      setRoot(folder ? path.slice(0, path.lastIndexOf("/")) : path);
      setCustom(null); setSelected(true); setError(false);
      return path;
    } catch (error) { if(mounted.current&&revision===selectionRevision.current)setError(true); throw error; }
  };
  return { directory: custom ?? suggested, selected: selected && (custom !== null || reserved?.folder === folderName),
    hasSelection: selected,
    resolveDirectory: async () => {
      if (!selected) throw new Error("Selecione a pasta de instalação antes de continuar.");
      return custom ?? await selectDefault();
    },
    selectDefault: async () => { await selectDefault(); },
    setDirectory: (path: string) => { ++selectionRevision.current; for(const reserved of reservations.current){if(reserved!==path){reservations.current.delete(reserved);void invoke("release_default_installation_directory",{path:reserved}).catch(console.error);}} setCustom(path); setSelected(true); },
    release,
    trackReservation:(path:string)=>{reservations.current.add(path);},
    reset: () => { ++selectionRevision.current; void release(); setCustom(null); setReserved(null); setSelected(false); }, error: error && custom === null };
}
