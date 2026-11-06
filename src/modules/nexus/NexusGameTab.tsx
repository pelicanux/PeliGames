import { sameGameInstallation } from "../../services/integrationGameIdentity";
import {useEffect,useRef,useState,type ReactNode} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "../../components/GameGrid";
import type {useNexusWorkspace} from "./useNexusWorkspace";
import {useI18n} from "../../i18n/I18nContext";
export function NexusGameTab({game,workspace,children}:{game:GameInfo;workspace:ReturnType<typeof useNexusWorkspace>;children:ReactNode}){
  const {t: tx} = useI18n();

 const [status,setStatus]=useState<"checking"|"unsupported"|"ready"|"error">("checking");const [error,setError]=useState("");const task=useRef<{key:string;promise:Promise<boolean>} | undefined>(undefined);
 const registered=workspace.workspace.games.some(g=>g.id===game.nexus_id||sameGameInstallation(g.game,game));
 const key=JSON.stringify([game.launcher,game.path,game.directory,game.prefix,game.external_id,game.nexus_id,registered]);
 useEffect(()=>{let alive=true;if(!workspace.loaded)return;
  setStatus("checking");setError("");
  if(task.current?.key!==key)task.current={key,promise:(async()=>{const report=await invoke<{domain?:string;platform?:string;vortex_supported?:boolean}>("inspect_nexus_game",{source:game.directory||game.path});if(!report.domain||report.vortex_supported===false)return false;if(registered)return true;if(!await workspace.add(game,report.platform==="native"?"native":"proton"))throw new Error("Não foi possível preparar o gerenciamento desta instalação. Confira a notificação e tente novamente.");return true})()};
  void task.current.promise.then(ok=>{if(alive)setStatus(ok?"ready":"unsupported")}).catch(e=>{if(alive){setError(String(e));setStatus("error")}});return()=>{alive=false};
 },[key,registered,workspace.loaded]);
 if(status==="ready"&&registered)return <>{children}</>;
 return <div className="modern-tile"><h3>{tx("ui-nexus-game-tab", "nexus_mods")}</h3><p role="status">{status==="checking"?(tx("ui-nexus-game-tab", "identifying_this_installation_s_module")):status==="error"?error:(tx("ui-nexus-game-tab", "this_game_does_not_yet_have_a_compatible_mod_module_in_peligames_this_tab_remains_availa"))}</p></div>
}
