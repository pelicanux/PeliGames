// Run with: bun scripts/test-notification-feedback.ts
import assert from "node:assert/strict";
import { notifyError, notifyInfo, LAUNCHER_ERROR_EVENT, LAUNCHER_NOTICE_EVENT } from "../src/services/notifications";
const events: CustomEvent[] = [];
Object.assign(globalThis, { window: { dispatchEvent: (event: CustomEvent) => { events.push(event); return true; } } });
notifyInfo("Manutenção", "  Atalho criado.  ");
notifyError("Winecfg", new Error("Não foi possível abrir."));
notifyInfo("Manutenção", " ");
notifyError("Regedit", "");
assert.equal(events.length, 2);
assert.equal(events[0].type, LAUNCHER_NOTICE_EVENT);
assert.deepEqual(events[0].detail, { source: "Manutenção", message: "Atalho criado." });
assert.equal(events[1].type, LAUNCHER_ERROR_EVENT);
assert.deepEqual(events[1].detail, { source: "Winecfg", message: "Não foi possível abrir." });
console.log("Passed: informational and error feedback use distinct notification events, keep their source, normalize messages and ignore empty feedback.");
