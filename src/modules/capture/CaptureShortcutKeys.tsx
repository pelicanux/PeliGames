import {shortcutLabel} from "../../components/preferences/mangoHudShortcut";
export function CaptureShortcutKeys({value}:{value?:string}){
 if(!value)return <span className="capture-shortcut-empty">Sem atalho</span>;
 const labels:Record<string,string>={alt_l:"Alt",alt_r:"Alt",control_l:"Ctrl",control_r:"Ctrl",shift_l:"Shift",shift_r:"Shift",super_l:"Super",super_r:"Super"};
 return <span className="capture-shortcut-keys" aria-label={`Atalho: ${shortcutLabel(value)}`}>{value.split('+').map((key,index)=><kbd key={index}>{labels[key.toLowerCase()]||key.replace(/_/g,' ').toUpperCase()}</kbd>)}</span>;
}
