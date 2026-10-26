import {advancedOptionValue, gamescopeEnabled, normalizeAdvanced, type AdvancedSettings} from "./advancedSettings";
import type {MangoHudSettings} from "./mangohud";
export interface MakoSummary {backend:string;enabled:boolean;multiplier:number;adaptive:boolean;live_enabled?:boolean;base_fps_cap?:number;scaling_enabled?:boolean;}
export interface ConfigurationBadge {id:string;label:string;description:string;}
/** Saved launch configuration, never a claim that an effect is running in the game. */
export function gameConfigurationBadges(raw:AdvancedSettings|undefined, profile:MangoHudSettings|undefined, mako:MakoSummary|undefined, pt:boolean):ConfigurationBadge[]{
 const s=normalizeAdvanced(raw),badges:ConfigurationBadge[]=[];
 const add=(id:string,label:string,description:string)=>badges.push({id,label,description});
 const g=s.gamescope!,scope=gamescopeEnabled(g);
 if(mako?.backend==="mako"&&mako.enabled){
  if(mako.live_enabled!==false)add("mako",mako.adaptive?(pt?"MAKO · adaptativo":"MAKO · adaptive"):`MAKO · ${mako.multiplier}×`,pt?"Geração de quadros MAKO habilitada no perfil do jogo.":"MAKO frame generation enabled in the game profile.");
  if(mako.scaling_enabled)add("mako-scaling",pt?"MAKO · escala":"MAKO · scaling",pt?"Escalonamento de imagem do MAKO habilitado.":"MAKO image scaling enabled.");
  if((mako.base_fps_cap??0)>0)add("mako-cap",`MAKO · ${mako.base_fps_cap} FPS`,pt?"Limite de FPS base do MAKO; não é o FPS final após a geração de quadros.":"MAKO base FPS cap, before frame generation.");
 }
 if(s.afmf?.enabled)add("afmf","AFMF",pt?"Geração de quadros AFMF habilitada para a próxima abertura do jogo.":"AFMF frame generation enabled for the next game launch.");
 const gameLimit=s.mangohud_fps_limit;
 const background=s.overlay_enabled&&(s.background_fps??0)>0;
 const display=s.mangohud_enabled??Boolean(profile?.launcher_enabled||s.options.mangohud||s.options.mangohud_env);
 const injected=!scope&&(display||(gameLimit??0)>0||background);
 if(!scope&&display)add("mangohud","MangoHUD",pt?"Monitoramento habilitado para este jogo.":"Monitoring enabled for this game.");
 if(s.overlay_enabled)add("overlay","Overlay",pt?"Overlay PeliGames habilitado no perfil.":"PeliGames overlay enabled in the profile.");
 if(scope)add("gamescope","Gamescope",pt?"Gamescope habilitado para este jogo.":"Gamescope enabled for this game.");
 if(scope&&g.enable_limiter&&g.fps>0)add("fps",`${g.fps} FPS`,pt?"Limitador do Gamescope habilitado.":"Gamescope FPS limiter enabled.");
 else if(s.options.dxvk_limit)add("fps",`${advancedOptionValue(s,"dxvk_limit")} FPS`,pt?"Limitador DXVK habilitado; aplica-se aos jogos que usam DXVK.":"DXVK FPS limiter enabled; applies to games using DXVK.");
 else if(injected){
  const cycle=gameLimit==null?String(profile?.launcher_fps_limits||""):"";
  const limit=gameLimit??Number(profile?.fps_limit??0);
  if(cycle)add("fps",`FPS · ${cycle.split(",").map(v=>v.trim()==="0"?"∞":v.trim()).join("/")}`,pt?"Lista de limites do MangoHUD. O limite selecionado durante o jogo pode variar.":"MangoHUD FPS limit cycle. The selected limit may change in game.");
  else if(limit>0)add("fps",`${limit} FPS`,pt?"Limitador de FPS do MangoHUD habilitado.":"MangoHUD FPS limiter enabled.");
 }
 if(scope&&g.enable_limiter&&g.fps_unfocused>0)add("background",`${pt?"Sem foco":"Unfocused"} · ${g.fps_unfocused} FPS`,pt?"Limite do Gamescope quando o jogo perde o foco.":"Gamescope FPS cap while the game is unfocused.");
 else if(!scope&&background)add("background",`${pt?"Sem foco":"Unfocused"} · ${s.background_fps} FPS`,pt?"Limitador de FPS sem foco do Overlay PeliGames habilitado.":"PeliGames overlay FPS cap while the game is unfocused.");
 if(injected&&profile){
  const vk=String(profile.vsync??"1"),gl=Number(profile.gl_vsync??0);
  if(["0","2","3"].includes(vk))add("vsync-vulkan","VSync · Vulkan",pt?"VSync configurado pelo MangoHUD para Vulkan; depende da API usada pelo jogo.":"Vulkan VSync configured through MangoHUD; depends on the game's graphics API.");
  if(gl>0)add("vsync-gl","VSync · OpenGL",pt?"VSync configurado pelo MangoHUD para OpenGL; depende da API usada pelo jogo.":"OpenGL VSync configured through MangoHUD; depends on the game's graphics API.");
 }
 return badges;
}
