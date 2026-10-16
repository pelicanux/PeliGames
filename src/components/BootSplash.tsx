/** The start screen of index.html (styled there, so it shows before any script):
 * used again while the launcher's code loads, so opening never shows an empty,
 * transparent window between the two. */
export function BootSplash({ message }: { message?: string }) {
  return <div className="boot-splash" role="status" aria-label={message || "PeliGames"}>
    <img src="/peligames.svg" alt="" />
    {message ? <p className="boot-splash-message">{message}</p> : <i />}
  </div>;
}
