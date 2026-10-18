import { comparisonInsight, metricBarWidths, metricGroups, metricDescriptions, testLabel } from "./benchmarkPresentation";
import type { BenchmarkStats } from "./mangoHudBenchmarks";

export const comparisonRows: {label:string; key:keyof BenchmarkStats; unit:string; lower?:boolean; neutral?:boolean}[] = [
  {label:"FPS médio",key:"averageFps",unit:"FPS"}, {label:"1% mínimo (amostras)",key:"low1",unit:"FPS"},
  {label:"0,1% mínimo (amostras)",key:"low01",unit:"FPS"}, {label:"Frametime médio",key:"frameTime",unit:"ms",lower:true},
  {label:"Uso médio da CPU",key:"cpuLoad",unit:"%",neutral:true},
  {label:"Uso médio da GPU",key:"gpuLoad",unit:"%",neutral:true},
  {label:"RAM média em uso (sistema)",key:"ramUsed",unit:"GiB",neutral:true},
  {label:"VRAM média em uso",key:"vramUsed",unit:"GiB",neutral:true},
  {label:"Consumo médio da GPU",key:"gpuPower",unit:"W",lower:true}, {label:"Temperatura média da GPU",key:"gpuTemp",unit:"°C",lower:true},
  {label:"Duração registrada",key:"duration",unit:"s"}, {label:"Amostras",key:"samples",unit:""},
];
export const comparisonValue = (value: unknown, unit: string) => typeof value === "number" && Number.isFinite(value) ? `${value.toLocaleString("pt-BR",{maximumFractionDigits:1})} ${unit}`.trim() : "—";
export interface ComparisonReport { game?: string; labels?:[string,string]; names: [string,string]; dates: [number|undefined,number|undefined]; results: [BenchmarkStats,BenchmarkStats] }
/** Render the complete report at a fixed readable size, independent of viewport or scroll. */
export async function comparisonPng(report: ComparisonReport): Promise<Blob> {
  const canvas=document.createElement("canvas"),width=1280,scale=2,ctx=canvas.getContext("2d");
  if(!ctx)throw new Error("Não foi possível criar a imagem da comparação.");
  const labels=[testLabel(report.labels?.[0],0),testLabel(report.labels?.[1],1)];
  const wrap=(text:string,maxWidth:number)=>{
    const lines:string[]=[];let current="";
    for(const word of text.split(/\s+/).filter(Boolean)) {
      const next=current?`${current} ${word}`:word;
      if(ctx.measureText(next).width<=maxWidth){current=next;continue;}
      if(current){lines.push(current);current="";}
      for(const char of word){
        if(current && ctx.measureText(current+char).width>maxWidth){lines.push(current);current="";}
        current+=char;
      }
    }
    if(current)lines.push(current);return lines;
  };
  ctx.font="bold 28px sans-serif";const title=wrap((report.game || "Comparação de benchmarks").slice(0,240),1176);
  ctx.font="bold 21px sans-serif";const labelLines=labels.map(label=>wrap(label,540));
  ctx.font="16px sans-serif";const filenames=report.names.map(name=>wrap(name.slice(0,512),540));
  const testsY=116+title.length*36,filenameY=testsY+32+Math.max(...labelLines.map(lines=>lines.length))*26;
  const summaryY=filenameY+Math.max(...filenames.map(lines=>lines.length))*22+62;
  const metricsY=summaryY+142,chartY=metricsY+metricGroups.length*44+comparisonRows.length*94+70;
  const height=chartY+430;
  canvas.width=width*scale;canvas.height=height*scale;ctx.scale(scale,scale);
  const colors=["#64b5f6","#fb923c"];
  const text=(value:string,x:number,y:number,color="#e9edf6",font="18px sans-serif",align:CanvasTextAlign="left")=>{ctx.fillStyle=color;ctx.font=font;ctx.textAlign=align;ctx.fillText(value,x,y);};
  ctx.fillStyle="#0e1018";ctx.fillRect(0,0,width,height);ctx.fillStyle="#169cff";ctx.fillRect(0,0,width,7);
  text("PeliGames  /  COMPARAÇÃO DE BENCHMARKS",52,52,"#78bfff","bold 17px sans-serif");
  title.forEach((line,i)=>text(line,52,100+i*36,"#f4f6fc","bold 28px sans-serif"));
  labels.forEach((_,i)=>{
    const x=52+i*640;
    text(i===0?"A · REFERÊNCIA":"B · COMPARAÇÃO",x,testsY,colors[i],"bold 15px sans-serif");
    labelLines[i].forEach((line,j)=>text(line,x,testsY+30+j*26,colors[i],"bold 21px sans-serif"));
    filenames[i].forEach((line,j)=>text(line,x,filenameY+25+j*22,"#a3afc5","16px sans-serif"));
    const date=report.dates[i];text(date?new Date(date*1000).toLocaleString("pt-BR"):"Data indisponível",x,summaryY-18,"#98a3b9","15px sans-serif");
  });
  ["averageFps","low1","frameTime"].forEach((key,i)=>{
    const row=comparisonRows.find(row=>row.key===key)!,insight=comparisonInsight(...report.results,row),x=36+i*406;
    ctx.fillStyle="#171d2a";ctx.fillRect(x,summaryY,396,112);
    text(row.label,x+18,summaryY+28,"#b9c5da","bold 17px sans-serif");
    text(comparisonValue(report.results[0][row.key],row.unit),x+18,summaryY+62,colors[0],"bold 23px sans-serif");
    text("→",x+178,summaryY+62,"#98a3b9","20px sans-serif");
    text(comparisonValue(report.results[1][row.key],row.unit),x+220,summaryY+62,colors[1],"bold 23px sans-serif");
    text(insight.text,x+18,summaryY+92,insight.tone==="positive"?"#6cdaa7":insight.tone==="negative"?"#ff9c98":"#b9c5da","15px sans-serif");
  });
  let y=metricsY;
  metricGroups.forEach(group=>{
    ctx.fillStyle="#1b2230";ctx.fillRect(36,y,1208,36);text(group.title,52,y+24,"#cbd6e8","bold 17px sans-serif");y+=44;
    comparisonRows.filter(row=>group.keys.some(key=>key===row.key)).forEach((row,i)=>{
      const av=report.results[0][row.key],bv=report.results[1][row.key],bars=metricBarWidths(av,bv),insight=comparisonInsight(...report.results,row);
      if(i%2===0){ctx.fillStyle="#141824";ctx.fillRect(36,y,1208,94);}
      text(comparisonValue(av,row.unit),365,y+38,colors[0],"bold 25px sans-serif","right");
      text(comparisonValue(bv,row.unit),915,y+38,colors[1],"bold 25px sans-serif");
      ctx.fillStyle="#293244";ctx.fillRect(60,y+58,305,8);ctx.fillRect(915,y+58,305,8);
      ctx.fillStyle=colors[0];ctx.fillRect(365-305*bars[0]/100,y+58,305*bars[0]/100,8);
      ctx.fillStyle=colors[1];ctx.fillRect(915,y+58,305*bars[1]/100,8);
      text(row.label,640,y+26,"#edf2fa","bold 17px sans-serif","center");
      ctx.font="13px sans-serif";const descriptions=wrap(metricDescriptions[row.key] || "",490);
      descriptions.forEach((line,j)=>text(line,640,y+46+j*15,"#98a3b9","13px sans-serif","center"));
      text(insight.text,640,y+83,insight.tone==="positive"?"#6cdaa7":insight.tone==="negative"?"#ff9c98":"#b9c5da","14px sans-serif","center");
      y+=94;
    });
  });
  text("Estabilidade dos quadros — menor tempo e menos variação são melhores",52,chartY-26,"#d5deee","bold 19px sans-serif");
  const left=90,top=chartY,graphWidth=1138,graphHeight=210,max=Math.ceil(Math.max(1,...report.results.flatMap(result=>result.graph))/5)*5;
  ctx.fillStyle="#171d2a";ctx.fillRect(left,top,graphWidth,graphHeight);
  for(let i=0;i<=4;i++) {
    const y=top+i*graphHeight/4;ctx.strokeStyle="#2e3749";ctx.beginPath();ctx.moveTo(left,y);ctx.lineTo(left+graphWidth,y);ctx.stroke();
    text((max*(1-i/4)).toLocaleString("pt-BR",{maximumFractionDigits:1}),left-12,y+5,"#98a3b9","15px sans-serif","right");
  }
  text("ms",40,top-10,"#98a3b9","15px sans-serif");
  report.results.forEach((result,index)=>{
    ctx.strokeStyle=colors[index];ctx.lineWidth=3;ctx.beginPath();
    result.graph.forEach((value,i)=>{const x=left+(result.graph.length===1?.5:i/(result.graph.length-1))*graphWidth,y=top+graphHeight-value/max*graphHeight;if(i===0)ctx.moveTo(x,y);else ctx.lineTo(x,y);});ctx.stroke();
  });
  [0,.25,.5,.75,1].forEach(progress=>text(`${progress*100}%`,left+progress*graphWidth,top+graphHeight+26,"#98a3b9","15px sans-serif","center"));
  labels.forEach((label,i)=>text(`${i===0?"A":"B"} · ${label}`,52+i*640,top+graphHeight+62,colors[i],"17px sans-serif"));
  const notes=["Barras: mesma escala por métrica, partindo de zero. Uso de hardware não define sozinho qual teste é melhor.","Mínimos estimados por amostragem. Gráfico: médias por trecho, com progresso normalizado; não sincroniza cenas.","RAM: uso do sistema. Compare o mesmo trecho, gráficos e resolução; duas gravações não comprovam ganho consistente.","Exportado em "+new Date().toLocaleString("pt-BR")+" · Dados ausentes: —."];
  notes.forEach((note,i)=>text(note,52,height-96+i*24,"#98a3b9","15px sans-serif"));
  return new Promise((resolve,reject)=>canvas.toBlob(blob=>blob?resolve(blob):reject(new Error("Não foi possível gerar o PNG.")),"image/png"));
}
