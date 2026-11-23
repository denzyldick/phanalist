declare module '*.wasm?url';
declare module '../wasm/phanalist-wasm.js' {
  const init: (input?: unknown) => Promise<unknown>;
  export default init;
  export function analyze(source: string): string;
  export function analyze_with_rules(source: string, rules: string): string;
}
