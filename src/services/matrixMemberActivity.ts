/** Matrix identities stay distinct, including Discord bridge accounts with matching names. */
export function recentMatrixMembers(users:Iterable<string>, messages:readonly {sender:string;ts:number}[], ignored:readonly string[]=[]) {
  const latest=new Map<string,number>();
  for(const message of messages)if(Number.isFinite(message.ts))latest.set(message.sender,Math.max(latest.get(message.sender)??0,message.ts));
  return [...new Set(users)].filter(user=>!ignored.includes(user)).sort((a,b)=>(latest.get(b)??0)-(latest.get(a)??0)||a.localeCompare(b));
}
export function visibleOwnGame(sharing:boolean,invisible:boolean,name:string):string|null {return sharing&&!invisible&&name?name:null;}
