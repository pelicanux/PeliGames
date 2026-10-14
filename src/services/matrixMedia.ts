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
export const matrixAvatar = (user: string) => cached(avatars, user, () => invoke<string | null>("matrix_chat_avatar", { user }));
/** A changed profile picture shows up without restarting the launcher. */
export function forgetMatrixAvatar(user: string) { avatars.delete(user); }
window.addEventListener("peligamesMatrixSignedOut", () => { images.clear(); avatars.clear(); });
