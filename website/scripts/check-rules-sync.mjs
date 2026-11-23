import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(__dirname, '..');
const repoRoot = path.resolve(websiteRoot, '..');

function readJson(p) { return JSON.parse(fs.readFileSync(p, 'utf8')); }
function listRuleCodesFromExamples() {
  const base = path.join(repoRoot, 'src', 'rules', 'examples');
  const codes = [];
  for (const dir of fs.readdirSync(base)) {
    const m = /^e(\d+)$/.exec(dir);
    if (!m) continue;
    const md = path.join(base, dir, `${dir}.md`);
    if (fs.existsSync(md)) codes.push(`E${String(Number(m[1])).padStart(4, '0')}`);
  }
  return codes.sort();
}
function listRuleCodesFromRust() {
  const base = path.join(repoRoot, 'src', 'rules');
  const codes = [];
  for (const f of fs.readdirSync(base)) {
    const m = /^e(\d+)\.rs$/.exec(f);
    if (m) codes.push(`E${String(Number(m[1])).padStart(4, '0')}`);
  }
  return codes.sort();
}
function codesFromIndex(p) {
  if (!fs.existsSync(p)) return null;
  const data = readJson(p);
  return (data.rules || []).map(r => r.rule_code).filter(Boolean).sort();
}
function diff(a, b) { const bs = new Set(b); return a.filter(x => !bs.has(x)); }

const exampleCodes = listRuleCodesFromExamples();
const rustCodes = listRuleCodesFromRust();
const webIndex = path.join(websiteRoot, 'data', 'rules', 'index.json');
const publicIndex = path.join(websiteRoot, 'public', 'data', 'rules', 'index.json');
const webCodes = codesFromIndex(webIndex);
const publicCodes = codesFromIndex(publicIndex);

let ok = true;
function fail(msg) { console.error(`FAIL: ${msg}`); ok = false; }
function warn(msg) { console.warn(`WARN: ${msg}`); }

if (!webCodes) fail(`Missing ${path.relative(repoRoot, webIndex)}`);
if (!publicCodes) warn(`Missing ${path.relative(repoRoot, publicIndex)} (generated/public data)`);

for (const [name, codes] of [['website/data/rules/index.json', webCodes], ['website/public/data/rules/index.json', publicCodes]]) {
  if (!codes) continue;
  const missing = diff(exampleCodes, codes);
  const extra = diff(codes, exampleCodes);
  if (missing.length) fail(`${name} missing rule docs codes: ${missing.slice(0,20).join(', ')}${missing.length>20?' ...':''}`);
  if (extra.length) fail(`${name} has codes not in src/rules/examples: ${extra.slice(0,20).join(', ')}${extra.length>20?' ...':''}`);
}

const rulesDir = path.join(websiteRoot, 'public', 'rules');
if (fs.existsSync(rulesDir)) {
  const htmlCodes = fs.readdirSync(rulesDir).filter(f => /^e\d{4}\.html$/.test(f)).map(f => `E${f.slice(1,5).toUpperCase()}`).sort();
  const missing = diff(exampleCodes, htmlCodes);
  const extra = diff(htmlCodes, exampleCodes);
  if (missing.length) fail(`website/public/rules missing HTML pages: ${missing.slice(0,20).join(', ')}${missing.length>20?' ...':''}`);
  if (extra.length) fail(`website/public/rules has extra HTML pages: ${extra.slice(0,20).join(', ')}${extra.length>20?' ...':''}`);
} else {
  warn('Missing website/public/rules (run generate-rule-pages)');
}

// Informational: Rust rule files vs docs. Not all documented rules have eNN.rs.
const docsNotInRust = diff(exampleCodes, rustCodes);
const rustNotInDocs = diff(rustCodes, exampleCodes);
if (docsNotInRust.length) warn(`Rule docs with no src/rules/eNN.rs file: ${docsNotInRust.join(', ')}`);
if (rustNotInDocs.length) fail(`Rust rule files missing docs: ${rustNotInDocs.join(', ')}`);

console.log(`Rule docs: ${exampleCodes.length}, website index: ${webCodes?.length ?? 0}, public index: ${publicCodes?.length ?? 0}, rust eNN.rs: ${rustCodes.length}`);
if (!ok) process.exit(1);
console.log('Rules sync check passed.');
