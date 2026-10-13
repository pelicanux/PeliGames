import { test, expect } from "bun:test";
import { DiscordSession, eligibility } from "./index.js";
const env = { DISCORD_CLIENT_ID: "app", DISCORD_CLIENT_SECRET: "server-only", GUILD_ID: "guild", YOUTUBE_ROLE_ID: "youtube" };
function session() {
  const values = new Map();
  const ctx = { blockConcurrencyWhile: fn => fn(), storage: { get: async k => structuredClone(values.get(k)), put: async (k,v) => values.set(k,structuredClone(v)), deleteAll: async () => values.clear(), setAlarm: async () => {} } };
  return { object: new DiscordSession(ctx, env), values };
}
const req = (path, body, secret) => new Request(`https://session${path}`, { method: body || secret ? "POST" : "GET", headers: secret ? { "X-Secret-Hash": secret } : {}, body: body ? JSON.stringify(body) : undefined });
test("YouTube or boost qualifies, ordinary membership does not", () => {
  expect(eligibility({ roles: ["youtube"] }, env)).toBe("youtube");
  expect(eligibility({ roles: [], premium_since: "2026-10-12" }, env)).toBe("booster");
  expect(eligibility({ roles: ["other"] }, env)).toBeNull();
  expect(eligibility({ roles: ["boost"] }, { ...env, BOOSTER_ROLE_ID: "boost" })).toBe("booster");
});
test("pending sessions require launcher secret; expired requests fail", async () => {
  const { object } = session();
  await object.fetch(req("/init", { secret_hash: "secret", expires_at: Date.now()+60000 }));
  expect((await object.fetch(req("/status", null, "wrong"))).status).toBe(401);
  expect(await (await object.fetch(req("/status", null, "secret"))).json()).toEqual({ pending: true, account: null });
  await object.fetch(req("/init", { secret_hash: "secret", expires_at: 1 }));
  expect((await object.fetch(req("/authorize"))).status).toBe(401);
});
test("browser cookie is required and callback cannot be replayed; tokens never leave server", async () => {
  const { object, values } = session();
  await object.fetch(req("/init", { secret_hash: "secret", expires_at: Date.now()+60000 }));
  await object.fetch(req("/browser", { nonce: "browser" }));
  expect((await object.fetch(req("/callback", { nonce: "wrong", code: "code" }))).status).toBe(400);
  const original = globalThis.fetch;
  globalThis.fetch = async input => {
    const url = String(input);
    if (url.endsWith("/oauth2/token")) return Response.json({ access_token: "private-access", refresh_token: "private-refresh", expires_in: 3600 });
    if (url.endsWith("/users/@me")) return Response.json({ id: "user", username: "Member" });
    return Response.json({ roles: ["youtube"] });
  };
  try {
    expect((await object.fetch(req("/callback", { nonce: "browser", code: "code", redirect: "https://relay/callback" }))).status).toBe(200);
    expect((await object.fetch(req("/callback", { nonce: "browser", code: "code" }))).status).toBe(400);
    const response = await (await object.fetch(req("/status", null, "secret"))).json();
    expect(response.account.benefit).toBe("youtube");
    expect(response.account.valid_until-Date.now()).toBeGreaterThan(6*86400000);
    expect(JSON.stringify(response)).not.toContain("private-");
    expect(values.get("session").tokens.refresh_token).toBe("private-refresh");
    globalThis.fetch = async () => Response.json({ roles: [] });
    // Membership removed: a fresh successful check must clear eligibility.
    globalThis.fetch = async input => String(input).endsWith("/users/@me") ? Response.json({ id: "user", username: "Member" }) : Response.json({ roles: [] });
    expect((await (await object.fetch(req("/status", null, "secret"))).json()).account.valid_until).toBeNull();
    globalThis.fetch = async () => new Response(null, { status: 200 });
    expect((await object.fetch(req("/disconnect", null, "secret"))).status).toBe(200);
    expect(values.size).toBe(0);
  } finally { globalThis.fetch = original; }
});
