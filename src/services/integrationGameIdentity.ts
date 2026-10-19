import type {GameInfo} from "../components/GameGrid";
const folder=(path:string|undefined)=>path?.replace(/\\/g,"/").replace(/\/+$/,"")||"";
/** Scanner/Nexus records keep a store ID; the external library namespaces it. */
function externalIdentity(game:GameInfo):string|undefined{
 if(!game.external_id)return undefined;
 const scope=`${game.launcher}:${game.flatpak?"flatpak":"native"}:`;
 const legacyStore="store" in game&&typeof game.store==="string"?game.store:"";
 const store=game.launcher==="Steam"?"":`${game.external_store||legacyStore}:`;
 let id=game.external_id;
 if(id.startsWith(scope)){
  id=id.slice(scope.length);
  if(store&&id.startsWith(store))id=id.slice(store.length);
 }
 return `${scope}${store}${id}`;
}
/** Never use titles: copies can share names/executables but use different prefixes. */
export function sameGameInstallation(a:GameInfo,b:GameInfo):boolean{
 if(a.prefix&&b.prefix&&folder(a.prefix).replace(/\/pfx$/,"")!==folder(b.prefix).replace(/\/pfx$/,""))return false;
 if(a.path.startsWith("peligames:")&&b.path.startsWith("peligames:"))return a.path===b.path;
 const leftId=externalIdentity(a),rightId=externalIdentity(b);
 if(leftId&&rightId)return leftId===rightId;
 if(a.path===b.path&&a.launcher===b.launcher)return true;
 const left=folder(a.directory||a.path),right=folder(b.directory||b.path);
 return Boolean(left&&right&&(left===right||left.startsWith(right+"/")||right.startsWith(left+"/")));
}
