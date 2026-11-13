import {resolve,join,basename} from "node:path";
import {mkdir,copyFile,cp,writeFile,readFile} from "node:fs/promises";
import {buildAddonUi} from "./build-ui";

// Package only explicitly staged runtime files, never a developer's source tree.
const [targetDir,outputDir,tag]=process.argv.slice(2);
if(!targetDir||!outputDir||!tag||!/^[a-zA-Z0-9._-]+$/.test(tag))throw new Error("Usage: bun package-release.ts TARGET_DIR OUTPUT_DIR RELEASE_TAG");
const root=resolve(import.meta.dir,"../../..");
const output=resolve(outputDir);
const kit=join(resolve(targetDir),"release/peligames-addon");
const recipes=["optiscaler","afmf","mako","dlssnr-mochizuki","gpu-screen-recorder"];
const entries=[];
await mkdir(output,{recursive:true});
for(const id of recipes){
 const source=join(root,"addons",id);
 const manifest=JSON.parse(await readFile(join(source,"manifest.json"),"utf8"));
 // These artifacts are built on Linux x86_64; other architectures need their own build.
 manifest.platforms=["linux-x86_64"];
 const payload=join(output,"payload",id);
 await mkdir(join(payload,"ui"),{recursive:true});await mkdir(join(payload,"bin"),{recursive:true});
 const cargo=await readFile(join(source,"service/Cargo.toml"),"utf8");
 const crate=cargo.match(/name\s*=\s*"([^"]+)"/)?.[1];if(!crate)throw new Error(`Missing service name: ${id}`);
 await copyFile(join(resolve(targetDir),"release",crate),join(payload,manifest.entrypoints.service));
 await writeFile(join(payload,manifest.entrypoints.ui),await buildAddonUi(join(source,"ui/src/index.tsx")),{flag:"wx"});
 await copyFile(join(root,"LICENSE"),join(payload,"LICENSE"));
 await cp(join(root,"licenses/upstream"),join(payload,"licenses/upstream"),{recursive:true});
 await copyFile(join(root,"licenses/generated/Rust-Dependencies.txt"),join(payload,"licenses/Rust-Dependencies.txt"));
 const manifestPath=join(output,`${id}-manifest.json`);await writeFile(manifestPath,JSON.stringify(manifest,null,2));
 const file=join(output,`${id}-${manifest.version}-linux-x86_64.zip`);
 const result=Bun.spawnSync([kit,"pack",manifestPath,payload,file]);
 if(result.exitCode!==0)throw new Error(result.stderr.toString());
 const artifact=JSON.parse(result.stdout.toString());
 entries.push({manifest,artifacts:{"linux-x86_64":{url:`https://github.com/pelicanux/PeliGames/releases/download/${tag}/${basename(file)}`,sha256:artifact.sha256,bytes:artifact.bytes}}});
}
const catalog=join(output,"catalog.json");await writeFile(catalog,JSON.stringify({schema_version:1,addons:entries},null,2)+"\n");
const check=Bun.spawnSync([kit,"validate-catalog",catalog]);if(check.exitCode!==0)throw new Error(check.stderr.toString());
console.log(check.stdout.toString());
