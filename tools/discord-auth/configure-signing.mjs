// Private key exists only in memory and Wrangler stdin. Only public JWK is saved.
import { webcrypto } from "node:crypto";
import { spawnSync } from "node:child_process";
import { writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("./", import.meta.url));
const publicPath = fileURLToPath(new URL("../../src/theme/discordEntitlementKey.json", import.meta.url));
const { readFileSync } = await import("node:fs");
if (existsSync(publicPath) && JSON.parse(readFileSync(publicPath, "utf8")).kty === "EC") {
  console.log("Chave pública já configurada. Mantendo a assinatura existente.");
  process.exit(0);
}
const [runner, ...prefix] = process.argv.slice(2);
if (!runner) throw new Error("Informe bunx wrangler ou npx wrangler como argumentos.");
const keys = await webcrypto.subtle.generateKey({ name: "ECDSA", namedCurve: "P-256" }, true, ["sign", "verify"]);
const privateJwk = await webcrypto.subtle.exportKey("jwk", keys.privateKey);
const publicJwk = await webcrypto.subtle.exportKey("jwk", keys.publicKey);
console.log("Cadastrando a chave de assinatura no Cloudflare. O segredo não será exibido.");
const result = spawnSync(runner, [...prefix, "secret", "put", "ENTITLEMENT_SIGNING_KEY", "--config", `${root}wrangler.toml`], { input: JSON.stringify(privateJwk), stdio: ["pipe", "inherit", "inherit"], cwd: root });
if (result.error || result.status !== 0) { console.error("Cadastro não concluído. A chave pública não foi alterada."); process.exit(1); }
writeFileSync(publicPath, JSON.stringify(publicJwk, null, 2) + "\n");
console.log("Chave pública salva no launcher. Ela pode ser versionada; não contém o segredo.");
