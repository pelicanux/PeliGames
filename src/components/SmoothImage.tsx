import { useLayoutEffect, useRef, useState, type ImgHTMLAttributes, type SyntheticEvent } from "react";

/** An image that never blinks: one already in memory shows in the first frame,
 * one still loading fades in once it is decoded instead of popping in. */
export function SmoothImage({ className, onLoad, onError, src, ...props }: ImgHTMLAttributes<HTMLImageElement>) {
  const image = useRef<HTMLImageElement>(null);
  const [shown, setShown] = useState(false);
  // Before the first paint: a cached image is visible at once, without a fade.
  useLayoutEffect(() => {
    const element = image.current;
    setShown(Boolean(element?.complete && element.naturalWidth));
  }, [src]);
  const reveal = (event: SyntheticEvent<HTMLImageElement>) => {
    const element = event.currentTarget;
    void element.decode().catch(() => {}).finally(() => { if (element.isConnected && element.getAttribute("src") === src) setShown(true); });
    onLoad?.(event);
  };
  return <img ref={image} {...props} src={src} className={`smooth-image${shown ? " is-shown" : ""}${className ? ` ${className}` : ""}`}
    onLoad={reveal} onError={event => { setShown(true); onError?.(event); }} />;
}
