import {resolve, dirname} from "node:path";
import {writeFile} from "node:fs/promises";

/** One ESM factory, with private module state per instance and host-owned React. */
export async function buildAddonUi(entrypoint: string): Promise<string> {
  const entry = resolve(entrypoint);
  const transpiler = new Bun.Transpiler({loader: "tsx", tsconfig: JSON.stringify({compilerOptions: {
    jsx: "react", jsxFactory: "React.createElement", jsxFragmentFactory: "React.Fragment",
  }})});
  const hooks = ["Children", "Fragment", "Suspense", "createContext", "createElement", "cloneElement", "isValidElement", "forwardRef", "memo", "lazy", "use", "useCallback", "useContext", "useDebugValue", "useDeferredValue", "useEffect", "useId", "useImperativeHandle", "useInsertionEffect", "useLayoutEffect", "useMemo", "useReducer", "useRef", "useState", "useSyncExternalStore", "useTransition", "startTransition"];
  const result = await Bun.build({
    entrypoints: ["peligames:ui-entry"],
    target: "browser", format: "iife", write: false,
    minify: {whitespace: true, identifiers: false, syntax: false},
    jsx: {runtime: "classic", factory: "React.createElement", fragment: "React.Fragment"},
    plugins: [{name: "peligames-host-ui", setup(builder) {
      builder.onResolve({filter: /^peligames:ui-entry$/}, () => ({path: "entry", namespace: "peligames"}));
      builder.onLoad({filter: /^entry$/, namespace: "peligames"}, () => ({
        loader: "js", resolveDir: dirname(entry),
        contents: `import {createAddon} from ${JSON.stringify(entry)}; __peligames_ui_instance__ = createAddon(__peligames_ui_host__);`,
      }));
      builder.onResolve({filter: /^react$/}, () => ({path: "react", namespace: "peligames"}));
      builder.onLoad({filter: /^react$/, namespace: "peligames"}, () => ({loader: "js", contents:
        `export default __peligames_ui_host__.React; export const {${hooks.join(",")}} = __peligames_ui_host__.React;`,
      }));
      builder.onResolve({filter: /^(react-dom|react\/jsx-runtime|react\/jsx-dev-runtime)(\/.*)?$/}, () => {
        throw new Error("Use host controls and classic JSX; add-ons must not bundle another renderer");
      });
      builder.onLoad({filter: /\.tsx$/}, async ({path}) => ({loader: "js", resolveDir: dirname(path),
        contents: transpiler.transformSync(await Bun.file(path).text()),
      }));
      builder.onLoad({filter: /\.css$/}, async ({path}) => ({loader: "js", contents:
        `__peligames_ui_styles__.push(${JSON.stringify(await Bun.file(path).text())});`,
      }));
    }}],
  });
  if (!result.success) throw new Error(result.logs.map(log => log.message).join("\n"));
  if (result.outputs.length !== 1) throw new Error("Add-on UI must be one self-contained module");
  const bundle = await result.outputs[0].text();
  const source = `export function createAddon(__peligames_ui_host__) {\nconst React = __peligames_ui_host__.React;\nlet __peligames_ui_instance__;\nconst __peligames_ui_styles__ = [];\n${bundle}\nfor (const css of __peligames_ui_styles__) __peligames_ui_host__.styles.add(css);\nreturn __peligames_ui_instance__;\n}\n`;
  if (new TextEncoder().encode(source).length > 2 * 1024 * 1024) throw new Error("Add-on UI exceeds the host's source limit");
  return source;
}
if (import.meta.main) {
  const [entry, output] = process.argv.slice(2);
  if (!entry || !output) throw new Error("Usage: bun addons/_infra/tools/build-ui.ts ENTRY.tsx OUTPUT.js");
  await writeFile(output, await buildAddonUi(entry), {flag: "wx"});
}
