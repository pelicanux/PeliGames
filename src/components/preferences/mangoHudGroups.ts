import type { MangoHudSettings } from "../../services/mangohud";
import {HUD_BLOCKS,hudOrder,placeHudBlock} from "./mangoHudLayout";
type Item = [string,string,string?];
export const MANGO_GROUPS: {id:string;title:string;color:string;blocks:string[];items:Item[]}[] = [
 {id:"performance",title:"Desempenho",color:"#34c77b",blocks:["fps","frame_timing","frame_count","fps_metrics"],items:[["fps","FPS"],["frametime","Tempo de quadro"],["frame_timing","Gráfico de quadros"],["fps_metrics","Média e 1% Low","avg,0.01"]]},
 {id:"gpu",title:"Placa de vídeo",color:"#55b9f3",blocks:["gpu","gpu_name","vulkan_driver","vram"],items:[["gpu_stats","Uso"],["gpu_temp","Temperatura"],["vram","Memória (VRAM)"],["gpu_core_clock","Frequência"],["gpu_power","Consumo"],["gpu_fan","Ventoinha"]]},
 {id:"cpu",title:"Processador",color:"#b793ef",blocks:["cpu","core_load"],items:[["cpu_stats","Uso"],["cpu_temp","Temperatura"],["cpu_mhz","Frequência"],["core_load","Por núcleo"],["cpu_power","Consumo"]]},
 {id:"memory",title:"Memória",color:"#dca0d5",blocks:["ram","io"],items:[["ram","RAM"],["swap","Swap"]]},
 {id:"system",title:"Sistema",color:"#f4b350",blocks:["resolution","display_server","time","arch","wine","engine_version","winesync","version","gamemode","vkbasalt","battery","media_player","network","custom_text_center"],items:[["resolution","Resolução"],["time","Relógio"],["wine","Versão do Wine/Proton"],["gamemode","GameMode"],["battery","Bateria"]]},
];
export function orderedMangoGroups(settings:MangoHudSettings){
 const order=hudOrder(settings);
 return [...MANGO_GROUPS].sort((a,b)=>Math.min(...a.blocks.map(id=>order.indexOf(id)))-Math.min(...b.blocks.map(id=>order.indexOf(id))));
}
export function placeMangoGroup(settings:MangoHudSettings,from:string,to:string):string {
 const groups=orderedMangoGroups(settings),i=groups.findIndex(g=>g.id===from),j=groups.findIndex(g=>g.id===to);
 if(i<0||j<0||i===j)return hudOrder(settings).join(",");
 const [moved]=groups.splice(i,1);groups.splice(j,0,moved);
 const order=hudOrder(settings);
 return groups.flatMap(group=>order.filter(id=>group.blocks.includes(id))).join(",");
}
const blockFor=(key:string)=>HUD_BLOCKS.find(b=>b.keys.includes(key))?.id;
// Secondary sensors share a fixed native row; only its anchor is movable.
export const movableMangoItem=(key:string)=>["fps","frame_timing","fps_metrics","gpu_stats","vram","cpu_stats","core_load","ram","resolution","time","wine","gamemode","battery"].includes(key);
export function orderedMangoItems(settings:MangoHudSettings,items:Item[]):Item[]{
 const order=hudOrder(settings);
 return [...items].sort((a,b)=>order.indexOf(blockFor(a[0])||"")-order.indexOf(blockFor(b[0])||""));
}
export function placeMangoItem(settings:MangoHudSettings,from:string,to:string):string {
 const a=blockFor(from),b=blockFor(to);
 return a&&b ? placeHudBlock(settings,a,b) : hudOrder(settings).join(",");
}
