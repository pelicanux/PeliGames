import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ModalSurface } from "./ModalSurface";
import { MatrixIcon } from "./MatrixIcon";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { openMatrixCommunity, MATRIX_COMMUNITY_ID, MATRIX_PREMIUM_ROOM_ID } from "../services/matrixCommunity";
import { displayName, loadOlderMessages, mentionsMe, moderation, refreshMatrixRooms, refreshMatrixStatus, reloadMatrixRooms, useMatrixChat, type ChatMessage } from "../services/matrixChat";
import { notifyError } from "../services/notifications";
import { useDiscordBenefits } from "../theme/useDiscordBenefits";
import "./matrixChat.css";

function ChatImage({ mxc }: {mxc:string}) {
  const [image,setImage] = useState<string | null>(null);
  useEffect(()=>{let active=true;void invoke<string|null>("matrix_chat_image",{mxc}).then(value=>{if(active)setImage(value);}).catch(()=>{});return()=>{active=false;};},[mxc]);
  return image ? <img className="matrix-message-image" src={image} alt="Matrix"/> : null;
}
const userColor = (user: string) => { let hash = 0; for (const c of user) hash = (hash * 31 + c.charCodeAt(0)) >>> 0; return `hsl(${hash % 360} 48% 64%)`; };
const LINK = /(https?:\/\/[^\s<>"']+[^\s<>"'.,;:!?)\]])/g;
/** Message text with web links opened in the system browser; never HTML. */
function Linked({ text, onError }: { text: string; onError: (error: unknown) => void }) {
  const parts: ReactNode[] = text.split(LINK).map((part, index) => index % 2
    ? <a key={index} href={part} onClick={event => { event.preventDefault(); void openUrl(part).catch(onError); }}>{part}</a>
    : part);
  return <>{parts}</>;
}
export function MatrixChatPanel({ onClose }: { onClose: () => void }) {
  const screenClipId = `matrix-screen-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const screenShape = "M .10 .04 C .30 .013333 .70 .013333 .90 .04 C .953333 .047111 .965333 .08 .972 .14 C .992 .32 .992 .68 .972 .86 C .965333 .92 .953333 .952889 .90 .96 C .70 .986667 .30 .986667 .10 .96 C .046667 .952889 .034667 .92 .028 .86 C .008 .68 .008 .32 .028 .14 C .034667 .08 .046667 .047111 .10 .04 Z";
  const { language } = useI18n(); const pt = language === "pt";
  const preview = "__PELI_UI_PREVIEW__" in window;
  const chat = useMatrixChat();
  const rooms = chat.roomList.length ? chat.roomList : [{ room_id: MATRIX_COMMUNITY_ID, name: "PeliGames" }];
  const [room, setRoom] = useState(MATRIX_COMMUNITY_ID);
  const current = chat.rooms[room];
  const { exclusiveEffects, benefitsChecked } = useDiscordBenefits();
  const [premium, setPremium] = useState<{user:string; phase:"checking" | "ready" | "error"; error?:string} | null>(null);
  const [premiumAttempt, setPremiumAttempt] = useState(0);
  const premiumSelected = room === MATRIX_PREMIUM_ROOM_ID;
  const premiumReady = exclusiveEffects && premium?.user === chat.userId && premium.phase === "ready";
  useEffect(() => {
    let active = true;
    if (!chat.userId || !benefitsChecked || !exclusiveEffects) { setPremium(null); return; }
    const user = chat.userId;
    setPremium({user,phase:"checking"});
    void invoke<{granted:boolean;room_id:string;user_id:string}>("matrix_chat_premium_access").then(async grant => {
      if (!active) return;
      if (!grant.granted || grant.room_id !== MATRIX_PREMIUM_ROOM_ID || grant.user_id !== user) throw new Error("Autorização premium inválida.");
      await reloadMatrixRooms();
      if (active) setPremium({user,phase:"ready"});
    }).catch(error => { if (active) setPremium({user,phase:"error",error:String(error)}); });
    return () => { active = false; };
  }, [chat.userId, benefitsChecked, exclusiveEffects, premiumAttempt]);
  const [avatars, setAvatars] = useState<Record<string,string | null>>({});
  const avatarLookups = useRef(new Set<string>());
  const [text, setText] = useState(""), [error, setError] = useState(""), [busy, setBusy] = useState(false);
  const [replyTo, setReplyTo] = useState<ChatMessage | null>(null);
  // An action on one message that asks for a reason and confirmation first.
  type Pending = { id: string; kind: "report" | "redact" | "kick" | "ban" };
  const [pending, setPending] = useState<Pending | null>(null), [reason, setReason] = useState("");
  const ask = (id: string, kind: Pending["kind"]) => { setPending(pending?.id === id && pending.kind === kind ? null : { id, kind }); setReason(""); };
  const fileInput = useRef<HTMLInputElement>(null);
  const frame = useRef<HTMLDivElement>(null), log = useRef<HTMLDivElement>(null), input = useRef<HTMLTextAreaElement>(null);
  const atBottom = useRef(true), olderFrom = useRef<number | null>(null);
  const actionLock = useRef(false);
  const [refreshingRooms, setRefreshingRooms] = useState(false);
  const roomRefreshLock = useRef(false);
  // The parent passes a new function on every render; the effects below must not restart.
  const close = useRef(onClose); close.current = onClose;
  const fail = (e: unknown, fallback: string) => {
    const message = typeof e === "string" ? e : e instanceof Error ? e.message : fallback;
    setError(message); notifyError("Matrix", message);
  };
  const refreshRooms = async () => {
    if (roomRefreshLock.current) return;
    roomRefreshLock.current = true; setRefreshingRooms(true);
    try { await refreshMatrixRooms(); }
    catch (e) { fail(e, pt ? "Não foi possível atualizar as salas." : "Could not refresh rooms."); }
    finally { roomRefreshLock.current = false; setRefreshingRooms(false); }
  };
  useEffect(() => {
    if (chat.userId) void refreshRooms();
  }, [chat.userId]);
  useEffect(() => {
    void refreshMatrixStatus();
    const oldFocus = document.activeElement; frame.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); close.current(); }
      if (event.key === "Tab") {
        const controls = frame.current?.querySelectorAll<HTMLElement>("button:not(:disabled),textarea,input:not([type=file]),a[href]");
        if (!controls?.length) return;
        const first = controls[0], last = controls[controls.length-1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown",keyboard,true);
    return () => { window.removeEventListener("keydown",keyboard,true); if (oldFocus instanceof HTMLElement && oldFocus.isConnected) oldFocus.focus(); };
  }, []);
  useEffect(() => { if (chat.error) setError(chat.error); }, [chat.error]);
  // Keep the selected room valid once the community's room list arrives.
  useEffect(() => { if (!rooms.some(item => item.room_id === room)) setRoom(rooms[0].room_id); }, [rooms, room]);
  useEffect(() => { setReplyTo(null); setPending(null); atBottom.current = true; }, [room]);
  const messages = (premiumSelected && !premiumReady ? [] : current?.messages ?? []).filter(message => !chat.ignored.includes(message.sender));
  useEffect(() => {
    for (const sender of new Set(messages.map(message => message.sender))) {
      if (avatarLookups.current.has(sender)) continue;
      avatarLookups.current.add(sender);
      void invoke<string | null>("matrix_chat_avatar",{user:sender}).then(avatar=>{if (frame.current?.isConnected) setAvatars(previous=>({...previous,[sender]:avatar}));}).catch(()=>{});
    }
  }, [messages]);
  // New messages follow the end only while the reader is already there; older
  // history keeps the message in view where it was.
  useLayoutEffect(() => {
    const element = log.current; if (!element) return;
    if (olderFrom.current !== null) { element.scrollTop += element.scrollHeight - olderFrom.current; olderFrom.current = null; }
    else if (atBottom.current) element.scrollTop = element.scrollHeight;
  }, [messages.length, room]);
  const run = async (command: string, args?: Record<string, unknown>) => {
    if (actionLock.current) return false; actionLock.current = true; setBusy(true); setError("");
    try { await invoke(command,args); return true; }
    catch (e) { fail(e, pt ? "Não foi possível concluir a operação." : "Could not complete the operation."); return false; }
    finally { actionLock.current = false; setBusy(false); }
  };
  const send = async () => {
    const value = text.trim(); if (!value || busy || (premiumSelected && !premiumReady)) return;
    const emote = /^\/me\s+/i.test(value);
    atBottom.current = true;
    if (await run("matrix_chat_send", { room, text: emote ? value.replace(/^\/me\s+/i, "") : value, replyTo: replyTo?.id ?? null, replySender: replyTo?.sender ?? null, emote })) { setText(""); setReplyTo(null); }
  };
  const join = async (target: string) => { if (await run("matrix_chat_join", { room: target })) await reloadMatrixRooms(); };
  const older = async () => {
    olderFrom.current = log.current?.scrollHeight ?? null;
    try { await loadOlderMessages(room); } catch (e) { olderFrom.current = null; fail(e, pt ? "Não foi possível carregar mensagens anteriores." : "Could not load older messages."); }
  };
  const upload = async (file: File) => {
    if (premiumSelected && !premiumReady) return;
    if (file.size > 5*1024*1024) { setError(pt ? "O arquivo deve ter até 5 MB." : "File must be at most 5 MB."); return; }
    const mime = /\.(txt|log)$/i.test(file.name) ? "text/plain" : file.type;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      atBottom.current = true;
      if (actionLock.current) return; actionLock.current = true; setBusy(true); setError("");
      await invoke("matrix_chat_upload", bytes, { headers: { "x-matrix-room": encodeURIComponent(room), "x-matrix-name": encodeURIComponent(file.name), "x-matrix-mime": encodeURIComponent(mime) } });
    } catch (e) { fail(e, pt ? "Não foi possível enviar o arquivo." : "Could not send the file."); }
    finally { actionLock.current = false; setBusy(false); }
  };
  const confirm = async (message: ChatMessage, kind: Pending["kind"]) => {
    const done = kind === "report"
      ? await run("matrix_chat_report", { room, event: message.id, reason })
      : await run("matrix_chat_moderate", { room, action: kind, target: kind === "redact" ? message.id : message.sender, reason });
    if (done) setPending(null);
  };
  const actionText = {
    report: { title: pt ? "Denunciar aos moderadores" : "Report to moderators", button: pt ? "Enviar denúncia" : "Send report" },
    redact: { title: pt ? "Apagar esta mensagem para todos" : "Remove this message for everyone", button: pt ? "Apagar" : "Remove" },
    kick: { title: pt ? "Remover a pessoa da sala (ela pode voltar)" : "Remove the person from the room (they can come back)", button: pt ? "Expulsar" : "Kick" },
    ban: { title: pt ? "Banir a pessoa desta sala" : "Ban the person from this room", button: pt ? "Banir" : "Ban" },
  };
  const byId = new Map(messages.map(message => [message.id, message]));
  const communityJoined = chat.rooms[MATRIX_COMMUNITY_ID]?.joined;
  const roomName = rooms.find(item=>item.room_id === room)?.name || "PeliGames";
  return createPortal(<div className="modal-overlay matrix-chat-overlay" onMouseDown={event => { if (event.target === event.currentTarget) close.current(); }}>
    <ModalSurface ref={frame} className="modal-content dialog-glass matrix-chat" role="dialog" aria-modal="true" aria-label="Matrix · PeliGames" onDismiss={() => close.current()}
      header={<h2><MatrixIcon/>Matrix <span className="matrix-terminal-tag">// PeliGames</span></h2>}>
      <div className="matrix-terminal-frame" style={{clipPath:`url(#${screenClipId})`}}>
      <svg width="0" height="0" className="matrix-crt-definitions" aria-hidden="true" focusable="false"><defs><clipPath id={screenClipId} clipPathUnits="objectBoundingBox"><path d={screenShape}/></clipPath></defs></svg>
      <svg className="matrix-crt-outline" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true" focusable="false"><path d={screenShape} fill="none" vectorEffect="non-scaling-stroke"/></svg>
      <div className="matrix-crt-screen">
      <div className="matrix-terminal-status"><span className="matrix-terminal-dot"/>{chat.userId ? displayName(chat.userId, chat.names) : pt ? "SESSÃO DESCONECTADA" : "DISCONNECTED SESSION"}<small>{preview ? pt ? "DEMONSTRAÇÃO · SEM LOGIN OU ENVIO REAL" : "DEMO · NO REAL SIGN-IN OR MESSAGES" : pt ? "CHAT DA COMUNIDADE" : "COMMUNITY CHAT"}</small></div>
      {!chat.userId ? <div className="matrix-terminal-welcome"><MatrixIcon/><h3>{pt ? "Conecte-se à comunidade" : "Connect to the community"}</h3><p>{pt ? "Converse sobre jogos, mods e Linux num chat com cara de terminal. Entre com sua conta Matrix pelo navegador." : "Talk about games, mods and Linux in a terminal-style chat. Sign in with your Matrix account through the browser."}</p><button className="btn btn-primary" disabled={busy || chat.pending} onClick={() => void run("connect_matrix_chat").then(() => refreshMatrixStatus())}><MatrixIcon/>{chat.pending ? pt ? "Aguardando login…" : "Waiting for sign-in…" : pt ? "Entrar com Matrix" : "Sign in with Matrix"}</button><p>{chat.pending ? pt ? "Autorize no navegador e volte aqui. O login expira em três minutos." : "Authorize in your browser and return here. Sign-in expires in three minutes." : pt ? "Sua senha não fica salva no launcher." : "Your password is not saved in the launcher."}</p></div> : <div className="matrix-terminal-layout">
        <aside className="matrix-terminal-rooms">
          {chat.ready && !communityJoined && <button className="matrix-terminal-join" disabled={busy} onClick={() => void join(MATRIX_COMMUNITY_ID)}>{pt ? "+ Entrar na comunidade" : "+ Join the community"}</button>}
          <div className="matrix-rooms-heading"><h3>{pt ? "# SALAS" : "# ROOMS"}</h3><button type="button" disabled={refreshingRooms} onClick={() => void refreshRooms()} aria-label={pt ? "Atualizar salas" : "Refresh rooms"} title={pt ? "Atualizar salas" : "Refresh rooms"}><MenuIcon name="repair"/></button></div>
          {preview && <small className="matrix-rooms-demo">{pt ? "Lista simulada na preview" : "Simulated preview rooms"}</small>}
          {rooms.map(item => <button key={item.room_id} aria-current={room === item.room_id ? "page" : undefined} onClick={() => setRoom(item.room_id)}># {item.name || "PeliGames"}{item.room_id === MATRIX_PREMIUM_ROOM_ID && !premiumReady ? " 🔒" : ""}</button>)}
          {chat.ignored.length > 0 && <><h3>{pt ? "# IGNORADOS" : "# IGNORED"}</h3>{chat.ignored.map(user => <button key={user} className="matrix-terminal-ignored" disabled={busy} title={pt ? "Voltar a mostrar" : "Show again"} onClick={() => void run("matrix_chat_ignore", { user, ignore: false })}>{displayName(user, chat.names)} ✕</button>)}</>}
          <button className="matrix-terminal-disconnect" disabled={busy} onClick={() => void run("disconnect_matrix_chat").then(() => refreshMatrixStatus())}>{pt ? "Desconectar" : "Disconnect"}</button>
        </aside>
        <section className="matrix-terminal-room"><header><strong># {roomName}</strong>{!premiumSelected && chat.ready && !current?.joined && <button className="btn btn-secondary" disabled={busy} onClick={() => void join(room)}>{pt ? "Entrar na sala" : "Join room"}</button>}</header>
          {premiumSelected && !premiumReady ? <div className="matrix-premium-gate" role="status">
            <h3>{pt ? "Suporte premium" : "Premium support"}</h3>
            <p>{!benefitsChecked || premium?.phase === "checking" ? pt ? "Verificando o Discord e liberando sua conta Matrix…" : "Checking Discord and granting Matrix access…" : !exclusiveEffects ? pt ? "Exclusivo para membros do YouTube e impulsionadores do servidor no Discord. Entre com Discord em Preferências → Conexões para liberar." : "For YouTube members and Discord server boosters. Sign in with Discord in Preferences → Connections to unlock." : premium?.error || (pt ? "Confirme sua conexão para acessar esta sala." : "Confirm your connection to access this room.")}</p>
            {exclusiveEffects && premium?.phase !== "checking" && <button type="button" className="btn btn-secondary" onClick={() => setPremiumAttempt(value=>value+1)}>{pt ? "Tentar novamente" : "Try again"}</button>}
          </div> : <>
          <div ref={log} className="matrix-terminal-messages" role="log" aria-live="polite" aria-relevant="additions" onScroll={event => { const element = event.currentTarget; atBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80; }}>
            {current?.encrypted ? <p>{pt ? "Esta sala usa criptografia. Abra o Element para conversar com segurança." : "This room uses encryption. Open Element to chat securely."}</p> : <>
              {current?.prevBatch && messages.length > 0 && <button className="matrix-terminal-older" disabled={current.loadingOlder} onClick={() => void older()}>{current.loadingOlder ? pt ? "> carregando…" : "> loading…" : pt ? "> carregar mensagens anteriores" : "> load older messages"}</button>}
              {messages.length ? messages.map(message => {
                const name = displayName(message.sender, chat.names), color = userColor(message.sender), own = message.sender === chat.userId;
                const replied = message.reply_to ? byId.get(message.reply_to) : undefined;
                const allowed = chat.userId ? moderation(current?.power, chat.userId, message.sender) : { redact: false, kick: false, ban: false };
                return <article key={message.id} className={`matrix-terminal-message${chat.userId && mentionsMe(message, chat.userId, chat.names) ? " is-mention" : ""}${message.msgtype === "m.notice" ? " is-notice" : ""}`}>
                  <div className="matrix-terminal-avatar" style={{color,borderColor:color}} aria-hidden="true">{avatars[message.sender] ? <img src={avatars[message.sender]!} alt=""/> : name.slice(0,2).toUpperCase()}</div>
                  <div>
                    <header><strong style={{color}} title={message.sender}>{name}</strong><time>{new Date(message.ts).toLocaleTimeString(pt ? "pt-BR" : "en-US",{hour:"2-digit",minute:"2-digit"})}</time>{message.edited && <small>{pt ? "(editada)" : "(edited)"}</small>}
                      <span className="matrix-message-actions">
                        <button type="button" onClick={() => { setReplyTo(message); input.current?.focus(); }}>{pt ? "responder" : "reply"}</button>
                        {!own && <button type="button" onClick={() => ask(message.id, "report")}>{pt ? "denunciar" : "report"}</button>}
                        {!own && <button type="button" disabled={busy} onClick={() => void run("matrix_chat_ignore", { user: message.sender, ignore: true })}>{pt ? "ignorar" : "ignore"}</button>}
                        {allowed.redact && <button type="button" onClick={() => ask(message.id, "redact")}>{pt ? "apagar" : "remove"}</button>}
                        {allowed.kick && <button type="button" className="is-moderation" onClick={() => ask(message.id, "kick")}>{pt ? "expulsar" : "kick"}</button>}
                        {allowed.ban && <button type="button" className="is-moderation" onClick={() => ask(message.id, "ban")}>{pt ? "banir" : "ban"}</button>}
                      </span>
                    </header>
                    {message.reply_to && <p className="matrix-message-reply">↳ {replied ? <><b>{displayName(replied.sender, chat.names)}</b>: {replied.body.slice(0, 120)}</> : pt ? "resposta a uma mensagem anterior" : "reply to an earlier message"}</p>}
                    <p>{message.msgtype === "m.emote" ? <>* <b style={{color}}>{name}</b> </> : null}{message.msgtype === "m.file" ? `[${pt ? "arquivo" : "file"}] ` : null}{message.msgtype === "m.image" ? null : <Linked text={message.body} onError={e => fail(e, pt ? "Não foi possível abrir o link." : "Could not open the link.")}/>}</p>
                    {message.msgtype === "m.image" && message.url && <ChatImage mxc={message.url}/>}
                    {pending?.id === message.id && <form className={`matrix-message-report${pending.kind === "kick" || pending.kind === "ban" ? " is-moderation" : ""}`} onSubmit={event => { event.preventDefault(); void confirm(message, pending.kind); }}>
                      <strong>{actionText[pending.kind].title}{pending.kind === "kick" || pending.kind === "ban" ? `: ${name}` : ""}</strong>
                      <input value={reason} maxLength={pending.kind === "report" ? 1000 : 500} autoFocus onChange={event => setReason(event.target.value)} placeholder={pt ? "Motivo (opcional)" : "Reason (optional)"}/>
                      <button type="submit" className="btn btn-secondary" disabled={busy}>{actionText[pending.kind].button}</button>
                      <button type="button" className="btn btn-secondary" onClick={() => setPending(null)}>{pt ? "Cancelar" : "Cancel"}</button>
                    </form>}
                  </div>
                </article>;
              }) : <p className="matrix-terminal-empty">{!chat.ready ? pt ? "> conectando…" : "> connecting…" : current?.joined ? pt ? "> Nenhuma mensagem ainda. Diga oi!" : "> No messages yet. Say hi!" : pt ? "> Entre na sala para ver as mensagens." : "> Join the room to see the messages."}</p>}
            </>}
          </div>
          <form className="matrix-terminal-compose" onSubmit={event=>{event.preventDefault();void send();}}>
            {replyTo ? <p className="matrix-compose-reply">↳ {pt ? "respondendo a" : "replying to"} <b>{displayName(replyTo.sender, chat.names)}</b>: {replyTo.body.slice(0, 80)} <button type="button" onClick={() => setReplyTo(null)} aria-label={pt ? "Cancelar resposta" : "Cancel reply"}>✕</button></p>
              : <label htmlFor="matrix-message">{pt ? "> MENSAGEM · Enter envia, Shift+Enter quebra a linha, /me para ação" : "> MESSAGE · Enter sends, Shift+Enter new line, /me for actions"}</label>}
            <div><input ref={fileInput} type="file" hidden accept="image/png,image/jpeg,image/gif,image/webp,.txt,.log" onChange={event=>{const file = event.target.files?.[0]; event.target.value = ""; if (file) void upload(file);}}/>
              <button type="button" className="btn btn-secondary" disabled={busy || current?.encrypted || !current?.joined} onClick={()=>fileInput.current?.click()} aria-label={pt ? "Enviar imagem ou log" : "Send image or log"}><MenuIcon name="plus"/></button>
              <textarea ref={input} id="matrix-message" value={text} maxLength={8000} disabled={busy || current?.encrypted || !current?.joined} onChange={event=>setText(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void send(); } }} placeholder={pt ? "Digite sua mensagem…" : "Type your message…"}/>
              <button className="matrix-send-button" aria-label={pt ? "Enviar mensagem · Enter" : "Send message · Enter"} disabled={busy || current?.encrypted || !current?.joined || !text.trim()} type="submit">
                <svg className="matrix-enter-key" width="72" height="82" viewBox="0 0 120 140" fill="none" aria-hidden="true" focusable="false">
                  <g stroke="currentColor" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round">
                    <path d="M20 8Q61 11 102 8L114 26V125Q114 132 107 132H43Q36 132 36 125V70H14Q8 70 8 64V27L20 8Z"/>
                    <path d="M20 8V51L48 55V114Q75 122 102 114V8M8 70L20 51M36 70L48 55M36 128L48 114M102 114L112 129"/>
                  </g>
                  <text x="68" y="43" textAnchor="middle" fill="currentColor">Enter</text>
                </svg>
              </button></div>
          </form>
          </>}
        </section>
      </div>}
      {error && <p className="matrix-terminal-error" role="alert">{error}</p>}
      <footer className="matrix-terminal-footer"><span>{chat.userId ? chat.ready ? pt ? "Matrix · comunidade conectada" : "Matrix · community connected" : pt ? "Matrix · sincronizando…" : "Matrix · syncing…" : pt ? "Matrix · comunidade PeliGames" : "Matrix · PeliGames community"}</span><button className="btn btn-secondary" onClick={() => void openMatrixCommunity().catch(e => fail(e, pt ? "Não foi possível abrir o Element." : "Could not open Element."))}>{pt ? "Abrir no Element" : "Open in Element"}</button></footer>
      </div>
      </div>
    </ModalSurface>
  </div>,document.body);
}
