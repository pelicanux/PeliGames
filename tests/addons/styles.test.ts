import {expect, test} from "bun:test";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {createAddonStyles} from "../../src/modules/addons/styles";

test("styles are literal text, bounded and removed with their owning session", async () => {
  const attached = new Set<object>();
  const document = {
    createElement: () => {const element = {dataset: {}, textContent: "", remove: () => {attached.delete(element)}}; return element},
    head: {appendChild: (element: object) => {attached.add(element)}},
  };
  const scope = new ResourceScope(() => {});
  const styles = createAddonStyles(scope, "example", document as unknown as Document);
  styles.add(".example { color: red }");
  expect(attached.size).toBe(1);
  const element = [...attached][0] as {dataset: {addon: string}; textContent: string};
  expect(element.textContent).toBe(".example { color: red }");expect(element.dataset.addon).toBe("example");
  expect(() => styles.add("x".repeat(512 * 1024))).toThrow();
  await scope.close();expect(attached.size).toBe(0);
  expect(() => styles.add(".example{}")).toThrow();
});
