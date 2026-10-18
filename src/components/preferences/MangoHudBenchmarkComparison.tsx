import { SelectionMenu } from "../SelectionMenu";
import { MenuIcon } from "../MenuIcon";
import { manualBenchmark, webBenchmarkPicker, type ManualBenchmark } from "./manualBenchmark";
import { BenchmarkSimpleComparison } from "./BenchmarkSimpleComparison";
import { BenchmarkVisualComparison } from "./BenchmarkVisualComparison";
import { testLabel } from "./benchmarkPresentation";
import { saveBenchmarkComparison } from "./saveBenchmarkComparison";
import { useEffect, useId, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { notifyError, notifyInfo } from "../../services/notifications";
import { type BenchmarkStats } from "./mangoHudBenchmarks";
const pair = <T,>(values:[T,T], index:0|1, value:T):[T,T] => index===0 ? [value,values[1]] : [values[0],value];
interface File { name: string; size: number; modified: number }
export interface BenchmarkGame { name: string; executable?: string; directory?: string; path?: string }
export function MangoHudBenchmarkComparison({ shortcut, game, fullPage = false }: { shortcut: string; game?: BenchmarkGame; fullPage?: boolean }) {
  const id=useId();
  const [viewMode,setViewMode] = useState<"simple"|"detailed">("simple");
  const [testNames,setTestNames] = useState<[string,string]>(["Teste A","Teste B"]);
  const [manual,setManual] = useState<[ManualBenchmark|null,ManualBenchmark|null]>([null,null]);
  const [manualMode,setManualMode] = useState<[boolean,boolean]>([false,false]);
  const [picking,setPicking] = useState<0|1|null>(null);
  const pickRevision=useRef(0), fileInputs=useRef<[HTMLInputElement|null,HTMLInputElement|null]>([null,null]);
  const [files,setFiles] = useState<File[]>([]), [names,setNames] = useState<[string,string]>(["", ""]);
  const [results,setResults] = useState<[BenchmarkStats|null,BenchmarkStats|null]>([null,null]);
  const [busy,setBusy] = useState(false), [ready,setReady] = useState(false), [revision,setRevision] = useState(0);
  const [saving,setSaving] = useState(false);
  const saveLock=useRef(false);
  const request = useRef(0);
  const listing = useRef(0);
  const refresh = async (latest = false) => {
    const current = ++listing.current;
    setBusy(true);
    try {
      const values = await invoke<File[]>("list_mangohud_benchmarks", {gameName: game?.name, executable: game?.executable, gameDirectory: game?.directory || game?.path});
      if (current !== listing.current) return;
      values.sort((a,b) => b.modified-a.modified || a.name.localeCompare(b.name));
      setFiles(values);
      setNames(previous => latest ? [values[1]?.name || "", values[0]?.name || ""] : previous.map((name,index) => values.some(file => file.name === name) || manual[index]?.id === name ? name : "") as [string,string]);
      if(latest)setManualMode([false,false]);
      setReady(true); setRevision(value=>value+1);
    } catch (error) { if (current === listing.current) notifyError("Benchmarks MangoHUD",error); }
    finally { if (current === listing.current) setBusy(false); }
  };
  useEffect(() => {
    if (game) void refresh(true);
    return () => { ++listing.current; ++pickRevision.current; };
  }, [game?.name, game?.executable, game?.directory, game?.path]); // Only load on explicit game-comparison access.
  useEffect(() => {
    const current = ++request.current;
    setResults([null,null]);
    if (!names.some(Boolean)) return;
    const imported=names.map((name,index)=>manual[index]?.id===name ? manual[index]!.stats : null);
    if(names.every((name,index)=>!name || imported[index])) {setResults(imported as typeof results);return;}
    let active = true;
    const worker = new Worker(new URL("./mangoHudBenchmark.worker.ts", import.meta.url), { type: "module" });
    worker.onmessage = (event: MessageEvent<{ results?: typeof results; error?: string }>) => {
      if(active && current === request.current) {
        if(event.data.error) notifyError("Benchmarks MangoHUD",event.data.error);
        else if(event.data.results) setResults(event.data.results.map((result,index)=>imported[index] || result) as typeof results);
      }
      worker.terminate();
    };
    worker.onerror = () => { if(active) notifyError("Benchmarks MangoHUD","Não foi possível analisar esse benchmark."); worker.terminate(); };
    void Promise.all(names.map((name,index) => name && !imported[index] ? invoke<string>("read_mangohud_benchmark",{name}) : Promise.resolve(null)))
      .then(values => { if(active) worker.postMessage(values); })
      .catch(error => { if(active) notifyError("Benchmarks MangoHUD", error); worker.terminate(); });
    return () => { active = false; worker.terminate(); };
  }, [names, revision, manual]);
  const chooseManual = async (index:0|1, file?:globalThis.File) => {
    const current=++pickRevision.current;setPicking(index);
    try {
      const selected=await manualBenchmark(file);
      if(!selected || current!==pickRevision.current)return;
      setManual(previous=>pair(previous,index,selected));
      setNames(previous=>pair(previous,index,selected.id));
      setManualMode(previous=>pair(previous,index,false));
    } catch(error) {if(current===pickRevision.current)notifyError("Benchmarks MangoHUD",error);}
    finally {if(current===pickRevision.current)setPicking(null);}
  };
  const browse = (index:0|1) => {if(webBenchmarkPicker())fileInputs.current[index]?.click();else void chooseManual(index);};
  const selectionName=(index:0|1)=>manual[index]?.id===names[index] ? manual[index]!.name : names[index];
  const [a,b] = results;
  const saveImage = async () => {
    if (!a || !b || busy || saveLock.current) return;
    saveLock.current=true;setSaving(true);
    try {
      const saved=await saveBenchmarkComparison({game:game?.name,labels:[testLabel(testNames[0],0),testLabel(testNames[1],1)],names:[selectionName(0),selectionName(1)],dates:names.map((name,index)=>manual[index]?.id===name ? manual[index]!.modified : files.find(file=>file.name===name)?.modified) as [number|undefined,number|undefined],results:[a,b]});
      if(saved)notifyInfo("Benchmarks MangoHUD","Imagem da comparação salva.");
    } catch(error) { notifyError("Benchmarks MangoHUD",error); }
    finally { saveLock.current=false;setSaving(false); }
  };
  return <section className={`mm-benchmarks${fullPage ? " mm-benchmarks-full-page" : " mm-card"}`}>
    <div className="mm-benchmark-title">
      <h2>Comparar benchmarks{game ? ` — ${game.name}` : ""}</h2>
      <div className="mm-segmented" role="group" aria-label="Modo de comparação">
        <button type="button" aria-pressed={viewMode==="simple"} onClick={()=>setViewMode("simple")}>Simples</button>
        <button type="button" aria-pressed={viewMode==="detailed"} onClick={()=>setViewMode("detailed")}>Detalhado</button>
      </div>
    </div>
    <p className="mm-hint">No jogo, use <kbd>{shortcut}</kbd> para iniciar e encerrar cada gravação. Repita o mesmo trecho com os mesmos gráficos e resolução.</p>
    <div className="mm-benchmark-toolbar">
    {game && <button type="button" className="mm-ghost" disabled={busy || picking!==null} onClick={()=>void refresh(true)}>Comparar os dois últimos testes</button>}
    <button type="button" className="mm-ghost" disabled={busy || picking!==null} onClick={()=>void refresh()}>{busy ? "Lendo registros…" : "Atualizar registros"}</button>
    <button type="button" className="mm-ghost" disabled={saving || busy || picking!==null || !a || !b} onClick={()=>void saveImage()}>{saving ? "Salvando imagem…" : "Salvar comparação"}</button>
    </div>
    {!ready && <p className="mm-hint">Atualize para listar os CSVs organizados por jogo. Registros gerais são agrupados pelo executável.</p>}
    {ready && !files.length && !names.some(Boolean) && <p className="mm-hint">Nenhum benchmark encontrado. Grave um teste e atualize a lista ou procure um CSV manualmente.</p>}
    {ready && game && files.length === 1 && names.every((name,index)=>manual[index]?.id!==name) && <p className="mm-hint">Só há um teste deste jogo. Grave mais um para comparar. Você também pode selecionar os registros manualmente abaixo.</p>}
      <div className="mm-benchmark-choices">{([0,1] as const).map(index=><div className="mm-benchmark-choice" key={index}>
        <div className={`mm-test-name mm-test-${index===0?"a":"b"}`}>
          <label htmlFor={`${id}-name-${index}`}>{index===0?"A · Referência":"B · Comparação"}</label>
          <input id={`${id}-name-${index}`} aria-label={`Nome do teste ${index===0?"A":"B"}`} type="text" maxLength={48} value={testNames[index]} placeholder={`Teste ${index===0?"A":"B"}`} onChange={event=>setTestNames(previous=>pair(previous,index,event.target.value))} onBlur={()=>setTestNames(previous=>pair(previous,index,testLabel(previous[index],index)))} onKeyDown={event=>{if(event.key==="Enter")event.currentTarget.blur();}}/>
        </div>
        <span className="mm-record-label">Registro do teste {index===0?"A":"B"}</span>
        <div className="mm-benchmark-picker-row">
          <SelectionMenu label={`Benchmark ${index===0?"A":"B"}`} disabled={busy || picking!==null}
            className="mm-benchmark-select" menuClassName="mm-benchmark-options"
            value={manualMode[index]?"__manual__":names[index]}
            options={[
              {value:"",label:"Selecione um registro"},
              {value:"__manual__",label:"Selecionar registro manualmente…"},
              ...(manual[index] ? [{value:manual[index]!.id,label:`${manual[index]!.name} (manual)`}] : []),
              ...files.map(file=>({value:file.name,label:file.name,disabled:file.name===names[1-index]})),
            ]}
            onChange={value=>{
              if(value==="__manual__"){setManualMode(previous=>pair(previous,index,true));setNames(previous=>pair(previous,index,""));}
              else {setManualMode(previous=>pair(previous,index,false));setNames(previous=>pair(previous,index,value));}
            }} />
          {(manualMode[index] || manual[index]?.id===names[index]) && <button type="button" className="mm-ghost mm-benchmark-browse" disabled={picking!==null || busy} aria-label={`Procurar registro do teste ${index===0?"A":"B"}`} title="Procurar arquivo CSV" onClick={()=>browse(index)}><MenuIcon name="folderSearch"/></button>}
          <input type="file" accept=".csv,text/csv" hidden aria-label={`Arquivo CSV do teste ${index===0?"A":"B"}`} ref={element=>{fileInputs.current[index]=element;}} onChange={event=>{const file=event.currentTarget.files?.[0];event.currentTarget.value="";if(file)void chooseManual(index,file);}} />
        </div>
        {picking===index && <span className="mm-hint" role="status">Carregando registro…</span>}
      </div>)}</div>
      {(files.length>0 || names.some(Boolean)) && <>
      {viewMode==="simple" ? <BenchmarkSimpleComparison results={results} labels={testNames}/> : <BenchmarkVisualComparison results={results} labels={testNames}/>}
      <p className="mm-hint">Mínimos estimados pelas amostras gravadas; dependem do intervalo de registro. Dados ausentes aparecem como “—”. RAM corresponde ao uso do sistema. Os nomes são rótulos da comparação e não renomeiam os CSVs. O comparador não identifica alterações de Proton ou gráficos; duas gravações isoladas não comprovam um ganho consistente.</p>
    </>}
  </section>;
}
