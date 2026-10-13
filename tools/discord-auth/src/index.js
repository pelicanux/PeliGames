import { signEntitlement } from "./entitlement.js";
const API = "https://discord.com/api/v10";
const WEEK = 7 * 24 * 60 * 60 * 1000;
const TEN_MINUTES = 10 * 60 * 1000;
const json = (value, status = 200) => Response.json(value, { status, headers: { "Cache-Control": "no-store" } });
const html = (message, status = 200) => new Response(`<!doctype html><html lang="pt-BR"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>PeliGames — Discord</title><body style="background:#0b1220;color:#e2e8f0;font:18px system-ui;padding:48px"><h1>PeliGames</h1><p>${message}</p></body></html>`, { status, headers: { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store", "Content-Security-Policy": "default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'", "Referrer-Policy": "no-referrer" } });
async function digest(value) {
  const bytes = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return Array.from(new Uint8Array(bytes), b => b.toString(16).padStart(2, "0")).join("");
}
export function eligibility(member, env, userId) {
  if (env.OWNER_USER_ID && env.OWNER_USER_ID === userId) return "owner";
  const roles = Array.isArray(member.roles) ? member.roles : [];
  const youtube = Boolean(env.YOUTUBE_ROLE_ID && roles.includes(env.YOUTUBE_ROLE_ID));
  const booster = Boolean(member.premium_since || (env.BOOSTER_ROLE_ID && roles.includes(env.BOOSTER_ROLE_ID)));
  return youtube ? "youtube" : booster ? "booster" : null;
}
async function discord(path, token) {
  const response = await fetch(`${API}${path}`, { headers: { Authorization: `Bearer ${token}` }, signal: AbortSignal.timeout(15000) });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error("discord");
  return response.json();
}
async function exchange(env, fields, redirect) {
  const body = new URLSearchParams({ client_id: env.DISCORD_CLIENT_ID, client_secret: env.DISCORD_CLIENT_SECRET, ...fields });
  if (redirect) body.set("redirect_uri", redirect);
  const response = await fetch(`${API}/oauth2/token`, { method: "POST", body, signal: AbortSignal.timeout(15000) });
  if (!response.ok) throw new Error("authorization");
  const value = await response.json();
  if (!value.access_token || !value.refresh_token) throw new Error("authorization");
  return { ...value, expires_at: Date.now() + value.expires_in * 1000 };
}
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (!env.DISCORD_CLIENT_ID || !env.DISCORD_CLIENT_SECRET || !env.GUILD_ID) return json({ error: "Serviço Discord não configurado." }, 503);
    if (env.LIMITER && !(await env.LIMITER.limit({ key: request.headers.get("CF-Connecting-IP") || "unknown" })).success) return json({ error: "Aguarde antes de tentar novamente." }, 429);
    try {
      if (url.pathname === "/start" && request.method === "POST") {
        const id = crypto.randomUUID();
        const secret = crypto.randomUUID() + crypto.randomUUID();
        const session = env.SESSIONS.get(env.SESSIONS.idFromName(id));
        await session.fetch(new Request("https://session/init", { method: "POST", body: JSON.stringify({ id, secret_hash: await digest(secret), expires_at: Date.now() + TEN_MINUTES }) }));
        return json({ id, secret, browser_url: `${url.origin}/authorize?state=${id}` });
      }
      const id = url.pathname === "/authorize" || url.pathname === "/callback" ? url.searchParams.get("state") : request.headers.get("X-Session-Id");
      if (!id || !/^[0-9a-f-]{36}$/.test(id)) return json({ error: "Sessão inválida." }, 400);
      const session = env.SESSIONS.get(env.SESSIONS.idFromName(id));
      if (url.pathname === "/authorize" && request.method === "GET") {
        const ready = await session.fetch(new Request("https://session/authorize"));
        if (!ready.ok) return html("Este pedido expirou. Inicie o login novamente no launcher.", 400);
        const nonce = crypto.randomUUID();
        await session.fetch(new Request("https://session/browser", { method: "POST", body: JSON.stringify({ nonce }) }));
        const authorize = new URL("https://discord.com/oauth2/authorize");
        authorize.search = new URLSearchParams({ client_id: env.DISCORD_CLIENT_ID, response_type: "code", scope: "identify guilds.members.read", redirect_uri: `${url.origin}/callback`, state: id }).toString();
        return new Response(null, { status: 302, headers: { Location: authorize.href, "Set-Cookie": `pg_discord=${nonce}; HttpOnly; Secure; SameSite=Lax; Path=/callback; Max-Age=600`, "Cache-Control": "no-store", "Referrer-Policy": "no-referrer" } });
      }
      if (url.pathname === "/callback" && request.method === "GET") {
        const nonce = request.headers.get("Cookie")?.split(";").map(s => s.trim()).find(s => s.startsWith("pg_discord="))?.slice(11);
        const response = await session.fetch(new Request("https://session/callback", { method: "POST", body: JSON.stringify({ nonce, code: url.searchParams.get("code"), redirect: `${url.origin}/callback` }) }));
        const page = html(response.ok ? "Conta conectada. Volte ao PeliGames; esta página já pode ser fechada." : "Não foi possível conectar a conta. Volte ao launcher e tente novamente.", response.ok ? 200 : 400);
        page.headers.set("Set-Cookie", "pg_discord=; HttpOnly; Secure; SameSite=Lax; Path=/callback; Max-Age=0");
        return page;
      }
      if (!["/status", "/disconnect"].includes(url.pathname) || request.method !== "POST") return json({ error: "Rota inválida." }, 404);
      const secret = request.headers.get("Authorization")?.replace(/^Bearer /, "");
      if (!secret || secret.length > 200) return json({ error: "Sessão inválida." }, 401);
      return session.fetch(new Request(`https://session${url.pathname}`, { method: "POST", headers: { "X-Secret-Hash": await digest(secret) } }));
    } catch { return json({ error: "Não foi possível consultar o Discord. Tente novamente." }, 502); }
  }
};

export class DiscordSession {
  constructor(ctx, env) { this.ctx = ctx; this.env = env; this.queue = Promise.resolve(); }
  fetch(request) {
    // Serialize callback consumption and token refresh without holding the
    // Durable Object initialization gate open during network requests.
    const next = this.queue.then(() => this.handle(request));
    this.queue = next.catch(() => {});
    return next;
  }
  async handle(request) {
      const path = new URL(request.url).pathname;
      if (path === "/init") {
        const data = await request.json();
        await this.ctx.storage.put("session", { ...data, created_at: Date.now() });
        await this.ctx.storage.setAlarm(data.expires_at);
        return json({ ok: true });
      }
      const data = await this.ctx.storage.get("session");
      if (!data || data.expires_at <= Date.now()) return json({ error: "Sessão expirada. Conecte novamente." }, 401);
      if (path === "/authorize") return !data.tokens && !data.claimed ? json({ ok: true }) : json({}, 400);
      if (path === "/browser") {
        if (data.tokens || data.claimed) return json({}, 400);
        data.nonce = (await request.json()).nonce;
        await this.ctx.storage.put("session", data);
        return json({ ok: true });
      }
      if (path === "/callback") {
        const input = await request.json();
        if (!data.nonce || data.nonce !== input.nonce || data.claimed || !input.code) return json({}, 400);
        data.claimed = true;
        delete data.nonce;
        await this.ctx.storage.put("session", data);
        try {
          data.tokens = await exchange(this.env, { grant_type: "authorization_code", code: input.code }, input.redirect);
          data.account = await this.account(data.tokens.access_token);
          data.entitlement = await signEntitlement(data.account, data.id, this.env);
          data.checked_at = Date.now();
          data.expires_at = Date.now() + 90 * 24 * 60 * 60 * 1000;
          await this.ctx.storage.put("session", data);
          await this.ctx.storage.setAlarm(data.expires_at);
          return json({ ok: true });
        } catch { data.failed = true; await this.ctx.storage.put("session", data); return json({}, 400); }
      }
      if (request.headers.get("X-Secret-Hash") !== data.secret_hash) return json({}, 401);
      if (path === "/disconnect") {
        if (data.tokens) {
          const response = await fetch(`${API}/oauth2/token/revoke`, { method: "POST", body: new URLSearchParams({ client_id: this.env.DISCORD_CLIENT_ID, client_secret: this.env.DISCORD_CLIENT_SECRET, token: data.tokens.refresh_token, token_type_hint: "refresh_token" }), signal: AbortSignal.timeout(15000) });
          if (!response.ok) return json({ error: "Não foi possível revogar a autorização. Tente novamente." }, 502);
        }
        await this.ctx.storage.deleteAll();
        return json({ account: null });
      }
      if (data.failed) return json({ error: "Login não concluído. Tente novamente." }, 400);
      if (!data.tokens) return json({ pending: true, account: null });
      try {
        if (data.tokens.expires_at < Date.now() + 60000) {
          data.tokens = await exchange(this.env, { grant_type: "refresh_token", refresh_token: data.tokens.refresh_token });
          await this.ctx.storage.put("session", data);
        }
        data.account = await this.account(data.tokens.access_token);
          data.entitlement = await signEntitlement(data.account, data.id, this.env);
        data.checked_at = Date.now();
        data.expires_at = Date.now() + 90 * 24 * 60 * 60 * 1000;
        await this.ctx.storage.put("session", data);
        await this.ctx.storage.setAlarm(data.expires_at);
        return json({ pending: false, account: data.account, entitlement: data.entitlement });
      } catch { return json({ error: "Não foi possível renovar a conexão. Tente novamente ou reconecte sua conta." }, 502); }
  }
  async account(token) {
    const user = await discord("/users/@me", token);
    if (!user?.id) throw new Error("account");
    const member = await discord(`/users/@me/guilds/${this.env.GUILD_ID}/member`, token);
    const benefit = eligibility(member || {}, this.env, user.id);
    return { id: user.id, name: user.global_name || user.username, in_server: Boolean(member), benefit, valid_until: benefit ? Date.now() + WEEK : null };
  }
  async alarm() { await this.ctx.storage.deleteAll(); }
}
