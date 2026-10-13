import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ModalSurface } from "./ModalSurface";
import { MatrixIcon } from "./MatrixIcon";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { openMatrixCommunity, MATRIX_COMMUNITY_ID } from "../services/matrixCommunity";
import { displayName, loadOlderMessages, mentionsMe, moderation, refreshMatrixStatus, reloadMatrixRooms, useMatrixChat, type ChatMessage } from "../services/matrixChat";
import { notifyError } from "../services/notifications";
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
  const { language } = useI18n(); const pt = language === "pt";
  const preview = "__PELI_UI_PREVIEW__" in window;
  const chat = useMatrixChat();
  const rooms = chat.roomList.length ? chat.roomList : [{ room_id: MATRIX_COMMUNITY_ID, name: "PeliGames" }];
  const [room, setRoom] = useState(MATRIX_COMMUNITY_ID);
  const current = chat.rooms[room];
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
  // The parent passes a new function on every render; the effects below must not restart.
  const close = useRef(onClose); close.current = onClose;
  const fail = (e: unknown, fallback: string) => {
    const message = typeof e === "string" ? e : e instanceof Error ? e.message : fallback;
    setError(message); notifyError("Matrix", message);
  };
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
  const messages = (current?.messages ?? []).filter(message => !chat.ignored.includes(message.sender));
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
    const value = text.trim(); if (!value || busy) return;
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
      <div className="matrix-terminal-status"><span className="matrix-terminal-dot"/>{chat.userId ? displayName(chat.userId, chat.names) : pt ? "SESSÃO DESCONECTADA" : "DISCONNECTED SESSION"}<small>{preview ? pt ? "DEMONSTRAÇÃO · SEM LOGIN OU ENVIO REAL" : "DEMO · NO REAL SIGN-IN OR MESSAGES" : pt ? "CHAT DA COMUNIDADE" : "COMMUNITY CHAT"}</small></div>
      {!chat.userId ? <div className="matrix-terminal-welcome"><MatrixIcon/><h3>{pt ? "Conecte-se à comunidade" : "Connect to the community"}</h3><p>{pt ? "Converse sobre jogos, mods e Linux num chat com cara de terminal. Entre com sua conta Matrix pelo navegador." : "Talk about games, mods and Linux in a terminal-style chat. Sign in with your Matrix account through the browser."}</p><button className="btn btn-primary" disabled={busy || chat.pending} onClick={() => void run("connect_matrix_chat").then(() => refreshMatrixStatus())}><MatrixIcon/>{chat.pending ? pt ? "Aguardando login…" : "Waiting for sign-in…" : pt ? "Entrar com Matrix" : "Sign in with Matrix"}</button><p>{chat.pending ? pt ? "Autorize no navegador e volte aqui. O login expira em três minutos." : "Authorize in your browser and return here. Sign-in expires in three minutes." : pt ? "Sua senha não fica salva no launcher." : "Your password is not saved in the launcher."}</p></div> : <div className="matrix-terminal-layout">
        <aside className="matrix-terminal-rooms">
          {chat.ready && !communityJoined && <button className="matrix-terminal-join" disabled={busy} onClick={() => void join(MATRIX_COMMUNITY_ID)}>{pt ? "+ Entrar na comunidade" : "+ Join the community"}</button>}
          <h3>{pt ? "# SALAS" : "# ROOMS"}</h3>
          {rooms.map(item => <button key={item.room_id} aria-current={room === item.room_id ? "page" : undefined} onClick={() => setRoom(item.room_id)}># {item.name || "PeliGames"}</button>)}
          {chat.ignored.length > 0 && <><h3>{pt ? "# IGNORADOS" : "# IGNORED"}</h3>{chat.ignored.map(user => <button key={user} className="matrix-terminal-ignored" disabled={busy} title={pt ? "Voltar a mostrar" : "Show again"} onClick={() => void run("matrix_chat_ignore", { user, ignore: false })}>{displayName(user, chat.names)} ✕</button>)}</>}
          <button className="matrix-terminal-disconnect" disabled={busy} onClick={() => void run("disconnect_matrix_chat").then(() => refreshMatrixStatus())}>{pt ? "Desconectar" : "Disconnect"}</button>
        </aside>
        <section className="matrix-terminal-room"><header><strong># {roomName}</strong>{chat.ready && !current?.joined && <button className="btn btn-secondary" disabled={busy} onClick={() => void join(room)}>{pt ? "Entrar na sala" : "Join room"}</button>}</header>
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
              <button className="btn btn-primary" disabled={busy || current?.encrypted || !current?.joined || !text.trim()} type="submit"><MenuIcon name="play"/>{pt ? "Enviar" : "Send"}</button></div>
          </form>
        </section>
      </div>}
      {error && <p className="matrix-terminal-error" role="alert">{error}</p>}
      <footer className="matrix-terminal-footer"><span>{chat.userId ? chat.ready ? pt ? "Matrix · comunidade conectada" : "Matrix · community connected" : pt ? "Matrix · sincronizando…" : "Matrix · syncing…" : pt ? "Matrix · comunidade PeliGames" : "Matrix · PeliGames community"}</span><button className="btn btn-secondary" onClick={() => void openMatrixCommunity().catch(e => fail(e, pt ? "Não foi possível abrir o Element." : "Could not open Element."))}>{pt ? "Abrir no Element" : "Open in Element"}</button></footer>
    </ModalSurface>
  </div>,document.body);
}
