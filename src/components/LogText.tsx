import { useMemo } from "react";

export function logLineTone(line: string): string | undefined {
  // Explicit levels take precedence over words occurring in a message/path.
  if (/\[(?:ERROR|ERR|ERRO|FATAL)\]|\berr:|^\s*(?:ERROR|FATAL|ERRO)\s*[:|]/i.test(line)) return "log-line-error";
  if (/\[(?:WARN|WARNING|ALERTA|AVISO)\]|\bwarn:|^\s*(?:WARN|WARNING|ALERTA|AVISO)\s*[:|]/i.test(line)) return "log-line-warning";
  if (/\[(?:SUCCESS|OK|SUCESSO)\]/i.test(line)) return "log-line-success";
  if (/\b(?:0 errors?|no errors?|sem erros?|nenhum erro)\b/i.test(line)) return "log-line-success";
  if (/successfully|\b(?:success|sucesso|concluído|concluída|finalizado|finalizada)\b|\bhas been (?:loaded|initialized|started)\b|\b\d+ plugin\(s\) loaded|compilation succeeded|import completed/i.test(line)) return "log-line-success";
  if (/\[(?:INFO|DEBUG|TRACE)\]/i.test(line)) return undefined;
  if (/^CrashType:\s*Crash\b|\b(?:error|fatal|panic|crashed|exception_access_violation|unhandled exception|segmentation fault|failed|failure|erro|falha|falhou)\b|not valid|path not found/i.test(line)) return "log-line-error";
  if (/\b(?:warning|warn|alerta|aviso)\b|ANTERIOR à última execução/i.test(line)) return "log-line-warning";

  return undefined;
}
export function LogText({ content }: { content: string }) {
  return useMemo(() => {
    const runs: {tone: string | undefined; text: string}[] = [];
    for (const raw of content.split(/\r?\n/)) {
      const line=raw.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "");
      const tone=logLineTone(line), previous=runs[runs.length-1];
      if (previous && previous.tone===tone) previous.text += `\n${line}`;
      else runs.push({tone,text:line});
    }
    return runs.map((run,index)=><span key={index} className={run.tone}>{run.text}{index<runs.length-1 ? "\n" : ""}</span>);
  }, [content]);
}

/** Limit initial rendering, independently of the on-disk retention budget. */
export function logSuffix(content: string, lines: number): string {
  let index=content.length-(content.endsWith("\n")?1:0);
  for(let count=0;count<lines;count++){index=content.lastIndexOf("\n",index-1);if(index<0)return content;}
  return content.slice(index+1);
}
