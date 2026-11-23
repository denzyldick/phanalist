import React, { useEffect, useMemo, useRef, useState } from 'react';
import { EditorState, StateEffect, StateField } from '@codemirror/state';
import {
  EditorView,
  Decoration,
  keymap,
  lineNumbers,
  highlightActiveLineGutter,
  highlightActiveLine,
} from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { php } from '@codemirror/lang-php';

type RuleMeta = {
  rule_code: string;
  title: string;
  severity: string;
  category: string;
  bad_example: string;
  good_example: string;
  plain_english?: string;
  why_matters?: string;
  what_it_checks?: string[];
};

type Finding = {
  rule?: string;
  message?: string;
  line?: number;
  column?: number;
  severity?: string;
};

type WasmApi = {
  default: (input?: unknown) => Promise<unknown>;
  analyze: (source: string) => string;
  analyze_with_rules: (source: string, rules: string) => string;
};

const WASM_SUPPORTED = new Set(['E0014', 'E0060', 'E0061', 'E0062', 'E0053', 'E0069', 'E0101', 'E0106', 'E0059', 'E0068', 'E0120']);

const highlightEffect = StateEffect.define<{line: number} | null>();
const highlightField = StateField.define({
  create() { return Decoration.none; },
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(highlightEffect)) {
        if (!e.value) return Decoration.none;
        const line = Math.max(1, Math.min(e.value.line, tr.state.doc.lines));
        const from = tr.state.doc.line(line).from;
        return Decoration.set([Decoration.line({ class: 'cm-finding-line' }).range(from)]);
      }
    }
    return deco;
  },
  provide: f => EditorView.decorations.from(f),
});

const FALLBACK_RULES: RuleMeta[] = [
  {
    rule_code: 'E0014',
    title: 'E0014: Law of Demeter violation. Method chaining should be avoided unless returning the same object type.',
    severity: 'major',
    category: 'law-of-demeter',
    bad_example: '<?php\n$a->foo()->bar();\n',
    good_example: '<?php\n$foo = $a->foo();\n$foo->bar();\n',
  },
];

function normalizeExample(code: string): string {
  const trimmed = code.trim();
  if (!trimmed) return '<?php\n\n';
  return trimmed.startsWith('<?php') ? `${trimmed}\n` : `<?php\n${trimmed}\n`;
}

function normCode(s: string): string {
  return s.replace(/\r\n/g, '\n').trim();
}

function findExampleLine(source: string, example: string): number {
  const srcLines = source.replace(/\r\n/g, '\n').split('\n');
  const exLines = example.replace(/\r\n/g, '\n').split('\n').map(l => l.trim()).filter(Boolean);
  if (exLines.length === 0) return 1;
  const needle = exLines[0];
  const idx = srcLines.findIndex(l => l.trim() === needle || l.includes(needle));
  return idx >= 0 ? idx + 1 : 1;
}

function mockAnalyze(source: string, rule?: RuleMeta): Finding[] {
  if (!rule) return [];
  const src = normCode(source);
  const bad = normCode(rule.bad_example || '');
  const good = normCode(rule.good_example || '');
  if (!src) return [];
  const isBad = !!bad && (src === bad || src.includes(bad) || (bad.includes(src) && src.length > 40));
  const isGood = !!good && (src === good || src.includes(good));
  if (!isBad || isGood) return [];
  return [{
    rule: rule.rule_code,
    severity: rule.severity || 'info',
    message: rule.what_it_checks?.[0] || rule.title.replace(`${rule.rule_code}: `, '') || `${rule.rule_code} issue detected`,
    line: findExampleLine(source, bad),
    column: 1,
  }];
}

export default function App() {
  const editorRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const wasmRef = useRef<WasmApi | null>(null);

  const [rules, setRules] = useState<RuleMeta[]>(FALLBACK_RULES);
  const [ruleCode, setRuleCode] = useState('E0014');
  const [source, setSource] = useState(normalizeExample(FALLBACK_RULES[0].bad_example));
  const [findings, setFindings] = useState<Finding[]>([]);
  const [selectedFinding, setSelectedFinding] = useState<number | null>(null);
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [error, setError] = useState<string | null>(null);

  const selectedRule = useMemo(
    () => rules.find((r) => r.rule_code === ruleCode) ?? rules[0],
    [rules, ruleCode],
  );
  const supported = WASM_SUPPORTED.has(ruleCode);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch(`${import.meta.env.BASE_URL}data/rules/index.json`);
        if (!res.ok) throw new Error(`Failed to load rules: ${res.status}`);
        const data = (await res.json()) as { rules?: RuleMeta[] };
        const loaded = (data.rules ?? [])
          .filter((r) => r && r.rule_code)
          .sort((a, b) => a.rule_code.localeCompare(b.rule_code));
        if (!cancelled && loaded.length > 0) {
          setRules(loaded);
          const params = new URLSearchParams(window.location.search);
          const requested = (params.get('rule') || '').toUpperCase();
          const preferred = loaded.find((r) => r.rule_code === requested)
            ?? loaded.find((r) => r.rule_code === 'E0014')
            ?? loaded[0];
          setRuleCode(preferred.rule_code);
          setSource(normalizeExample(preferred.bad_example || preferred.good_example || '<?php\n'));
        }
      } catch {
        // Keep fallback rules.
      }
    })();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const wasm = (await import('../wasm/phanalist-wasm.js')) as unknown as WasmApi;
        await wasm.default(new URL('../wasm/phanalist-wasm_bg.wasm', import.meta.url));
        if (cancelled) return;
        wasmRef.current = wasm;
        setStatus('ready');
      } catch (e) {
        if (cancelled) return;
        setStatus('error');
        setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!editorRef.current || viewRef.current) return;
    const state = EditorState.create({
      doc: source,
      extensions: [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightActiveLine(),
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        php(),
        highlightField,
        EditorView.updateListener.of((u) => {
          if (u.docChanged) setSource(u.state.doc.toString());
        }),
        EditorView.theme({
          '&': { height: '360px', fontSize: '13px' },
          '.cm-scroller': { overflow: 'auto', fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace' },
        }),
      ],
    });
    viewRef.current = new EditorView({ state, parent: editorRef.current });
    return () => { viewRef.current?.destroy(); viewRef.current = null; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const current = view.state.doc.toString();
    if (current !== source) {
      view.dispatch({ changes: { from: 0, to: current.length, insert: source } });
    }
  }, [source]);

  useEffect(() => {
    const handler = (e: Event) => {
      const code = (e as CustomEvent<string>).detail;
      if (typeof code === 'string' && rules.some((r) => r.rule_code === code)) {
        setRuleCode(code);
        const r = rules.find((x) => x.rule_code === code);
        if (r) setSource(normalizeExample(r.bad_example || '<?php\n'));
      }
    };
    window.addEventListener('phanalist:select-rule', handler as EventListener);
    return () => window.removeEventListener('phanalist:select-rule', handler as EventListener);
  }, [rules]);

  useEffect(() => {
    const handle = setTimeout(() => {
      try {
        if (supported && status === 'ready' && wasmRef.current) {
          const raw = wasmRef.current.analyze_with_rules(source, ruleCode);
          const parsed = JSON.parse(raw) as Finding[];
          setFindings(Array.isArray(parsed) ? parsed : []);
        } else {
          setFindings(mockAnalyze(source, selectedRule));
        }
        setSelectedFinding(null);
        viewRef.current?.dispatch({ effects: highlightEffect.of(null) });
      } catch (e) {
        setFindings([{ rule: ruleCode, severity: 'error', message: e instanceof Error ? e.message : String(e) }]);
        setSelectedFinding(null);
      }
    }, 180);
    return () => clearTimeout(handle);
  }, [source, ruleCode, status, supported, selectedRule]);

  const loadExample = (kind: 'bad' | 'good') => {
    if (!selectedRule) return;
    const example = kind === 'bad' ? selectedRule.bad_example : selectedRule.good_example;
    setSource(normalizeExample(example || '<?php\n'));
  };

  const focusFinding = (f: Finding, idx: number) => {
    setSelectedFinding(idx);
    const view = viewRef.current;
    if (!view || !f.line) return;
    const line = Math.max(1, Math.min(f.line, view.state.doc.lines));
    const pos = view.state.doc.line(line).from;
    view.dispatch({
      effects: [highlightEffect.of({ line }), EditorView.scrollIntoView(pos, { y: 'center' })],
      selection: { anchor: pos },
    });
    view.focus();
  };

  const severityClass = (sev?: string) => (sev || 'info').toLowerCase();
  return (
    <div className="playground-layout">
      <div className="play-card">
        <div className="play-header">
          <h3>PHP Code Editor</h3>
          <p>
            {supported && status === 'ready'
              ? 'Live analysis via phanalist-wasm demo.'
              : `Rule preview for ${ruleCode}. Edit code to see findings.`}
          </p>
        </div>
        <div className="toolbar">
          <select
            id="ruleSelect"
            className="select-field"
            value={ruleCode}
            onChange={(e) => {
              const code = e.target.value;
              setRuleCode(code);
              const r = rules.find((x) => x.rule_code === code);
              if (r) setSource(normalizeExample(r.bad_example || r.good_example || '<?php\n'));
            }}
          >
            {rules.map((r) => (
              <option key={r.rule_code} value={r.rule_code}>
                {r.rule_code} — {r.title.replace(`${r.rule_code}: `, '')}
              </option>
            ))}
          </select>
          <button className="btn btn-secondary" onClick={() => loadExample('bad')}>Bad example</button>
          <button className="btn btn-secondary" onClick={() => loadExample('good')}>Good example</button>
        </div>
        <div ref={editorRef} />
        {selectedRule && (
          <div style={{ marginTop: 14, padding: '14px 16px', border: '1px solid var(--border)', borderRadius: 12, background: 'rgba(255,255,255,.02)', color: 'var(--muted)', fontSize: 13 }}>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap', marginBottom: 8 }}>
              <strong style={{ color: 'var(--fg)', fontSize: 14 }}>{selectedRule.rule_code}</strong>
              <span style={{ padding: '2px 8px', border: '1px solid var(--border)', borderRadius: 999, fontSize: 11, textTransform: 'uppercase', letterSpacing: '.04em' }}>{selectedRule.category}</span>
              <span style={{ padding: '2px 8px', border: '1px solid var(--border)', borderRadius: 999, fontSize: 11, textTransform: 'uppercase', letterSpacing: '.04em' }}>{selectedRule.severity}</span>
              <a href={`/rules/${selectedRule.rule_code.toLowerCase()}.html`} style={{ color: 'var(--accent)', marginLeft: 'auto', fontWeight: 600 }}>Open rule page</a>
            </div>
            {selectedRule.plain_english ? <div style={{ color: '#d7d7e2', lineHeight: 1.55 }}>{selectedRule.plain_english}</div> : null}
          </div>
        )}
      </div>

      <div className="play-card">
        <div className="play-header">
          <div>
            <h3>Results</h3>
            <p>Click a finding to jump to the relevant line.</p>
          </div>
          <div className="result-count" style={{ margin: 0, padding: '6px 10px', border: '1px solid var(--border)', borderRadius: 999, background: 'rgba(255,255,255,.02)' }}>
            {findings.length} finding{findings.length === 1 ? '' : 's'}
          </div>
        </div>
        <div className="results-area">
          <div id="resultsList">
            {findings.length === 0 ? (
              <div className="results-empty">No findings for <strong>{ruleCode}</strong>.</div>
            ) : (
              findings.map((f, i) => (
                <button
                  type="button"
                  key={`${f.rule}-${f.line}-${i}`}
                  className={`result-item ${severityClass(f.severity)} ${selectedFinding === i ? 'selected' : ''}`}
                  onClick={() => focusFinding(f, i)}
                  style={{ width: '100%', textAlign: 'left', color: 'inherit', font: 'inherit' }}
                >
                  <div className="result-head">
                    <span className="result-rule">{f.rule || ruleCode}</span>
                    <span className={`result-sev ${severityClass(f.severity)}`}>{f.severity || 'info'}</span>
                  </div>
                  <div className="result-msg">{f.message || 'No message'}</div>
                  <div className="result-snippet">{f.line ? `line ${f.line}${f.column ? `:${f.column}` : ''}` : 'Click not available'}</div>
                </button>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
