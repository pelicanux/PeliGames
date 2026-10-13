// Public community address. No account credentials or access tokens belong here.
import { invoke } from "@tauri-apps/api/core";
export const MATRIX_COMMUNITY_ID = "!pNHMqrnxSAmcVFqrgf:matrix.org";
export const MATRIX_COMMUNITY_URL = `https://app.element.io/#/room/${MATRIX_COMMUNITY_ID}`;
export const MATRIX_COMMUNITY_LINK = `https://matrix.to/#/${MATRIX_COMMUNITY_ID}`;
export function openMatrixCommunity(): Promise<void> {
  return invoke("open_matrix_community");
}
