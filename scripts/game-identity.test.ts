import { expect, test } from "bun:test";
import { gameViewKey } from "../src/services/gameIdentity";

test("launcher entries and mod targets never share a view identity", () => {
  const game = { name: "Same game", path: "/games/Game/game.exe", launcher: "PeliGames" };
  expect(gameViewKey(game)).not.toBe(gameViewKey({ ...game, library_view: "all" }));
  expect(gameViewKey(game)).toBe(gameViewKey({ ...game, library_view: "own" }));
  expect(gameViewKey({ ...game, launcher: "Steam", library_view: "all" })).not.toBe(
    gameViewKey({ ...game, library_view: "all" }));
  expect(gameViewKey({ ...game, path: "peligames:first" })).not.toBe(
    gameViewKey({ ...game, path: "peligames:second" }));
});
