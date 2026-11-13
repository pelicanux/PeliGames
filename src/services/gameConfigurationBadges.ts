import {translator} from "../i18n/engine";
import {advancedOptionValue, gamescopeEnabled, normalizeAdvanced, type AdvancedSettings} from "./advancedSettings";
import type {MangoHudSettings} from "./mangohud";
export interface ConfigurationBadge {id:string;label:string;description:string;}
/** Saved launch configuration, never a claim that an effect is running in the game. */
export function gameConfigurationBadges(raw:AdvancedSettings|undefined, profile:MangoHudSettings|undefined, language: string):ConfigurationBadge[]{
  const tx = translator(language);

 const s=normalizeAdvanced(raw),badges:ConfigurationBadge[]=[];
 const add=(id:string,label:string,description:string)=>badges.push({id,label,description});
 const g=s.gamescope!,scope=gamescopeEnabled(g);
 const gameLimit=s.mangohud_fps_limit;
 const background=s.overlay_enabled&&(s.background_fps??0)>0;
 const display=s.mangohud_enabled??Boolean(profile?.launcher_enabled||s.options.mangohud||s.options.mangohud_env);
 const injected=!scope&&(display||(gameLimit??0)>0||background);
 if(!scope&&display)add("mangohud","MangoHUD",tx("gameConfigurationBadges", "monitoring_enabled_for_this_game"));
 if(s.overlay_enabled)add("overlay","Overlay",tx("gameConfigurationBadges", "peligames_overlay_enabled_in_the_profile"));
 if(scope)add("gamescope","Gamescope",tx("gameConfigurationBadges", "gamescope_enabled_for_this_game"));
 if(scope&&g.enable_limiter&&g.fps>0)add("fps",`${g.fps} FPS`,tx("gameConfigurationBadges", "gamescope_fps_limiter_enabled"));
 else if(s.options.dxvk_limit)add("fps",`${advancedOptionValue(s,"dxvk_limit")} FPS`,tx("gameConfigurationBadges", "dxvk_fps_limiter_enabled_applies_to_games_using_dxvk"));
 else if(injected){
  const cycle=gameLimit==null?String(profile?.launcher_fps_limits||""):"";
  const limit=gameLimit??Number(profile?.fps_limit??0);
  if(cycle)add("fps",`FPS · ${cycle.split(",").map(v=>v.trim()==="0"?"∞":v.trim()).join("/")}`,tx("gameConfigurationBadges", "mangohud_fps_limit_cycle_the_selected_limit_may_change_in_game"));
  else if(limit>0)add("fps",`${limit} FPS`,tx("gameConfigurationBadges", "mangohud_fps_limiter_enabled"));
 }
 if(scope&&g.enable_limiter&&g.fps_unfocused>0)add("background",`${tx("gameConfigurationBadges", "unfocused")} · ${g.fps_unfocused} FPS`,tx("gameConfigurationBadges", "gamescope_fps_cap_while_the_game_is_unfocused"));
 else if(!scope&&background)add("background",`${tx("gameConfigurationBadges", "unfocused")} · ${s.background_fps} FPS`,tx("gameConfigurationBadges", "peligames_overlay_fps_cap_while_the_game_is_unfocused"));
 if(injected&&profile){
  const vk=String(profile.vsync??"1"),gl=Number(profile.gl_vsync??0);
  if(["0","2","3"].includes(vk))add("vsync-vulkan","VSync · Vulkan",tx("gameConfigurationBadges", "vulkan_vsync_configured_through_mangohud_depends_on_the_game_s_graphics_api"));
  if(gl>0)add("vsync-gl","VSync · OpenGL",tx("gameConfigurationBadges", "opengl_vsync_configured_through_mangohud_depends_on_the_game_s_graphics_api"));
 }
 return badges;
}
