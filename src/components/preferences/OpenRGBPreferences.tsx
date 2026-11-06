import {useI18n} from "../../i18n/I18nContext";
import {useState} from 'react';
import {useOpenRGB,configureOpenRGB,refreshOpenRGB,setOpenRGBDemoColor} from '../../theme/openrgbSync';
import {rgbHex,openrgbDisplayColor} from '../../theme/openrgbColor';
import {useAccentTheme} from '../../theme/ThemeProvider';
import {MenuIcon} from '../MenuIcon';
import {SelectionMenu} from '../SelectionMenu';
export function OpenRGBPreferences(){
  const {t: tx} = useI18n();

 const {preferences,devices,color,status,error,busy,demoColor}=useOpenRGB(),{interfaceTheme}=useAccentTheme();
 const [port,setPort]=useState(String(preferences.port)),[invalid,setInvalid]=useState(false);
 const device=devices.find(d=>d.key===preferences.device),preview='__PELI_UI_PREVIEW__' in window;
 const applyPort=()=>{const value=Number(port);if(!Number.isInteger(value)||value<1||value>65535){setInvalid(true);return;}setInvalid(false);configureOpenRGB({port:value});};
 return <section className="interface-settings-card openrgb-preferences" data-preference-id="openrgb">
 <header className="interface-section-heading"><span className="mod-component-icon"><MenuIcon name="chainLinked"/></span><div><h3>OpenRGB</h3><p>{tx("ui-openrgb-preferences", "your_pc_lighting_color_in_the_launcher")}</p></div><span className="mod-status-badge" data-ready={status==='connected'} role="status">{status==='connected'?(preview?tx("ui-openrgb-preferences", "simulation"):tx("ui-openrgb-preferences", "connected")):status==='connecting'?(tx("ui-openrgb-preferences", "connecting")):status==='error'?(tx("ui-openrgb-preferences", "disconnected")):(tx("ui-openrgb-preferences", "off"))}</span></header>
 <label className="preference-switch"><input type="checkbox" checked={preferences.enabled} onChange={e=>configureOpenRGB({enabled:e.target.checked})}/><span className="preference-switch-track"/><span>{tx("ui-openrgb-preferences", "sync_color_with_openrgb")}<small>{tx("ui-openrgb-preferences", "reconnects_automatically_your_previous_color_returns_when_disabled_or_disconnected")}</small></span></label>
 {preferences.enabled&&<>
 <div className="openrgb-connection-row"><label>{tx("ui-openrgb-preferences", "local_sdk_port")}<input aria-label={tx("ui-openrgb-preferences", "local_sdk_port")} type="number" min="1" max="65535" value={port} onChange={e=>setPort(e.target.value)} onBlur={applyPort} onKeyDown={e=>{if(e.key==='Enter')applyPort();}}/></label><button className="mod-action-secondary" type="button" disabled={busy} onClick={()=>void refreshOpenRGB()}><MenuIcon name="repair"/>{tx("ui-openrgb-preferences", "reconnect")}</button></div>{invalid&&<small role="alert">{tx("ui-openrgb-preferences", "use_a_port_from_1_to_65535")}</small>}
 <div className="openrgb-device-fields"><div className="openrgb-device-field"><span>{tx("ui-openrgb-preferences", "device")}</span><SelectionMenu label={tx("ui-openrgb-preferences", "openrgb_device")} disabled={!devices.length} value={preferences.device} onChange={device=>configureOpenRGB({device,zone:-1})} options={[...(!device?[{value:preferences.device,label:tx("ui-openrgb-preferences", "waiting_for_device")}]:[]),...devices.map(d=>({value:d.key,label:d.name}))]}/></div>
 <div className="openrgb-device-field"><span>{tx("ui-openrgb-preferences", "zone")}</span><SelectionMenu label={tx("ui-openrgb-preferences", "openrgb_zone")} disabled={!device} value={String(preferences.zone)} onChange={zone=>configureOpenRGB({zone:Number(zone)})} options={[{value:'-1',label:tx("ui-openrgb-preferences", "device_average")},...(device?.zones.map((z,i)=>({value:String(i),label:z.name}))??[])]}/></div></div>
 {color&&<>
 <div className="openrgb-color"><i style={{background:rgbHex(color)}}/><code>{rgbHex(color).toUpperCase()}</code><small>{tx("ui-openrgb-preferences", "received_color")}</small></div>
 {rgbHex(openrgbDisplayColor(color))!==rgbHex(color)&&<div className="openrgb-color"><i style={{background:rgbHex(openrgbDisplayColor(color))}}/><code>{rgbHex(openrgbDisplayColor(color)).toUpperCase()}</code><small>{tx("ui-openrgb-preferences", "interface_color_brightness_adjusted_for_readability")}</small></div>}
 </>}
 {preview&&<label className="openrgb-demo">{tx("ui-openrgb-preferences", "simulate_pc_color")}<input aria-label={tx("ui-openrgb-preferences", "simulate_pc_color")} type="color" value={rgbHex(demoColor)} onChange={e=>{const h=e.target.value;setOpenRGBDemoColor([1,3,5].map(i=>parseInt(h.slice(i,i+2),16)) as [number,number,number]);void refreshOpenRGB();}}/></label>}
 {interfaceTheme==='hackerman'&&<p>{tx("ui-openrgb-preferences", "hackerman_keeps_its_green_palette_sync_applies_to_standard_and_neon_wave")}</p>}
 {error&&<p role="alert">{error}</p>}
 </>}
 <p className="openrgb-help">{tx("ui-openrgb-preferences", "in_openrgb_enable_sdk_server_start_server_default_port_6742_works_with_rpm_deb_appimage_")}</p>
 </section>;
}
