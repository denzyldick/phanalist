import React, { useState, useEffect, useRef, useCallback } from 'react';
import { CodeMirrorProvider, EditorView } from '@codemirror/state';
import { php } from '@codemirror/lang-php';
import { drawSelection, highlightSelection } from '@codemirror/view';
import { autocomplete } from '@codemirror/autocomplete';
import { history } from '@codemirror/history';
import { default as react } from 'react';

// Lazy-load the WASM module
const importWasm = React.lazy(() => import('phanalist-wasm'));

function UseWasmAnalyzer() {
  const [analyze, setAnalyze] = useState<any>(null);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    importWasm.then((module) => {
      setAnalyze(module.analyze);
      setLoaded(true);
    });
  }, [importWasm]);

  return { analyze, loaded };
}

export default function App() {
  const [source, setSource] = useState('<?php $a->foo->bar(); ?>');
  const [findings, setFindings] = useState<any[]>([]);
  const [rule, setRule] = useState('E0014');
  const [rulesList, setRulesList] = useState<string[]>([]);
  const [severity, setSeverity] = useState('error');
  const [showGood, setShowGood] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const editorRef = useRef<any>(null);

  const { analyze: wasmAnalyze, loaded: wasmLoaded } = UseWasmAnalyzer();

  useEffect(() => {
    if (wasmLoaded && wasmAnalyze) {
      setIsLoading(false);
    }
  }, [wasmLoaded, wasmAnalyze]);

  useEffect(() => {
    if (wasmAnalyze && source.trim()) {
      const result = wasmAnalyze(source);
      try {
        const parsed = JSON.parse(result);
        setFindings(parsed);
      } catch (e) {
        console.error('Failed to parse WASM result:', e);
        setFindings([]);
      }
    }
  }, [wasmAnalyze, source]);

  useEffect(() => {
    // Initialize rules list from the JSON index
    fetch('/rules/index.json')
      .then((res) => {
        if (!res.ok) throw new Error('Not found');
        return res.json();
      })
      .then((data) => {
        setRulesList(data.rules.map((r: any) => r.rule_code));
      })
      .catch(() => {
        // Fallback rules
        setRulesList([
          'E0014',
          'E0031',
          'E0044',
          'E0045',
          'E0046',
          'E0049',
          'E0051',
          'E0052',
          'E0053',
          'E0054',
          'E0055',
          'E0056',
          'E0057',
          'E0058',
          'E0059',
          'E0060',
          'E0061',
          'E0062',
        ]);
      });
  }, []);

  // Debounced analysis
  const [debounceId, setDebounceId] = useState<NodeJS.Timeout | null>();
  const handleChange = useCallback(
    (newSource: string) => {
      setSource(newSource);
      if (debounceId) clearTimeout(debounceId);
      const newId = setTimeout(() => {
        if (wasmAnalyze && newSource.trim()) {
          const result = wasmAnalyze(newSource);
          try {
            const parsed = JSON.parse(result);
            setFindings(parsed);
          } catch (e) {
            setFindings([]);
          }
        } else {
          setFindings([]);
        }
      }, 200);
      setDebounceId(newId);
    },
    [wasmAnalyze]
  );

  const toggleGood = () => setShowGood((prev) => !prev);

  // Initialize CodeMirror when editorRef is ready
  useEffect(() => {
    if (editorRef.current) {
      // CodeMirror is initialized via the ref handling in the JSX
    }
  }, [editorRef]);

  return (
    <div
      style={{
        padding: '2rem',
        maxWidth: '100%',
        fontFamily: 'system-ui, sans-serif',
        background: '#0a0a0a',
        color: '#e0e0e0',
      }}
    >
      <h2 style={{ marginBottom: '1rem', color: '#fff' }}>phanalist Playground</h2>

      {isLoading && <p style={{ margin: '2rem 0', color: '#888' }}>Loading analyzer...</p>}

      <div style={{ marginBottom: '1rem' }}>
        <label style={{ marginRight: '1rem', color: '#ccc' }}>
          Rule:
          <select
            value={rule}
            onChange={(e) => {
              setRule(e.target.value);
              setSource('');
              setFindings([]);
              if (debounceId) clearTimeout(debounceId);
            }}
          >
            {rulesList.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
        </label>
        <button
          onClick={toggleGood}
          style={{
            marginLeft: '1rem',
            padding: '0.5rem 1rem',
            background: '#555',
            color: '#fff',
            border: 'none',
            borderRadius: '4px',
          }}
          title="Show Good Example (toggle)"
        >
          {showGood ? 'Show Bad' : 'Show Good'}
        </button>
      </div>

      <div
        style={{
          border: '1px solid #333',
          borderRadius: '4px',
          overflow: 'hidden',
          marginBottom: '1rem',
        }}
      >
        <h3
          style={{ margin: '0.5rem 1rem', background: '#222', color: '#fff', padding: '0 0.5rem' }}
        >
          Code Editor ({showGood ? 'Good Example' : 'Your Code'})
        </h3>
        <div style={{ height: '450px', width: '100%' }}>
          {wasmLoaded && wasmAnalyze ? (
            <div style={{ height: '100%', width: '100%' }}>
              {/* CodeMirror will be rendered here - placeholder for now */}
              <div
                style={{
                  height: '100%',
                  width: '100%',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: '#666',
                }}
              >
                CodeMirror editor ready
                <br />
                (analyze code to see findings)
              </div>
            </div>
          ) : (
            <div style={{ height: '450px', width: '100%', color: '#666', padding: '2rem' }}>
              {wasmLoaded ? (
                <p>WASM loaded - start typing PHP code above</p>
              ) : (
                <p>Loading analyzer...</p>
              )}
            </div>
          )}
        </div>
      </div>

      <div
        style={{
          border: '1px solid #500',
          borderRadius: '4px',
          padding: '1rem',
          marginBottom: '1rem',
          background: '#ffe',
          display: showGood ? 'block' : 'none',
        }}
      >
        <h4 style={{ margin: '0 0 0.5rem 0', color: '#500' }}>Good Example</h4>
        <pre style={{ whiteSpace: 'pre-wrap', color: '#d00', fontSize: '0.9rem' }}>
          {showGood
            ? '<?php $id = $_GET["id"] ?? "";\n' +
              '$stmt = $pdo->prepare("SELECT * FROM users WHERE id = ?");\n' +
              '$stmt->execute([$id]);\n' +
              '$user = $stmt->fetch();'
            : '<?php $id = $_GET["id"] ?? "";\n' +
              '$result = mysqli_query($conn, "SELECT * FROM users WHERE id = $id");\n' +
              '---\n' +
              'This demonstrates an E0060 SQL injection violation'}
        </pre>
      </div>

      <div
        style={{
          border: '1px solid #500',
          borderRadius: '4px',
          padding: '1rem',
          marginBottom: '1rem',
          background: '#ffe',
          display: !showGood && findings.length > 0 ? 'block' : 'none',
        }}
      >
        <h4 style={{ margin: '0 0 0.5rem 0', color: '#500' }}>Findings ({findings.length})</h4>
        {findings.map((f: any, i: number) => (
          <div
            key={i}
            style={{ padding: '0.25rem 0', borderBottom: '1px solid #555', fontSize: '0.85rem' }}
          >
            <span style={{ fontWeight: 'bold', color: '#500' }}>{f.rule || 'E???'}:</span>
            <span style={{ color: '#d00' }}>{f.message || 'No message'}</span>
            <span style={{ float: 'right', color: '#555', fontSize: '0.7em' }}>
              line {f.line}, col {f.column}
            </span>
          </div>
        ))}
        {findings.length === 0 && (
          <p style={{ margin: '0.5rem 0' }}>No findings. The code appears clean!</p>
        )}
      </div>

      <textarea
        style={{
          width: '100%',
          height: '120px',
          marginTop: '1rem',
          padding: '0.5rem',
          background: '#1a1a1a',
          color: '#e0e0e0',
          border: '1px solid #333',
          borderRadius: '4px',
          resize: 'vertical',
        }}
        placeholder="Enter PHP code here..."
        value={source}
        onChange={(e) => {
          handleChange(e.target.value);
        }}
      ></textarea>

      <div style={{ marginTop: '1rem', display: 'flex', gap: '1rem', alignItems: 'center' }}>
        <button
          onClick={toggleGood}
          style={{
            flex: 1,
            padding: '0.5rem 1rem',
            background: '#555',
            color: '#fff',
            border: 'none',
            borderRadius: '4px',
          }}
        >
          {showGood ? 'Show Bad Example' : 'Show Good Example'}
        </button>
        <button
          onClick={() => navigator.clipboard.writeText(source)}
          style={{
            flex: 1,
            padding: '0.5rem 1rem',
            background: '#555',
            color: '#fff',
            border: 'none',
            borderRadius: '4px',
          }}
          title="Copy code to clipboard"
        >
          Copy
        </button>
      </div>
    </div>
  );
}
