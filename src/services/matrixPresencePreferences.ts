import { useSyncExternalStore } from "react";

export type MatrixPresenceChoice = "online" | "idle" | "dnd" | "invisible";
export type MatrixMemberPresence = "online" | "idle" | "dnd" | "offline";
const choices = new Map<string, MatrixPresenceChoice>();
const listeners = new Set<() => void>();
export const validMatrixPresence = (value: unknown): value is MatrixPresenceChoice => ["online", "idle", "dnd", "invisible"].includes(value as string);
export function matrixPresenceChoice(user: string | null): MatrixPresenceChoice {
  if (!user) return "online";
  if (!choices.has(user)) {
    let saved: unknown;
    try { saved = localStorage.getItem(`peligames.matrix.presence.${user}`); } catch { /* Session-only preference. */ }
    choices.set(user, validMatrixPresence(saved) ? saved : "online");
  }
  return choices.get(user)!;
}
export function setMatrixPresenceChoice(user: string, choice: MatrixPresenceChoice) {
  if (!validMatrixPresence(choice)) return;
  choices.set(user, choice);
  try { localStorage.setItem(`peligames.matrix.presence.${user}`, choice); } catch { /* Session-only preference. */ }
  listeners.forEach(listener => listener());
}
export function useMatrixPresenceChoice(user: string | null) {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => matrixPresenceChoice(user));
}
export const matrixWirePresence = (choice: MatrixPresenceChoice) => choice === "invisible" ? "offline" : choice === "online" ? "online" : "unavailable";
