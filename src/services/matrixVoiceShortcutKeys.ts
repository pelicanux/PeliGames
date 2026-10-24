export type VoiceShortcutAction="microphone"|"deafen";
export type VoiceShortcuts=Record<VoiceShortcutAction,string>;
export const emptyVoiceShortcuts:VoiceShortcuts={microphone:"",deafen:""};
export function voiceShortcut(event:Pick<KeyboardEvent,"code"|"ctrlKey"|"altKey"|"shiftKey"|"metaKey">):string|null {
  const key=/^Key[A-Z]$/.test(event.code)?event.code.slice(3).toLowerCase():/^Digit[0-9]$/.test(event.code)?event.code.slice(5):/^F([1-9]|1[0-9]|2[0-4])$/.test(event.code)?event.code:({Space:"space",Enter:"Return",Insert:"Insert",Home:"Home",End:"End",PageUp:"Page_Up",PageDown:"Page_Down"} as Record<string,string>)[event.code];
  if(!key||(!(event.ctrlKey||event.altKey||event.metaKey)&&!/^F\d+$/.test(key)))return null;
  const value=[event.ctrlKey?"CTRL":"",event.altKey?"ALT":"",event.shiftKey?"SHIFT":"",event.metaKey?"LOGO":"",key].filter(Boolean).join("+");
  return ["ALT+F4","CTRL+k","LOGO+k"].includes(value)?null:value;
}
export function validateVoiceShortcuts(value:VoiceShortcuts):string|null {
  for(const key of Object.values(value)){
    if(!key)continue;
    const parts=key.split("+"),main=parts.pop()!;
    const functionKey=/^F(?:[1-9]|1\d|2[0-4])$/.test(main);
    if(!/^(?:[a-z0-9]|F(?:[1-9]|1\d|2[0-4])|space|Return|Insert|Home|End|Page_Up|Page_Down)$/.test(main)||parts.some(part=>!["CTRL","ALT","SHIFT","LOGO"].includes(part))||new Set(parts).size!==parts.length||["ALT+F4","CTRL+k","LOGO+k"].includes(key)||(!functionKey&&!parts.some(part=>["CTRL","ALT","LOGO"].includes(part))))return "Escolha uma combinação com Ctrl, Alt ou Super, ou uma tecla F1–F24.";
  }
  return value.microphone&&value.microphone===value.deafen?"Escolha atalhos diferentes para microfone e silenciamento geral.":null;
}
export const voiceShortcutLabel=(value:string)=>value?value.split("+").map(key=>({CTRL:"Ctrl",SHIFT:"Shift",ALT:"Alt",LOGO:"Super",space:"Espaço",Return:"Enter"} as Record<string,string>)[key]??(key.length===1?key.toUpperCase():key)).join(" + "):"Não definido";
