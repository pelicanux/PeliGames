// Ponte de relatórios do PeliGames: recebe o relatório do programa e o publica
// como postagem no fórum do Discord pelo webhook, que fica só aqui (segredo
// DISCORD_WEBHOOK) e nunca dentro do PeliGames.
const MAX_REPORT_BYTES = 9_500_000;
const HEADER = "PELIGAMES — RELATÓRIO";

function reply(body, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json; charset=utf-8" } });
}

// Texto curto vindo do programa: sem caracteres de controle e com tamanho limitado.
function clean(value, limit) {
  return (typeof value === "string" ? value : "").replace(/[\u0000-\u0009\u000b-\u001f\u007f]/g, "").trim().slice(0, limit);
}

async function handle(request, env) {
    if (request.method !== "POST" || new URL(request.url).pathname !== "/report") {
      return reply({ error: "Endereço inválido." }, 404);
    }
    if (!env.DISCORD_WEBHOOK) return reply({ error: "Ponte sem webhook configurado." }, 503);
    if (env.LIMITER) {
      const { success } = await env.LIMITER.limit({ key: request.headers.get("CF-Connecting-IP") || "desconhecido" });
      if (!success) return reply({ error: "Muitos relatórios em pouco tempo. Tente de novo em alguns minutos." }, 429);
    }
    if (Number(request.headers.get("content-length") || 0) > MAX_REPORT_BYTES + 32_000) {
      return reply({ error: "Relatório grande demais." }, 413);
    }
    let body;
    try { body = await request.json(); } catch { return reply({ error: "Envio inválido." }, 400); }
    const report = typeof body.report === "string" ? body.report : "";
    // Só relatórios gerados pelo PeliGames.
    if (!report.startsWith(HEADER) || new TextEncoder().encode(report).length > MAX_REPORT_BYTES) {
      return reply({ error: "Relatório inválido." }, 400);
    }
    const title = clean(body.title, 100) || "Relatório do PeliGames";
    // One forum post per game: later reports of the same game go into it.
    const game = clean((report.match(/^Jogo: (.*)$/m) || [])[1], 100) || "Outros";
    const key = `game:${game.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim()}`;
    const description = clean(body.description, 1500);
    const discord = clean(body.discord, 40);
    const version = clean(body.version, 64);
    // Official build or not, as the launcher checked it (support sees it at a glance).
    const build = clean((report.match(/^Versão: (.*)$/m) || [])[1], 64);
    const content = [
      `**${title}**`,
      description || "Sem descrição.",
      "",
      `Versão: ${version || "não informada"}${build ? ` (${build})` : ""}${discord ? ` · Discord: ${discord}` : ""}`,
    ].join("\n").slice(0, 2000);
    const payload = {
      content,
      allowed_mentions: { parse: [] },
      attachments: [{ id: 0, filename: "relatorio-peligames.txt" }],
    };
    // The game's own object posts it: one game at a time, with an index read right after it is written.
    const index = env.GAME_THREADS.get(env.GAME_THREADS.idFromName(key));
    return index.fetch("https://relay/post", { method: "POST", body: JSON.stringify({ game, payload, report }) });
}

function send(env, payload, report, thread, game) {
  // The secret may carry spaces or a line break from pasting.
  const hook = new URL(String(env.DISCORD_WEBHOOK).trim());
  hook.searchParams.set("wait", "true");
  const body = { ...payload };
  if (thread) hook.searchParams.set("thread_id", thread);
  else {
    body.thread_name = game;
    if (env.FORUM_TAG_ID) body.applied_tags = [env.FORUM_TAG_ID];
  }
  const form = new FormData();
  form.append("payload_json", JSON.stringify(body));
  form.append("files[0]", new Blob([report], { type: "text/plain; charset=utf-8" }), "relatorio-peligames.txt");
  return fetch(hook, { method: "POST", body: form });
}

// Index of one game's forum post. Reports of the same game wait in line here,
// so the first one creates the post and the next ones join it.
export class GameThreads {
  constructor(ctx, env) {
    this.ctx = ctx;
    this.env = env;
    this.queue = Promise.resolve();
  }

  async fetch(request) {
    const job = await request.json();
    const run = this.queue.then(() => this.post(job));
    this.queue = run.catch(() => {});
    return run;
  }

  async post({ game, payload, report }) {
    try {
      const known = await this.ctx.storage.get("thread");
      let response = await send(this.env, payload, report, known, game);
      if (known && !response.ok) {
        // The game's post was deleted, locked or is unreachable: start a new one.
        console.error("Postagem do jogo indisponível", response.status, (await response.text()).slice(0, 300));
        response = await send(this.env, payload, report, null, game);
      }
      if (!response.ok) {
        console.error("Discord", response.status, (await response.text()).slice(0, 500));
        return reply({ error: `O Discord recusou o relatório (${response.status}).` }, 502);
      }
      const message = await response.json();
      if (message.channel_id && message.channel_id !== known) await this.ctx.storage.put("thread", message.channel_id);
      const url = this.env.GUILD_ID && message.channel_id ? `https://discord.com/channels/${this.env.GUILD_ID}/${message.channel_id}/${message.id}` : null;
      return reply({ ok: true, url });
    } catch (error) {
      console.error("Falha ao publicar:", error && error.stack ? error.stack : error);
      return reply({ error: "Falha interna da ponte de relatórios. Tente de novo mais tarde." }, 500);
    }
  }
}

export default {
  async fetch(request, env) {
    try {
      return await handle(request, env);
    } catch (error) {
      // Details only in the private log (wrangler tail): they may include the webhook address.
      console.error("Falha na ponte:", error && error.stack ? error.stack : error);
      return reply({ error: "Falha interna da ponte de relatórios. Tente de novo mais tarde." }, 500);
    }
  },
};
