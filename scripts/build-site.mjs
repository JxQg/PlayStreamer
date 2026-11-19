import { copyFile, cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Marked } from 'marked';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const config = JSON.parse(await readFile(path.join(root, 'site/site.json'), 'utf8'));
const pkg = JSON.parse(await readFile(path.join(root, 'package.json'), 'utf8'));
const origin = new URL(process.env.SITE_URL || config.url);
const base = process.env.BASE_PATH || origin.pathname;
if (!base.startsWith('/') || !base.endsWith('/') || base.includes('..') || /[?#]/.test(base)) throw new Error('BASE_PATH must be an absolute directory path.');
if (!['http:', 'https:'].includes(origin.protocol) || origin.pathname !== base) throw new Error('SITE_URL must match BASE_PATH.');
const output = path.join(root, 'site-dist');
// Only the generated site output is replaced; source documents remain in docs/.
await rm(output, { recursive: true, force: true });
await mkdir(path.join(output, 'assets/fonts'), { recursive: true });
for (const name of ['site.css', 'site.js']) await copyFile(path.join(root, 'site', name), path.join(output, 'assets', name));
await copyFile(path.join(root, 'public/brand/playstreamer.svg'), path.join(output, 'assets/playstreamer.svg'));
await copyFile(path.join(root, 'public/fonts/NotoSansSC-OFL.txt'), path.join(output, 'assets/fonts/OFL.txt'));
const fontRoot = path.join(root, 'node_modules/@fontsource-variable/noto-sans-sc');
await copyFile(path.join(fontRoot, 'index.css'), path.join(output, 'assets/fonts/index.css'));
await cp(path.join(fontRoot, 'files'), path.join(output, 'assets/fonts/files'), { recursive: true });

const esc = (value) => String(value).replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]);
const slug = (value) => value.toLowerCase().replace(/<[^>]*>/g, '').replace(/[^\p{L}\p{N}_\s-]/gu, '').trim().replace(/\s/g, '-');
const iconPaths = {
  DOWNLOAD: '<path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5"/>',
  ARROW: '<path d="M5 12h14m-6-6 6 6-6 6"/>',
  GAMEPAD: '<path d="M7 6h10a4 4 0 0 1 4 3l1 8a3 3 0 0 1-5 2l-3-3h-4l-3 3a3 3 0 0 1-5-2l1-8a4 4 0 0 1 4-3Z"/><path d="M5 11h6m-3-3v6m8-4h.01M19 13h.01"/>',
  MONITOR: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8m-4-4v4"/>',
  CABLE: '<path d="M5 3v5m4-5v5M3 8h8v3a4 4 0 0 1-8 0V8Zm4 7v2a4 4 0 0 0 8 0v-3m-2-4h4v4h-4zM15 3v7"/>',
  COPY: '<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 8V3H3v13h5"/>',
  WINDOW: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 8h18m-5 9h3"/>',
  BOOK: '<path d="M12 5C9 3 5 3 2 4v16c4-1 7-1 10 1 3-2 6-2 10-1V4c-3-1-7-1-10 1Zm0 0v16"/>',
  PLAY: '<path d="m9 5 11 7-11 7V5Z"/><path d="M3 5v14"/>',
  HELP: '<circle cx="12" cy="12" r="10"/><path d="M9 9a3 3 0 1 1 5 2c-2 1-2 2-2 3m0 3h.01"/>',
};
const icons = Object.fromEntries(Object.entries(iconPaths).map(([name, body]) => [`ICON_${name}`, `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`]));
const download = `${config.repository}/releases/download/v${pkg.version}/PlayStreamer_${pkg.version}_windows_x64_setup.exe`;
const replacements = { BASE: base, REPO: config.repository, VERSION: pkg.version, DOWNLOAD: download, ...icons };
const fill = (html) => html.replace(/\{\{([A-Z_]+)\}\}/g, (_, key) => { if (!(key in replacements)) throw new Error(`Unknown template value ${key}`); return replacements[key]; });

const nav = (current) => `<header class="site-header"><div class="wrap header-inner"><a href="${base}" class="brand" aria-label="PlayStreamer 首页"><img src="${base}assets/playstreamer.svg" width="32" height="32" alt=""><span>PlayStreamer</span></a><button class="menu-toggle" type="button" aria-controls="site-navigation" aria-expanded="false" aria-label="展开导航"><span></span><span></span></button><nav id="site-navigation" aria-label="主导航"><a href="${base}#how-it-works">工作方式</a><a href="${base}guide/"${current ? ' aria-current="true"' : ''}>使用文档</a><a href="${config.repository}" class="github-link">GitHub <span aria-hidden="true">↗</span></a><a class="nav-download" href="${download}">下载 v${pkg.version}</a></nav></div></header>`;
const footer = `<footer class="site-footer"><div class="wrap footer-top"><a class="brand" href="${base}"><img src="${base}assets/playstreamer.svg" width="28" height="28" alt=""><span>PlayStreamer</span></a><div><a href="${base}guide/">操作文档</a><a href="${config.repository}/issues">反馈问题</a><a href="${config.repository}/releases">版本发布</a><a href="${config.repository}/blob/main/LICENSE">MIT 许可证</a></div></div><div class="wrap footer-bottom"><p>让主机的精彩，多一个去处。</p><p>独立开源项目，与 Sony、PlayStation、Twitch、Bilibili 及 OOPZ 无隶属关系。</p></div></footer>`;
function page({ title, description, route = '', body, document = false }) {
  const canonical = new URL(route, origin).href;
  return `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="theme-color" content="#175ce6"><title>${esc(title)}</title><meta name="description" content="${esc(description)}"><link rel="canonical" href="${canonical}"><meta property="og:type" content="website"><meta property="og:locale" content="zh_CN"><meta property="og:site_name" content="PlayStreamer"><meta property="og:title" content="${esc(title)}"><meta property="og:description" content="${esc(description)}"><meta property="og:url" content="${canonical}"><meta property="og:image" content="${new URL('assets/social-preview.png', origin).href}"><meta name="twitter:card" content="summary_large_image"><link rel="icon" type="image/svg+xml" href="${base}assets/playstreamer.svg"><link rel="stylesheet" href="${base}assets/fonts/index.css"><link rel="stylesheet" href="${base}assets/site.css"><script src="${base}assets/site.js" defer></script></head><body${document ? ' class="docs-page"' : ''}><a class="skip-link" href="#main">跳到正文</a>${nav(document)}${body}${footer}<div class="toast" role="status" aria-live="polite"></div></body></html>`;
}

const home = fill(await readFile(path.join(root, 'site/home.html'), 'utf8'));
await writeFile(path.join(output, 'index.html'), page({ title: 'PlayStreamer · 把 PS5 的精彩带到你的直播间', description: config.description, body: home }));
for (let index = 0; index < config.documents.length; index++) {
  const doc = config.documents[index];
  const headings = [];
  const seen = new Map();
  const markdown = new Marked({ gfm: true, renderer: {
    heading({ tokens, depth, text }) {
      const key = slug(text);
      const count = seen.get(key) || 0; seen.set(key, count + 1);
      const id = key + (count ? `-${count}` : '');
      const label = this.parser.parseInline(tokens);
      if (depth === 2) headings.push({ id, text: text.replace(/\*|`/g, '') });
      return `<h${depth} id="${esc(id)}">${label}</h${depth}>\n`;
    },
    link(token) {
      let href = token.href;
      if (!/^(?:[a-z][a-z0-9+.-]*:|\/|#)/i.test(href)) {
        const [file, hash] = href.split('#');
        const related = config.documents.find(item => item.source === file);
        href = related ? `${base}${related.path}${hash ? '#' + hash : ''}` : `${config.repository}/blob/main/docs/${href}`;
      }
      if (!/^(?:https?:|mailto:|#|\/)/i.test(href)) throw new Error(`Unsupported document link ${href}`);
      return `<a href="${esc(href)}"${token.title ? ` title="${esc(token.title)}"` : ''}>${this.parser.parseInline(token.tokens)}</a>`;
    },
    table(token) { return `<div class="table-scroll" tabindex="0" role="region" aria-label="数据表格">${this.constructor.prototype.table.call(this, token)}</div>`; },
  } });
  const content = markdown.parse(await readFile(path.join(root, 'docs', doc.source), 'utf8'));
  const previous = config.documents[index - 1];
  const next = config.documents[index + 1];
  const sidebar = `<aside class="docs-sidebar"><p>操作文档</p><nav aria-label="文档导航">${config.documents.map(item => `<a href="${base}${item.path}"${item === doc ? ' aria-current="page"' : ''}>${esc(item.label)}</a>`).join('')}</nav><a class="sidebar-release" href="${config.repository}/releases/latest">下载 v${pkg.version} ${icons.ICON_ARROW}</a></aside>`;
  const toc = `<aside class="docs-toc"><span>本页内容</span><nav aria-label="本页目录">${headings.map(h => `<a href="#${esc(h.id)}">${esc(h.text)}</a>`).join('')}</nav></aside>`;
  const body = `<div class="wrap docs-layout">${sidebar}<main id="main" class="docs-main"><div class="doc-breadcrumb"><a href="${base}">首页</a><span aria-hidden="true">/</span><span>${esc(doc.label)}</span></div><article class="prose">${content}</article><div class="doc-edit"><a href="${config.repository}/edit/main/docs/${doc.source}">在 GitHub 编辑此页 ↗</a></div><nav class="doc-pagination" aria-label="相邻文档">${previous ? `<a href="${base}${previous.path}"><span>上一篇</span>${esc(previous.label)}</a>` : '<span></span>'}${next ? `<a href="${base}${next.path}"><span>下一篇</span>${esc(next.label)}</a>` : ''}</nav></main>${toc}</div>`;
  await mkdir(path.join(output, doc.path), { recursive: true });
  await writeFile(path.join(output, doc.path, 'index.html'), page({ title: `${doc.label} · PlayStreamer 文档`, description: doc.description, route: doc.path, body, document: true }));
}
const routes = ['', ...config.documents.map(item => item.path)];
await writeFile(path.join(output, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${routes.map(route => `<url><loc>${new URL(route, origin).href}</loc></url>`).join('')}</urlset>`);
await writeFile(path.join(output, 'robots.txt'), `User-agent: *\nAllow: /\nSitemap: ${new URL('sitemap.xml', origin).href}\n`);
await writeFile(path.join(output, '.nojekyll'), '');
await writeFile(path.join(output, '404.html'), page({ title: '页面未找到 · PlayStreamer', description: '返回 PlayStreamer 首页或查看操作文档。', body: `<main id="main" class="wrap not-found"><p>404</p><h1>这个页面不在接收范围内。</h1><p>可以返回首页，或从操作文档继续。</p><a class="button primary" href="${base}">返回首页</a><a class="button secondary" href="${base}guide/">查看文档</a></main>` }));
// Generate a repo-native social preview from the same brand symbol.
const { Resvg } = await import('@resvg/resvg-js');
const symbol = (await readFile(path.join(root, 'public/brand/playstreamer.svg'), 'utf8')).replace(/<svg[^>]*>|<\/svg>/g, '');
const social = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630"><rect width="1200" height="630" fill="#f3f7ff"/><path d="M780 0h420v630H570Z" fill="#e4edff"/><g transform="translate(85 95) scale(1.1)">${symbol}</g><text x="85" y="345" font-family="sans-serif" font-weight="700" font-size="90" fill="#15294d">PlayStreamer</text><text x="90" y="430" font-family="sans-serif" font-size="36" fill="#175ce6">PS5 → OBS → Your live stream</text><text x="90" y="535" font-family="sans-serif" font-size="25" fill="#526480">Windows · Open source · MIT</text></svg>`;
await writeFile(path.join(output, 'assets/social-preview.png'), new Resvg(social, { font: { loadSystemFonts: true } }).render().asPng());
console.log(`Built ${routes.length} pages plus 404 at ${output} for ${origin.href}`);
