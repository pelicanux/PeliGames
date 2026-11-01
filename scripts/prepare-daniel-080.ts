#!/usr/bin/env bun
/** Assemble an opt-in local runtime package. Proprietary files are supplied locally, never bundled in PeliGames. */
import { copyFileSync, mkdirSync, readdirSync, readFileSync, existsSync, lstatSync, chmodSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve, relative, dirname } from "node:path";
import { createHash } from "node:crypto";
const args = Bun.argv.slice(2);
const options = new Map<string,string>();
for(let i=0;i<args.length;i+=2) { if(!["--base","--host","--bridge","--output"].includes(args[i]) || !args[i+1]) throw new Error("Usage: bun scripts/prepare-daniel-080.ts --base <prepared-0.6> --host <prepared-0.8-host> --bridge <paired-v3-build> --output <new-folder>"); options.set(args[i],resolve(args[i+1])); }
for(const key of ["--base","--host","--bridge","--output"]) if(!options.has(key)) throw new Error(`Missing ${key}`);
const base=options.get("--base")!,host=options.get("--host")!,bridge=options.get("--bridge")!,output=options.get("--output")!;
if(existsSync(output)) throw new Error("Output already exists; do not overwrite prepared runtimes");
const sha=(path:string)=>createHash("sha256").update(readFileSync(path)).digest("hex");
const hub="AMDNR-NeuralHub-080";
for(const [file,expected] of [
 [join(host,hub,"neuralhub_host.dll"),"be9172b68d0e77f98f04f941f4e2abe2c73eb01b8aafe94b26ee31eefefb5eb8"],
 [join(host,"winmm.dll"),"f693f539d2637507fb3744cb598e45d11119b2974e200a407fbe61055a29dd79"],
 [join(bridge,"x86_64-windows/amdhip64_7.dll"),"181c8fbe1a63976a51c39684ee0ce8b064d7c83ebf8a84eec19ef06c87e8038a"],
 [join(bridge,"x86_64-unix/amdhip64_7.so"),"40e364d2403b58690158bbf533550b90cf79f75ff559571e276423e5f3f73b7e"],
 [join(bridge,"x86_64-unix/libamdnr_signal_guard.so"),"81694316d032c4242c6f1419822634563fc818fd3a706ca183c520ecc292a21d"],
]) if(sha(file)!==expected) throw new Error(`Unvalidated host/bridge: ${file}`);
const dependencies=readFileSync(join(host,hub,"data/index.ini"),"utf8");
for(const block of dependencies.split(/(?=^\[)/m)) {
 const file=/^File=(.+)$/m.exec(block)?.[1].trim(),expected=/^Sha256=(.+)$/m.exec(block)?.[1].trim();
 if(file && expected && sha(join(host,hub,"data",file))!==expected) throw new Error(`Dependency integrity mismatch: ${file}`);
}
function files(folder:string):string[] { return readdirSync(folder,{withFileTypes:true}).flatMap(e=>{const p=join(folder,e.name);if(lstatSync(p).isSymbolicLink())throw new Error(`Symlink refused: ${p}`);return e.isDirectory()?files(p):[p];}); }
function copy(source:string,target:string) { if(!lstatSync(source).isFile())throw new Error(`Not a file: ${source}`);mkdirSync(dirname(target),{recursive:true});copyFileSync(source,target);chmodSync(target,target.endsWith(".sh")?0o700:0o600); }
function ini(input:string,section:string,key:string,value:string):string {
 let found=false,inSection=false;const lines=input.replace(/\r\n/g,"\n").split("\n");
 const out=lines.map(line=>{const match=/^\s*\[([^\]]+)\]/.exec(line);if(match){inSection=match[1].toLowerCase()===section.toLowerCase();}if(inSection&&new RegExp(`^\\s*${key}\\s*=`,'i').test(line)){found=true;return `${key}=${value}`;}return line;});
 if(!found){const index=out.findIndex(l=>l.trim().toLowerCase()===`[${section.toLowerCase()}]`);if(index>=0)out.splice(index+1,0,`${key}=${value}`);else out.push(`[${section}]`,`${key}=${value}`);}
 return out.join("\n");
}
mkdirSync(output,{recursive:true});
try {
 // Copy only the existing Linux launcher, private ROCm and native compiler/D3D12 support.
 for(const file of files(join(base,"amdnr-linux"))) {
  const rel=relative(base,file);
  if(rel.endsWith(".log")||rel.includes("/prefix")||rel.includes("/__pycache__/"))continue;
  copy(file,join(output,rel));
 }
 for(const name of ["d3d12.dll","d3d12core.dll","d3dcompiler_47.dll"])copy(join(base,name),join(output,name));
 for(const file of files(host)) {
  const rel=relative(host,file);
  if(rel==="test-provenance.json"||rel.endsWith(".log")||rel.includes("/cache/"))continue;
  copy(file,join(output,rel));
 }
 for(const file of files(bridge))copy(file,join(output,"amdnr-linux",relative(bridge,file)));
 writeFileSync(join(output,hub,"settings.ini"),"[Compute]\nBackend=HIP\n[Debug]\nHipPredicatedWait=0\n");
 writeFileSync(join(output,hub,"neuralhub.ini"),"[Upscaling]\nProvider=Passthrough\n[RayReconstruction]\nProvider=Passthrough\n[FrameGeneration]\nProvider=Off\n[Enhancement]\nProvider=DLSS-NR\nPosition=Pre-upscale\n");
 let config=readFileSync(join(output,"OptiScaler.ini"),"utf8");
 for(const [section,key,value] of [["DLSS","NeuralHubEnabled","true"],["DLSS","NeuralHubNrSelected","true"],["DLSS","NeuralHubNrEnabled","true"],["DlssNr","NrBackend","daniel"],["DlssNr","Enabled","true"],["Libraries","NvngxPath",hub+"\\neuralhub_host.dll"],["FrameGen","Enabled","false"],["Menu","ShortcutKey","0x22"],["Menu","FpsCycleShortcutKey","-1"]])config=ini(config,section,key,value);
 writeFileSync(join(output,"OptiScaler.ini"),config);
 const components=JSON.parse(readFileSync(join(output,"amdnr-linux/components.json"),"utf8"));components.daniel={version:"0.8.0",bridge:"peligames-hip-080-v3"};writeFileSync(join(output,"amdnr-linux/components.json"),JSON.stringify(components,null,2));
 const manifest={schema:1,runtime:"0.8.0",bridge:"peligames-hip-080-v3",files:Object.fromEntries(files(output).sort().map(p=>[relative(output,p),sha(p)]))};
 writeFileSync(join(output,"peligames-runtime.json"),JSON.stringify(manifest,null,2)+"\n");
 console.log(`Prepared local Daniel 0.8.0 package: ${output} (${Object.keys(manifest.files).length} files). Import this folder in PeliGames. Game files were not changed.`);
} catch(error) { rmSync(output,{recursive:true,force:true});throw error; }
