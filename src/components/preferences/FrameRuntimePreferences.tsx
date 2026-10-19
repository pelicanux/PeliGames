import {FrameOverlaySettings} from "../../modules/frameGeneration/FrameOverlaySettings";
import {FrameGenerationPanel} from "../../modules/frameGeneration/FrameGenerationPanel";
import {experimentalFrameGeneration} from "../../services/releaseFeatures";
export function FrameRuntimePreferences({backend,disabled,onBusy}:{backend:"mako";disabled:boolean;onBusy:(busy:boolean)=>void}){
 if(!experimentalFrameGeneration)return <p role="status">Geração de quadros e overlay em desenvolvimento. Indisponíveis nesta versão.</p>;
 return <><FrameGenerationPanel key={backend} game={{name:"MAKO",path:`runtime-preferences:${backend}`,launcher:"PeliGames"}} disabled={disabled} managementBackend={backend} onBusyChange={onBusy} onSaved={()=>{}}/><FrameOverlaySettings disabled={disabled}/></>;
}
