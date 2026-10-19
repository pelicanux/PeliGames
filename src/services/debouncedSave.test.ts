import { expect, test } from "bun:test";
import { DebouncedSave } from "./debouncedSave";

test("rapid edits collapse to the latest value and leaving the panel flushes it", async () => {
  const queue = new DebouncedSave(60_000);
  const saved: number[] = [];
  for (const n of [2, 3, 4]) queue.schedule(async () => { saved.push(n); });
  expect(saved).toEqual([]);
  await queue.flush();
  expect(saved).toEqual([4]);
  await queue.flush();
  expect(saved).toEqual([4]);
});

test("writes are serial and a pending edit survives an in-flight save", async () => {
  const queue = new DebouncedSave(60_000);
  const order: string[] = [];
  let finish!: () => void;
  queue.schedule(async () => { order.push("first:start"); await new Promise<void>(resolve => { finish = resolve; }); order.push("first:end"); });
  const active = queue.flush();
  queue.schedule(async () => { order.push("second"); });
  const leaving = queue.flush();
  expect(order).toEqual(["first:start"]);
  finish();
  await Promise.all([active, leaving]);
  expect(order).toEqual(["first:start", "first:end", "second"]);
});
