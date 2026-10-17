// Images and avatars of the chat, kept while the launcher runs: switching
// rooms or reopening the chat no longer downloads them again.
import { invoke } from "@tauri-apps/api/core";

const MAX_IMAGES = 60; // Thumbnails reach 1 MB; avatars are small.
const images = new Map<string, Promise<string | null>>();
const avatars = new Map<string, Promise<string | null>>();

function cached(cache: Map<string, Promise<string | null>>, key: string, load: () => Promise<string | null>, limit = Infinity) {
  const known = cache.get(key);
  if (known) { cache.delete(key); cache.set(key, known); return known; } // Most recent last.
  const request = load().catch(error => { cache.delete(key); throw error; }); // Failures are tried again later.
  cache.set(key, request);
  while (cache.size > limit) cache.delete(cache.keys().next().value!);
  return request;
}
export const matrixImage = (mxc: string) => cached(images, mxc, () => invoke<string | null>("matrix_chat_image", { mxc }), MAX_IMAGES);
// Animated GIFs and short videos (Discord's looping GIFs arrive as MP4 through the
// bridge) are whole files, up to 8 and 12 MB: fewer are kept.
const MAX_MOTION = 16;
const motion = new Map<string, Promise<string | null>>();
export const matrixAnimatedImage = (mxc: string) => cached(motion, "gif:" + mxc, () => invoke<string | null>("matrix_chat_image", { mxc, animated: true }), MAX_MOTION);
export const matrixVideo = (mxc: string) => cached(motion, "video:" + mxc, () => invoke<string | null>("matrix_chat_video", { mxc }), MAX_MOTION);
/** The media behind a GIF link (Tenor, Giphy, Discord, Imgur), or null. */
export interface GifMedia { src: string; video: boolean }
const gifs = new Map<string, Promise<GifMedia | null>>();
export function matrixGifMedia(url: string): Promise<GifMedia | null> {
  const known = gifs.get(url);
  if (known) return known;
  const request = invoke<GifMedia | null>("matrix_gif_media", { url }).catch(() => { gifs.delete(url); return null; });
  gifs.set(url, request);
  while (gifs.size > 64) gifs.delete(gifs.keys().next().value!);
  return request;
}
/** Links that may lead to a GIF: only known GIF services are asked about. */
export const GIF_LINK = /^https:\/\/(?:(?:www\.|media\d?\.|c\.)?tenor\.com|(?:www\.|i\.|media\d?\.)?giphy\.com|cdn\.discordapp\.com|media\.discordapp\.net|i\.imgur\.com)\//i;
export const matrixAvatar = (user: string) => cached(avatars, user, () => invoke<string | null>("matrix_chat_avatar", { user }));
/** A changed profile picture shows up without restarting the launcher. */
export function forgetMatrixAvatar(user: string) { avatars.delete(user); }
window.addEventListener("peligamesMatrixSignedOut", () => { images.clear(); avatars.clear(); motion.clear(); });
