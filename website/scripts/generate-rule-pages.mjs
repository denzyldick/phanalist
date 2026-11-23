import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { marked } from 'marked';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const repoRoot = path.resolve(root, '..');
const rulesPath = path.join(root, 'data', 'rules', 'index.json');
const outDir = path.join(root, 'public', 'rules');
const distRulesDir = path.join(root, 'dist', 'rules');

const data = JSON.parse(fs.readFileSync(rulesPath, 'utf8'));
const rules = (data.rules || []).filter((r) => r && r.rule_code);
fs.mkdirSync(outDir, { recursive: true });

marked.setOptions({ gfm: true, breaks: false });

const esc = (s = '') =>
  String(s)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');

function readRuleMarkdown(ruleCode) {
  const n = parseInt(ruleCode.slice(1), 10);
  const p = path.join(repoRoot, 'src', 'rules', 'examples', `e${n}`, `e${n}.md`);
  if (!fs.existsSync(p)) return '';
  return fs.readFileSync(p, 'utf8').replace(/^\uFEFF/, '');
}

function stripLeadingTitle(md, ruleCode) {
  const lines = md.split(/\r?\n/);
  if (lines[0] && /^#\s+/.test(lines[0]) && lines[0].includes(ruleCode)) {
    return lines.slice(1).join('\n').trim();
  }
  return md.trim();
}

function fallbackMarkdown(rule) {
  const code = rule.rule_code;
  const parts = [];
  if (rule.plain_english) parts.push(`**In plain English:** ${rule.plain_english}`);
  if (rule.why_matters) parts.push(`**Why it matters:** ${rule.why_matters}`);
  if (Array.isArray(rule.what_it_checks) && rule.what_it_checks.length) {
    parts.push(`## What it checks\n\n${rule.what_it_checks.map((c) => `- ${c}`).join('\n')}`);
  }
  if (rule.bad_example) parts.push(`## Bad example\n\n\`\`\`php\n${rule.bad_example}\n\`\`\``);
  if (rule.good_example) parts.push(`## Good example\n\n\`\`\`php\n${rule.good_example}\n\`\`\``);
  if (!parts.length) parts.push(`Documentation for ${code} is not available yet.`);
  return parts.join('\n\n');
}

const baseUrl = 'https://phanalist.io';
const urls = [`${baseUrl}/`];

for (const rule of rules) {
  const code = rule.rule_code;
  const mdRaw = readRuleMarkdown(code);
  const md = stripLeadingTitle(mdRaw || fallbackMarkdown(rule), code);
  const bodyHtml = marked.parse(md);
  const titleText = (rule.title || `${code}`).replace(`${code}: `, '');
  const desc =
    rule.plain_english ||
    rule.why_matters ||
    (Array.isArray(rule.what_it_checks) ? rule.what_it_checks[0] : '') ||
    `Phanalist ${code} rule documentation.`;
  const canonical = `${baseUrl}/rules/${code.toLowerCase()}.html`;
  urls.push(canonical);

  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>${esc(code)}: ${esc(titleText)} | Phanalist</title>
  <meta name="description" content="${esc(desc).slice(0, 160)}" />
  <link rel="canonical" href="${canonical}" />
  <meta property="og:title" content="${esc(code)}: ${esc(titleText)} | Phanalist" />
  <meta property="og:description" content="${esc(desc).slice(0, 200)}" />
  <meta property="og:type" content="article" />
  <meta property="og:url" content="${canonical}" />
  <style>
    :root { --bg:#0a0a0f; --card:#161620; --subtle:#12121a; --fg:#e8e8f0; --muted:#8f8fa3; --accent:#00d4aa; --border:#2a2a3a; }
    *{box-sizing:border-box} body{margin:0;font-family:system-ui,-apple-system,sans-serif;background:var(--bg);color:var(--fg);line-height:1.7}
    .wrap{max-width:980px;margin:0 auto;padding:32px 20px 72px}
    a{color:var(--accent);text-decoration:none} a:hover{text-decoration:underline}
    .crumbs{font-size:14px;color:var(--muted);margin-bottom:18px}
    .badges{display:flex;gap:8px;flex-wrap:wrap;margin-bottom:14px}
    .badge{display:inline-flex;padding:4px 10px;border:1px solid var(--border);border-radius:999px;font-size:12px;color:var(--muted);background:var(--subtle)}
    h1{font-size:clamp(28px,5vw,44px);line-height:1.15;margin:0 0 12px;letter-spacing:-.02em}
    .lead{font-size:18px;color:#cfcfda;max-width:78ch;margin:0 0 20px}
    .actions{display:flex;gap:10px;flex-wrap:wrap;margin:0 0 28px}
    .btn{display:inline-flex;padding:10px 14px;border-radius:8px;border:1px solid var(--border);background:var(--subtle);color:var(--fg);font-weight:600}
    .btn-primary{background:linear-gradient(135deg,var(--accent),#0066ff);color:#081018;border:none}
    .content{background:var(--card);border:1px solid var(--border);border-radius:16px;padding:28px}
    .content > :first-child{margin-top:0} .content > :last-child{margin-bottom:0}
    .content h2{margin-top:32px;font-size:24px;line-height:1.25}
    .content h3{margin-top:24px;font-size:19px;line-height:1.3}
    .content p,.content li{color:#d9d9e3}
    .content ul,.content ol{padding-left:24px}
    .content pre{margin:16px 0;background:#0d0d15;border:1px solid var(--border);border-radius:10px;padding:16px;overflow:auto}
    .content code{font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace;font-size:.94em}
    .content p code,.content li code{background:#0d0d15;border:1px solid var(--border);padding:.12em .35em;border-radius:5px}
    .content pre code{background:none;border:none;padding:0;font-size:13px;line-height:1.55}
    .content blockquote{margin:16px 0;padding:0 0 0 16px;border-left:3px solid var(--accent);color:#cfcfda}
    .content table{width:100%;border-collapse:collapse;margin:16px 0}
    .content th,.content td{border:1px solid var(--border);padding:10px;text-align:left;vertical-align:top}
    .content th{background:var(--subtle)}
  </style>
  <script type="application/ld+json">${JSON.stringify({
    '@context': 'https://schema.org',
    '@type': 'TechArticle',
    headline: `${code}: ${titleText}`,
    description: desc,
    url: canonical,
  })}</script>
</head>
<body>
  <div class="wrap">
    <div class="crumbs"><a href="/">Phanalist</a> / <a href="/#rules">Rules</a> / ${esc(code)}</div>
    <div class="badges">
      <span class="badge">${esc(code)}</span>
      <span class="badge">${esc(rule.category || 'general')}</span>
      <span class="badge">${esc(rule.severity || 'info')}</span>
    </div>
    <h1>${esc(titleText)}</h1>
    <p class="lead">${esc(rule.plain_english || desc)}</p>
    <div class="actions">
      <a class="btn btn-primary" href="/?rule=${encodeURIComponent(code)}#playground">Try in Playground</a>
      <a class="btn" href="/#rules">All rules</a>
      <a class="btn" href="https://github.com/denzyldick/phanalist">GitHub</a>
    </div>
    <article class="content">
      ${bodyHtml}
    </article>
  </div>
</body>
</html>`;
  fs.writeFileSync(path.join(outDir, `${code.toLowerCase()}.html`), html);
}

fs.writeFileSync(path.join(root, 'public', 'robots.txt'), `User-agent: *\nAllow: /\nSitemap: ${baseUrl}/sitemap.xml\n`);
const sitemap = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls
  .map((u) => `  <url><loc>${u}</loc></url>`)
  .join('\n')}\n</urlset>\n`;
fs.writeFileSync(path.join(root, 'public', 'sitemap.xml'), sitemap);

if (fs.existsSync(path.join(root, 'dist'))) {
  fs.mkdirSync(distRulesDir, { recursive: true });
  for (const f of fs.readdirSync(outDir)) fs.copyFileSync(path.join(outDir, f), path.join(distRulesDir, f));
  fs.copyFileSync(path.join(root, 'public', 'robots.txt'), path.join(root, 'dist', 'robots.txt'));
  fs.copyFileSync(path.join(root, 'public', 'sitemap.xml'), path.join(root, 'dist', 'sitemap.xml'));
}
console.log(`Generated ${rules.length} rule pages`);
