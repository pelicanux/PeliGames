import pinnedKey from "./discordEntitlementKey.json";
export interface DiscordStatus {
  configured: boolean; pending: boolean; offline: boolean;
  session_id?: string | null; entitlement?: string | null;
  account: { id: string; name: string; in_server: boolean; benefit: "youtube" | "booster" | "owner" | null; valid_until: number | null } | null;
}
export interface BenefitClaims { sub: string; sid: string; aud: string; guild: string; benefit: string; iat: number; exp: number }
const decode = (text: string) => Uint8Array.from(atob(text.replace(/-/g, "+").replace(/_/g, "/")), c => c.charCodeAt(0));
export async function verifyDiscordBenefit(status: DiscordStatus, key: JsonWebKey = pinnedKey as JsonWebKey, now = Date.now()): Promise<BenefitClaims | null> {
  try {
    if (!status.account || !status.session_id || !status.entitlement || status.pending || key.kty !== "EC" || key.crv !== "P-256") return null;
    const parts = status.entitlement.split(".");
    if (parts.length !== 2 || status.entitlement.length > 4096) return null;
    const publicKey = await crypto.subtle.importKey("jwk", key, { name: "ECDSA", namedCurve: "P-256" }, false, ["verify"]);
    if (!await crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, publicKey, decode(parts[1]), new TextEncoder().encode(parts[0]))) return null;
    const value = JSON.parse(new TextDecoder().decode(decode(parts[0]))) as BenefitClaims;
    if (value.sub !== status.account.id || value.sid !== status.session_id || value.aud !== "peligames-cover-effects" || value.guild !== "1088170837921779812" || !["youtube", "booster", "owner"].includes(value.benefit)) return null;
    if (!Number.isSafeInteger(value.iat) || !Number.isSafeInteger(value.exp) || value.iat > now + 60000 || value.exp <= now || value.exp <= value.iat || value.exp - value.iat > 7 * 86400000) return null;
    return value;
  } catch { return null; }
}
