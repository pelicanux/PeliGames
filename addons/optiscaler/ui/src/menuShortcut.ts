// OptiScaler stores a single Windows virtual-key code, not a modifier combination.
const special:Record<string,[number,string]>={
 Backspace:[0x08,"Backspace"],Tab:[0x09,"Tab"],Enter:[0x0d,"Enter"],Pause:[0x13,"Pause"],CapsLock:[0x14,"Caps Lock"],
 Space:[0x20,"Space"],PageUp:[0x21,"Page Up"],PageDown:[0x22,"Page Down"],End:[0x23,"End"],Home:[0x24,"Home"],
 ArrowLeft:[0x25,"Left"],ArrowUp:[0x26,"Up"],ArrowRight:[0x27,"Right"],ArrowDown:[0x28,"Down"],
 PrintScreen:[0x2c,"Print Screen"],Insert:[0x2d,"Insert"],Delete:[0x2e,"Delete"],
 NumpadMultiply:[0x6a,"Numpad *"],NumpadAdd:[0x6b,"Numpad +"],NumpadSubtract:[0x6d,"Numpad -"],NumpadDecimal:[0x6e,"Numpad ."],NumpadDivide:[0x6f,"Numpad /"],
 NumLock:[0x90,"Num Lock"],ScrollLock:[0x91,"Scroll Lock"],Semicolon:[0xba,";"],Equal:[0xbb,"="],Comma:[0xbc,","],Minus:[0xbd,"-"],Period:[0xbe,"."],Slash:[0xbf,"/"],Backquote:[0xc0,"`"],BracketLeft:[0xdb,"["],Backslash:[0xdc,"\\"],BracketRight:[0xdd,"]"],Quote:[0xde,"'"],
};
const hex=(key:number)=>`0x${key.toString(16).toUpperCase().padStart(2,"0")}`;
export function optiscalerMenuKey(event:Pick<KeyboardEvent,"code"|"ctrlKey"|"altKey"|"shiftKey"|"metaKey">):string|null {
 if(event.ctrlKey||event.altKey||event.shiftKey||event.metaKey)return null;
 const code=event.code;
 if(/^Key[A-Z]$/.test(code))return hex(code.charCodeAt(3));
 if(/^Digit[0-9]$/.test(code))return hex(code.charCodeAt(5));
 if(/^Numpad[0-9]$/.test(code))return hex(0x60+Number(code.slice(6)));
 if(/^F([1-9]|1\d|2[0-4])$/.test(code))return hex(0x6f+Number(code.slice(1)));
 return special[code]?hex(special[code][0]):null;
}
export function optiscalerMenuKeyLabel(value:string):string {
 if(value==="auto")return "Insert";
 const key=/^0x[\da-f]+$/i.test(value)?Number.parseInt(value.slice(2),16):/^\d+$/.test(value)?Number(value):NaN;
 if(key>=0x41&&key<=0x5a||key>=0x30&&key<=0x39)return String.fromCharCode(key);
 if(key>=0x70&&key<=0x87)return `F${key-0x6f}`;
 if(key>=0x60&&key<=0x69)return `Numpad ${key-0x60}`;
 return Object.values(special).find(([number])=>number===key)?.[1]??value;
}
