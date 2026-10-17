import { useEffect } from "react";

/** The start screen of index.html (styled there, so it shows before any script):
 * used again while the launcher's code loads, so opening never shows an empty,
 * transparent window between the two. */
export function BootSplash({ message }: { message?: string }) {
  return <div className="boot-splash" role="status" aria-label={message || "PeliGames"}>
    <img src="/peligames.svg" alt="" />
    {message ? <p className="boot-splash-message">{message}</p> : <i />}
  </div>;
}

let finished: Promise<void> | null = null;
/** Fades out index.html's start screen, which stays above everything until the
 * interface has drawn its first frames; resolves once it is gone. */
export function finishBootSplash(): Promise<void> {
  if (finished) return finished;
  const splash = document.getElementById("boot-splash");
  if (!splash) return finished = Promise.resolve();
  return finished = new Promise(resolve => {
    // Two frames for the first paint, then a short margin for images and layers.
    requestAnimationFrame(() => requestAnimationFrame(() => window.setTimeout(() => {
      splash.classList.add("is-done");
      const remove = () => { splash.remove(); resolve(); };
      splash.addEventListener("transitionend", remove, { once: true });
      window.setTimeout(remove, 450);
    }, 220)));
  });
}
/** Resolves when the start screen has gone (at once where there is none). */
export function bootSplashGone(): Promise<void> {
  return document.getElementById("boot-splash") ? finished ?? new Promise(resolve => {
    const wait = () => { if (!document.getElementById("boot-splash")) resolve(); else window.setTimeout(wait, 100); };
    wait();
  }) : Promise.resolve();
}
/** Mounted together with a loaded module: its first frames are drawn, so the
 * start screen can go. */
export function SplashDone() {
  useEffect(() => { void finishBootSplash(); }, []);
  return null;
}
