#!/usr/bin/env bun
// Two local sessions; ephemeral credentials; loopback-only LiveKit server.
// Usage: bun scripts/test-native-call.ts [--check <rust test binary>]
import { createHash, createHmac, randomBytes } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer, connect } from "node:net";
import { createSocket } from "node:dgram";
import { arch, platform, tmpdir } from "node:os";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";

const ROOT = join(import.meta.dir, "..");
const VERSION = "1.13.9";
const SHA256 = "0b7fa208b662d09cfdeae8c06cf4c481aead0556086558b48250501e2e2d6e20";
const sha256 = (data: Uint8Array) => createHash("sha256").update(data).digest("hex");

/** The one regular file named `name` in a .tar.gz, read without extracting anything else. */
export function tarFile(archive: Uint8Array, name: string) {
  const data = gunzipSync(archive);
  const found: Uint8Array[] = [];
  for (let at = 0; at + 512 <= data.length;) {
    const header = data.subarray(at, at + 512);
    if (header.every(byte => byte === 0)) break;
    const field = (start: number, length: number) => new TextDecoder().decode(header.subarray(start, start + length)).replace(/\0.*$/s, "");
    const path = (field(345, 155) ? field(345, 155) + "/" : "") + field(0, 100);
    const size = parseInt(field(124, 12).trim() || "0", 8);
    const type = field(156, 1) || "0";
    if (path.split("/").pop() === name && type === "0") found.push(data.subarray(at + 512, at + 512 + size));
    at += 512 + Math.ceil(size / 512) * 512;
  }
  if (found.length !== 1) throw new Error("Pacote LiveKit inválido.");
  return found[0];
}

async function serverBinary() {
  if (platform() !== "linux" || arch() !== "x64") throw new Error("Este pacote de teste requer Linux x86_64.");
  const cache = join(ROOT, "src-tauri/target/rtc-test-server");
  mkdirSync(cache, { recursive: true });
  const archive = join(cache, `livekit-${VERSION}.tar.gz`);
  if (!existsSync(archive) || sha256(readFileSync(archive)) !== SHA256) {
    console.log("Baixando servidor LiveKit oficial para o teste local…");
    const url = `https://github.com/livekit/livekit/releases/download/v${VERSION}/livekit_${VERSION}_linux_amd64.tar.gz`;
    const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
    const data = new Uint8Array(await response.arrayBuffer());
    if (data.length > 100 * 1024 * 1024 || sha256(data) !== SHA256) throw new Error("Checksum do LiveKit inválido.");
    writeFileSync(archive, data);
  }
  const binary = join(cache, "livekit-server");
  writeFileSync(binary, tarFile(readFileSync(archive), "livekit-server"));
  chmodSync(binary, 0o700);
  return binary;
}

const base64url = (data: Buffer | string) => Buffer.from(data).toString("base64url");
export function token(key: string, secret: string, identity: string) {
  const now = Math.floor(Date.now() / 1000);
  const payload = { iss: key, sub: identity, name: "Sessão " + identity.toUpperCase(), nbf: now - 5, exp: now + 3600,
    video: { roomJoin: true, room: "peligames-local-audio", canPublish: true, canSubscribe: true, canPublishData: false } };
  const body = base64url(JSON.stringify({ alg: "HS256", typ: "JWT" })) + "." + base64url(JSON.stringify(payload));
  return body + "." + base64url(createHmac("sha256", secret).update(body).digest());
}

/** Fails early when another program holds the test ports. */
async function portsFree() {
  for (const port of [17880, 17881]) {
    await new Promise<void>((resolve, reject) => {
      const server = createServer().once("error", reject).listen(port, "127.0.0.1", () => server.close(() => resolve()));
    });
  }
  await new Promise<void>((resolve, reject) => {
    const socket = createSocket("udp4").once("error", reject);
    socket.bind(17882, "127.0.0.1", () => socket.close(() => resolve()));
  });
}
const listening = () => new Promise<boolean>(resolve => {
  const socket = connect(17880, "127.0.0.1").once("connect", () => { socket.destroy(); resolve(true); }).once("error", () => resolve(false));
  socket.setTimeout(200, () => { socket.destroy(); resolve(false); });
});

async function main() {
  const check = process.argv.indexOf("--check") > -1 ? process.argv[process.argv.indexOf("--check") + 1] : undefined;
  const binary = await serverBinary();
  await portsFree();
  const folder = mkdtempSync(join(tmpdir(), "peligames-call-"));
  const processes: ReturnType<typeof Bun.spawn>[] = [];
  try {
    const key = randomBytes(12).toString("hex"), secret = randomBytes(32).toString("hex");
    const config = join(folder, "server.yaml");
    writeFileSync(config, `port: 17880\nbind_addresses: ["127.0.0.1"]\nrtc:\n  tcp_port: 17881\n  udp_port: 17882\n  node_ip: 127.0.0.1\n  use_external_ip: false\n  enable_loopback_candidate: true\nkeys:\n  ${key}: ${secret}\n`, { mode: 0o600 });
    const credentials = join(folder, "sessions.json");
    writeFileSync(credentials, JSON.stringify({ url: "ws://127.0.0.1:17880", a: token(key, secret, "a"), b: token(key, secret, "b") }), { mode: 0o600 });
    const log = Bun.file(join(folder, "server.log"));
    const server = Bun.spawn([binary, "--config", config], { stdout: log, stderr: log });
    processes.push(server);
    let up = false;
    for (let i = 0; i < 100 && !up; i++) {
      if (server.exitCode !== null) throw new Error("O servidor local não iniciou. Verifique as portas 17880–17882.");
      up = await listening();
      if (!up) await Bun.sleep(100);
    }
    if (!up) throw new Error("O servidor local não respondeu.");
    const env = { ...process.env, PELIGAMES_RTC_TEST_CONFIG: credentials, PELIGAMES_RTC_TEST_ID: "a" };
    if (check) {
      const test = Bun.spawn([check, "commands::matrix_call::tests::two_sessions_exchange_audio", "--ignored", "--exact", "--nocapture"], { env, stdout: "inherit", stderr: "inherit" });
      const timer = setTimeout(() => test.kill(), 60_000);
      const code = await test.exited;
      clearTimeout(timer);
      return code;
    }
    console.log("Abrindo duas sessões. Clique em Entrar na chamada nas duas. Use fones e ative apenas um microfone por vez.");
    for (const identity of ["a", "b"]) processes.push(Bun.spawn([join(ROOT, "Release/peligames"), "--matrix-rtc-test"], { env: { ...env, PELIGAMES_RTC_TEST_ID: identity }, stdout: "inherit", stderr: "inherit" }));
    await Promise.all(processes.slice(1).map(process => process.exited));
    return 0;
  } finally {
    for (const process of processes.reverse()) {
      if (process.exitCode !== null) continue;
      process.kill();
      const stopped = await Promise.race([process.exited.then(() => true), Bun.sleep(5000).then(() => false)]);
      if (!stopped) process.kill(9);
    }
    rmSync(folder, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  process.on("SIGINT", () => process.exit(130));
  main().then(code => process.exit(code)).catch(error => { console.log(`Teste de chamada: ${error instanceof Error ? error.message : error}`); process.exit(1); });
}
