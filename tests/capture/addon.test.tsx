import {expect,test} from 'bun:test';
import {mkdtemp,mkdir,cp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';import {join} from 'node:path';
import {renderToStaticMarkup} from 'react-dom/server';
import {buildAddonUi} from '../../addons/_infra/tools/build-ui';
import type {AddonHost} from '../../addons/_infra/ui-sdk';
import {importAddonModule} from '../../src/modules/addons/moduleImport';
import {ResourceScope} from '../../src/modules/addons/lifecycle';
import {createAddonI18n} from '../../src/modules/addons/i18n';
import {addonDesign} from '../../src/modules/addons/sharedDesign';
import {I18nProvider} from '../../src/i18n/I18nContext';
import {loadLanguage} from '../../src/i18n/engine';
import {checkMessages} from '../../scripts/i18n/catalogs';
import en from '../../addons/gpu-screen-recorder/ui/locales/en.json';
import pt from '../../addons/gpu-screen-recorder/ui/locales/pt-BR.json';
import {defaults,connect} from '../../addons/gpu-screen-recorder/ui/src/api';
import {Controls} from '../../addons/gpu-screen-recorder/ui/src/Controls';
function bridge(language:string,scope:ResourceScope):AddonHost{return {...addonDesign,apiVersion:1,addon:{id:'gpu-screen-recorder',version:'0.1.0'},language,i18n:createAddonI18n(language,scope),lifecycle:scope,styles:{add(){}},service:{async call(){throw Error('SSR cannot record or start services')}},setBusy(){}}}
test('capture translations and presets belong to the independent package',async()=>{
 expect(checkMessages({...en},{...pt},'pt-BR',false,'pt-BR')).toEqual([]);
 await loadLanguage('pt-BR');const scope=new ResourceScope(()=>{}),host=bridge('pt-BR',scope);
 try{for(const section of ['video','audio'] as const){const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><Controls host={host} api={connect(host)} value={{...defaults,fps:90}} patch={()=>{}} disabled={false} section={section}/></I18nProvider>);if(section==='video'){expect(html).toContain('<span>90</span>');expect(html).toContain('qualidade constante')}else{expect(html.match(/role="switch"/g)).toHaveLength(3);expect(html).toContain('mod-option-switch--rail')}}}finally{await scope.close()}
});
test('capture UI builds outside the launcher and contributes settings and game views in both languages',async()=>{
 const root=await mkdtemp(join(tmpdir(),'peligames-capture-ui-'));
 try{await mkdir(join(root,'addons/_infra'),{recursive:true});await cp('addons/gpu-screen-recorder/ui',join(root,'addons/gpu-screen-recorder/ui'),{recursive:true});await cp('addons/_infra/ui-sdk',join(root,'addons/_infra/ui-sdk'),{recursive:true});const source=await buildAddonUi(join(root,'addons/gpu-screen-recorder/ui/src/index.tsx'));
 for(const forbidden of ['react-dom','node_modules/react/','/home/',root])expect(source).not.toContain(forbidden);
 const module=await importAddonModule({source} as Parameters<typeof importAddonModule>[0]);
 for(const language of ['en','pt-BR']){await loadLanguage(language);const scope=new ResourceScope(()=>{});try{const instance=await module.createAddon(bridge(language,scope));const Preferences=instance.views.preferences,Game=instance.views.game;const settings=renderToStaticMarkup(<I18nProvider initialPreference={language}><Preferences context={{target:'settings',route:'preferences'}} disabled={false}/></I18nProvider>);expect(settings).toContain(language==='en'?'Download component':'Baixar componente');expect(settings).toContain('git.dec05eba.com');const game=renderToStaticMarkup(<I18nProvider initialPreference={language}><Game context={{target:'game-captures',route:'game',game:{id:'fixture',directory:'/fixture'}}} disabled={false}/></I18nProvider>);expect(game.match(/role="tab" /g)).toHaveLength(7);expect(game).toContain(language==='en'?'Start replay':'Iniciar replay');await instance.dispose()}finally{await scope.close()}}
 }finally{await rm(root,{recursive:true,force:true})}
});

test('preview and application indicators share the same capture provider state',async()=>{
 const {createCapturePreview}=await import('./preview');const events:unknown[]=[];
 const provider=createCapturePreview(status=>events.push(status));
 provider.call('settings.save',{options:{...defaults,game_key:'fixture',game_name:'Fixture',replay_seconds:137,replay_on_launch:true}});
 expect((provider.call('settings.read',{gameKey:'fixture'}) as typeof defaults).replay_seconds).toBe(137);
 provider.call('session.start',{options:{...defaults,game_key:'fixture',game_name:'Fixture'},replay:true});
 expect(provider.call('session.status',{})).toMatchObject({phase:'active',replay:true,recording:false});
 provider.call('session.action',{action:'stop'});
 expect(provider.call('session.status',{})).toMatchObject({phase:'idle',replay:false});expect(events).toHaveLength(2);
});
