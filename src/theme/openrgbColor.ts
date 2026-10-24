import type {Appearance} from './accentTheme';
export type RGB=readonly [number,number,number];
export function validRGB(value:unknown):value is RGB{return Array.isArray(value)&&value.length===3&&value.every(n=>Number.isInteger(n)&&n>=0&&n<=255);}
export function rgbHex(color:RGB){return '#'+color.map(n=>n.toString(16).padStart(2,'0')).join('');}
export function rgbHue(color:RGB){const [r,g,b]=color.map(n=>n/255),max=Math.max(r,g,b),min=Math.min(r,g,b),d=max-min;if(!d)return 275;return ((max===r?(g-b)/d:max===g?(b-r)/d+2:(r-g)/d+4)*60+360)%360;}
/** LED brightness is independent of the accent's readability on screen. */
export function openrgbDisplayColor(color:RGB):RGB {
 const peak=Math.max(...color);
 if(peak===0||peak>=184)return color;
 return color.map(channel=>Math.round(channel*184/peak)) as [number,number,number];
}
export function openrgbVariables(color:RGB,appearance:Appearance):Record<string,string>{
 color=openrgbDisplayColor(color);
 const hue=rgbHue(color),luminance=(v:RGB)=>{const c=v.map(n=>{const s=n/255;return s<=.04045?s/12.92:((s+.055)/1.055)**2.4;});return c[0]*.2126+c[1]*.7152+c[2]*.0722;};
 const mix=(c:RGB,to:number,t:number)=>c.map(n=>Math.round(n+(to-n)*t)) as [number,number,number];
 let readable=[...color] as [number,number,number];
 const target=appearance==='dark'?.21:.12;
 for(let i=0;i<20&&(appearance==='dark'?luminance(readable)<target:luminance(readable)>target);i++)readable=mix(readable,appearance==='dark'?255:0,.12);
 const button=[...color] as [number,number,number],lum=luminance(button),white=1.05/(lum+.05),black=(lum+.05)/.05;
 const accent=rgbHex(readable);
 return {'--accent':accent,'--accent-rgb':readable.join(', '),'--accent-soft':rgbHex(mix(readable,appearance==='dark'?255:0,.25)),'--accent-light':rgbHex(mix(readable,appearance==='dark'?255:0,.4)),'--accent-button':rgbHex(button),'--accent-deep':rgbHex(mix(button,0,.25)),'--accent-hover':rgbHex(mix(button,appearance==='dark'?255:0,.12)),'--accent-deeper':rgbHex(mix(button,0,.4)),'--accent-foreground':white>black?'#ffffff':'#000000','--background-hue':`${hue}deg`,'--brand-hue':`${hue-210}deg`};
}
