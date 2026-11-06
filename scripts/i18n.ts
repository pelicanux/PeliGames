import {generate, validateCatalogs} from "./i18n/catalogs";
import {auditSources} from "./i18n/audit";
import {exportKit, importKit} from "./i18n/kits";
const [command = "check", tag, folder] = process.argv.slice(2);
try {
  if (command === "generate") console.log("Generated", await generate());
  else if (command === "check") console.log(`Translation catalogues valid: ${(await validateCatalogs()).size} languages.`);
  else if (command === "audit") {
    const report = await auditSources();
    console.log(JSON.stringify(report, null, 2));
    if (report.binary.length) process.exitCode = 1;
  }
  else if (command === "export" && tag && folder) console.log("Translation kit exported:", await exportKit(tag, folder));
  else if (command === "import" && tag && folder) {await importKit(tag, folder); console.log("Imported and generated", await generate());}
  else throw Error("Commands: check, generate, audit, export <locale> <folder>, import <locale> <folder>");
} catch (error) {console.error(error instanceof Error ? error.message : error); process.exitCode = 1;}
