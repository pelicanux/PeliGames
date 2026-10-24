import {useState} from 'react';
import {useOpenRGB,configureOpenRGB,refreshOpenRGB,setOpenRGBDemoColor} from '../../theme/openrgbSync';
import {rgbHex,openrgbDisplayColor} from '../../theme/openrgbColor';
import {useAccentTheme} from '../../theme/ThemeProvider';
import {MenuIcon} from '../MenuIcon';
export function OpenRGBPreferences({pt}:{pt:boolean}){
 const {preferences,devices,color,status,error,busy,demoColor}=useOpenRGB(),{interfaceTheme}=useAccentTheme();
 const [port,setPort]=useState(String(preferences.port)),[invalid,setInvalid]=useState(false);
 const device=devices.find(d=>d.key===preferences.device),preview='__PELI_UI_PREVIEW__' in window;
 const applyPort=()=>{const value=Number(port);if(!Number.isInteger(value)||value<1||value>65535){setInvalid(true);return;}setInvalid(false);configureOpenRGB({port:value});};
 return <section className="interface-settings-card openrgb-preferences" data-preference-id="openrgb">
 <header className="interface-section-heading"><span className="mod-component-icon"><MenuIcon name="chainLinked"/></span><div><h3>OpenRGB</h3><p>{pt?'A cor do seu PC também no launcher.':'Your PC lighting color in the launcher.'}</p></div><span className="mod-status-badge" data-ready={status==='connected'} role="status">{status==='connected'?(preview?pt?'Simulação':'Simulation':pt?'Conectado':'Connected'):status==='connecting'?(pt?'Conectando…':'Connecting…'):status==='error'?(pt?'Desconectado':'Disconnected'):(pt?'Desligado':'Off')}</span></header>
 <label className="preference-switch"><input type="checkbox" checked={preferences.enabled} onChange={e=>configureOpenRGB({enabled:e.target.checked})}/><span className="preference-switch-track"/><span>{pt?'Sincronizar cor com OpenRGB':'Sync color with OpenRGB'}<small>{pt?'Reconecta automaticamente. Sua cor anterior volta ao desligar ou desconectar.':'Reconnects automatically. Your previous color returns when disabled or disconnected.'}</small></span></label>
 {preferences.enabled&&<>
 <div className="openrgb-connection-row"><label>{pt?'Porta SDK local':'Local SDK port'}<input aria-label={pt?'Porta SDK local':'Local SDK port'} type="number" min="1" max="65535" value={port} onChange={e=>setPort(e.target.value)} onBlur={applyPort} onKeyDown={e=>{if(e.key==='Enter')applyPort();}}/></label><button className="mod-action-secondary" type="button" disabled={busy} onClick={()=>void refreshOpenRGB()}><MenuIcon name="repair"/>{pt?'Reconectar':'Reconnect'}</button></div>{invalid&&<small role="alert">{pt?'Use uma porta de 1 a 65535.':'Use a port from 1 to 65535.'}</small>}
 <div className="openrgb-device-fields"><label>{pt?'Dispositivo':'Device'}<select disabled={!devices.length} value={preferences.device} onChange={e=>configureOpenRGB({device:e.target.value,zone:-1})}>{!device&&<option value={preferences.device}>{pt?'Aguardando dispositivo…':'Waiting for device…'}</option>}{devices.map(d=><option key={d.key} value={d.key}>{d.name}</option>)}</select></label>
 <label>{pt?'Zona':'Zone'}<select disabled={!device} value={preferences.zone} onChange={e=>configureOpenRGB({zone:Number(e.target.value)})}><option value={-1}>{pt?'Média do dispositivo':'Device average'}</option>{device?.zones.map((z,i)=><option key={i} value={i}>{z.name}</option>)}</select></label></div>
 {color&&<>
 <div className="openrgb-color"><i style={{background:rgbHex(color)}}/><code>{rgbHex(color).toUpperCase()}</code><small>{pt?'Cor recebida':'Received color'}</small></div>
 {rgbHex(openrgbDisplayColor(color))!==rgbHex(color)&&<div className="openrgb-color"><i style={{background:rgbHex(openrgbDisplayColor(color))}}/><code>{rgbHex(openrgbDisplayColor(color)).toUpperCase()}</code><small>{pt?'Tom na interface · brilho ajustado para leitura':'Interface color · brightness adjusted for readability'}</small></div>}
 </>}
 {preview&&<label className="openrgb-demo">{pt?'Simular cor do PC':'Simulate PC color'}<input aria-label={pt?'Simular cor do PC':'Simulate PC color'} type="color" value={rgbHex(demoColor)} onChange={e=>{const h=e.target.value;setOpenRGBDemoColor([1,3,5].map(i=>parseInt(h.slice(i,i+2),16)) as [number,number,number]);void refreshOpenRGB();}}/></label>}
 {interfaceTheme==='hackerman'&&<p>{pt?'Hackerman mantém a paleta verde. A sincronização é aplicada no Padrão e no Neon Wave.':'Hackerman keeps its green palette. Sync applies to Standard and Neon Wave.'}</p>}
 {error&&<p role="alert">{error}</p>}
 </>}
 <p className="openrgb-help">{pt?'No OpenRGB, ative SDK Server → Start Server (porta padrão 6742). Funciona com RPM, DEB, AppImage e Flatpak. Apenas lê as cores; não modifica seus LEDs. Efeitos do hardware podem não informar a cor em tempo real.':'In OpenRGB, enable SDK Server → Start Server (default port 6742). Works with RPM, DEB, AppImage and Flatpak. Reads colors only; never changes LEDs. Hardware effects may not report live colors.'}</p>
 </section>;
}
