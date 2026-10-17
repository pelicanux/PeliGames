import { useLayoutEffect, useRef, useState, type ImgHTMLAttributes, type SyntheticEvent } from "react";

/** An image that never blinks: one already in memory shows in the first frame,
 * one still loading fades in once it is decoded instead of popping in. */
export function SmoothImage({ className, onLoad, onError, src, ...props }: ImgHTMLAttributes<HTMLImageElement>) {
  const image = useRef<HTMLImageElement>(null);
  const [displayedSrc, setDisplayedSrc] = useState(src);
  const [shown, setShown] = useState(false);
  // Before the first paint: a cached image is visible at once, without a fade.
  useLayoutEffect(() => {
    const element = image.current;
    setShown(previous => previous || Boolean(element?.complete && element.naturalWidth));
  }, [displayedSrc]);
  useLayoutEffect(() => {
    if (src === displayedSrc) return;
    if (!src || !shown) { setDisplayedSrc(src); if (!src) setShown(false); return; }
    // A scanner/cache may replace a remote URL with its local copy. Keep the
    // already painted artwork until the replacement is decoded, never fade it
    // back to zero just because its address changed.
    let active = true;
    const candidate = new Image();
    const publish = () => { if (active) setDisplayedSrc(src); };
    candidate.onload = () => { void candidate.decode().catch(() => {}).then(publish); };
    // Let the actual element deliver React's normal error event to the caller.
    candidate.onerror = publish;
    candidate.src = src;
    return () => { active = false; candidate.onload = null; candidate.onerror = null; };
  }, [src, displayedSrc, shown]);
  const reveal = (event: SyntheticEvent<HTMLImageElement>) => {
    const element = event.currentTarget;
    void element.decode().catch(() => {}).finally(() => { if (element.isConnected && element.getAttribute("src") === displayedSrc) setShown(true); });
    if (displayedSrc === src) onLoad?.(event);
  };
  return <img ref={image} {...props} src={displayedSrc} className={`smooth-image${shown ? " is-shown" : ""}${className ? ` ${className}` : ""}`}
    onLoad={reveal} onError={event => { if (displayedSrc === src) { setShown(true); onError?.(event); } }} />;
}
