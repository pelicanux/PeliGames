const WEEK = 7 * 86400000;
const encode = bytes => btoa(String.fromCharCode(...bytes)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
export async function signEntitlement(account, sessionId, env) {
  if (!account.benefit || !env.ENTITLEMENT_SIGNING_KEY) return null;
  const issued = Date.now();
  const claims = { sub: account.id, sid: sessionId, aud: "peligames-cover-effects", guild: env.GUILD_ID, benefit: account.benefit, iat: issued, exp: issued + WEEK };
  const payload = encode(new TextEncoder().encode(JSON.stringify(claims)));
  const key = await crypto.subtle.importKey("jwk", JSON.parse(env.ENTITLEMENT_SIGNING_KEY), { name: "ECDSA", namedCurve: "P-256" }, false, ["sign"]);
  const signature = await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, key, new TextEncoder().encode(payload));
  return `${payload}.${encode(new Uint8Array(signature))}`;
}
