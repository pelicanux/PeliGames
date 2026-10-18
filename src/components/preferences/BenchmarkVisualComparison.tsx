import { useState, type PointerEvent } from "react";
import type { BenchmarkStats } from "./mangoHudBenchmarks";
import { comparisonRows, comparisonValue } from "./mangoHudComparisonReport";
import { comparisonInsight, metricBarWidths, metricDescriptions, metricGroups, testLabel } from "./benchmarkPresentation";

export function BenchmarkVisualComparison({results,labels}:{results:[BenchmarkStats|null,BenchmarkStats|null];labels:[string,string]}) {
  const [a,b]=results;
  return <div className="mm-visual-comparison">
    <div className="mm-comparison-summary">{["averageFps","low1","frameTime"].map(key=>{
      const row=comparisonRows.find(row=>row.key===key)!, insight=comparisonInsight(a,b,row);
      return <div className="mm-summary-metric" key={key}>
        <span>{row.label}</span><div><strong className="mm-test-a">{comparisonValue(a?.[row.key],row.unit)}</strong><span aria-hidden="true">→</span><strong className="mm-test-b">{comparisonValue(b?.[row.key],row.unit)}</strong></div>
        <small data-tone={insight.tone}>{insight.text}</small>
      </div>;
    })}</div>
    <div className="mm-comparison-heading"><strong className="mm-test-a">A · {testLabel(labels[0],0)}</strong><span>Métrica no centro · B em relação a A</span><strong className="mm-test-b">B · {testLabel(labels[1],1)}</strong></div>
    {metricGroups.map(group=><section className="mm-metric-group" key={group.title}>
      <h3>{group.title}</h3>
      {comparisonRows.filter(row=>group.keys.some(key=>key===row.key)).map(row=>{
        const av=a?.[row.key],bv=b?.[row.key],widths=metricBarWidths(av,bv),insight=comparisonInsight(a,b,row);
        return <div className="mm-versus-row" key={row.key} role="group" aria-label={row.label}>
          <div className="mm-versus-value mm-side-a"><strong aria-label={`${testLabel(labels[0],0)}: ${comparisonValue(av,row.unit)}`}>{comparisonValue(av,row.unit)}</strong><span className="mm-metric-track" aria-hidden="true"><i style={{width:`${widths[0]}%`}}/></span></div>
          <div className="mm-versus-label"><strong>{row.label}</strong><small>{metricDescriptions[row.key]}</small><span className="mm-insight" data-tone={insight.tone}>{insight.text}</span></div>
          <div className="mm-versus-value mm-side-b"><strong aria-label={`${testLabel(labels[1],1)}: ${comparisonValue(bv,row.unit)}`}>{comparisonValue(bv,row.unit)}</strong><span className="mm-metric-track" aria-hidden="true"><i style={{width:`${widths[1]}%`}}/></span></div>
        </div>;
      })}
    </section>)}
    <p className="mm-hint">Cada par de barras usa a mesma escala, a partir de zero. As escalas variam entre métricas. CPU, memória, consumo e temperatura são contexto de uso; não definem sozinhos qual teste é melhor.</p>
    {(a||b) && <BenchmarkFrametimeChart results={results} labels={labels}/>}
  </div>;
}
function BenchmarkFrametimeChart({results,labels}:{results:[BenchmarkStats|null,BenchmarkStats|null];labels:[string,string]}) {
  const [hover,setHover]=useState<number|null>(null);
  const max=Math.ceil(Math.max(1,...results.flatMap(result=>result?.graph || []))/5)*5;
  const left=55,top=22,width=900,height=210;
  const move=(event:PointerEvent<SVGSVGElement>)=>{const rect=event.currentTarget.getBoundingClientRect();setHover(Math.max(0,Math.min(1,((event.clientX-rect.left)/rect.width*980-left)/width)));};
  return <section className="mm-frametime-card">
    <h3>Estabilidade dos quadros</h3><p className="mm-hint">Linhas mais baixas e regulares indicam menor tempo por quadro. Passe o mouse para consultar os valores.</p>
    <div className="mm-chart-legend"><span className="mm-test-a">● A · {testLabel(labels[0],0)}</span><span className="mm-test-b">● B · {testLabel(labels[1],1)}</span></div>
    <svg className="mm-frametime-chart" role="img" aria-label="Comparação do frametime dos testes A e B" viewBox="0 0 980 280" onPointerMove={move} onPointerLeave={()=>setHover(null)}>
      <text x="12" y="14">ms</text>
      {Array.from({length:5},(_,i)=>{const y=top+i*height/4;return <g key={i}><line x1={left} y1={y} x2={left+width} y2={y}/><text x={left-10} y={y+5} textAnchor="end">{(max*(1-i/4)).toLocaleString("pt-BR",{maximumFractionDigits:1})}</text></g>;})}
      {[0,.25,.5,.75,1].map(progress=><text key={progress} x={left+progress*width} y="258" textAnchor="middle">{progress*100}%</text>)}
      {results.map((result,index)=>result&&<polyline key={index} className={`mm-line-${index}`} points={result.graph.map((value,i)=>`${left+(result.graph.length===1?.5:i/(result.graph.length-1))*width},${top+height-value/max*height}`).join(" ")}/>)}
      {hover!==null && <line className="mm-chart-cursor" x1={left+hover*width} x2={left+hover*width} y1={top} y2={top+height}/>}
    </svg>
    <div className="mm-chart-readout" aria-live="off">{hover===null?"Progresso normalizado da gravação (0–100%)":`${Math.round(hover*100)}% do teste · `+results.map((result,index)=>`${index===0?"A":"B"}: ${result?comparisonValue(result.graph[Math.round(hover*(result.graph.length-1))],"ms"):"—"}`).join(" · ")}</div>
    <p className="mm-hint">O gráfico mostra médias por trecho das amostras, não todos os quadros. O progresso é normalizado; gravações diferentes não ficam sincronizadas pela cena.</p>
  </section>;
}
