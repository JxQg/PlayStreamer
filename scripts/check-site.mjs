import { readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(root, process.argv[2] ?? 'site-dist');
const basePath = process.env.BASE_PATH ?? '/PlayStreamer/';
const siteUrl = new URL(process.env.SITE_URL ?? 'https://jxqg.github.io/PlayStreamer/');
if (!basePath.startsWith('/') || !basePath.endsWith('/') || siteUrl.pathname !== basePath) {
  throw new Error('BASE_PATH must start/end with / and match the SITE_URL pathname.');
}

function decodeEntities(value) {
  return value.replace(/&(?:#(x[0-9a-f]+|[0-9]+)|amp|quot|apos|lt|gt);/gi, (entity, numeric) => {
    if (numeric) {
      const number = numeric[0].toLowerCase() === 'x' ? parseInt(numeric.slice(1), 16) : parseInt(numeric, 10);
      return String.fromCodePoint(number);
    }
    return { '&amp;': '&', '&quot;': '"', '&apos;': "'", '&lt;': '<', '&gt;': '>' }[entity.toLowerCase()];
  });
}

function attributes(tag) {
  return [...tag.matchAll(/(?:^|\s)(href|src|id|name)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/gi)]
    .map((match) => [match[1].toLowerCase(), decodeEntities(match[2] ?? match[3] ?? match[4])]);
}

async function htmlFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(entries.map(async (entry) => {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return htmlFiles(file);
    return entry.isFile() && entry.name.endsWith('.html') ? [file] : [];
  }));
  return nested.flat();
}

for (const file of ['index.html', 'sitemap.xml', 'robots.txt', '.nojekyll']) {
  if (!(await stat(path.join(output, file))).isFile()) throw new Error(`Missing website output: ${file}`);
}

const pages = new Map();
for (const file of await htmlFiles(output)) {
  const content = (await readFile(file, 'utf8'))
    .replace(/<!--[^]*?-->/g, '')
    .replace(/(<(script|style)\b[^>]*>)[^]*?<\/\2>/gi, '$1');
  const tags = [...content.matchAll(/<([a-z][\w:-]*)\b[^>]*>/gi)];
  const ids = new Set();
  const links = [];
  for (const tag of tags) {
    for (const [name, value] of attributes(tag[0])) {
      if (name === 'id' || (name === 'name' && tag[1].toLowerCase() === 'a')) ids.add(value);
      if (name === 'href' || name === 'src') links.push(value);
    }
  }
  const relative = path.relative(output, file).split(path.sep).join('/');
  const route = relative === 'index.html' || relative.endsWith('/index.html')
    ? relative.slice(0, -'index.html'.length) : relative;
  pages.set(file, { relative, url: new URL(route, siteUrl), ids, links });
}

const failures = [];
let checked = 0;
for (const page of pages.values()) {
  for (const href of page.links) {
    if (!href.trim()) {
      failures.push(`${page.relative}: empty href/src`);
      continue;
    }
    let link;
    try {
      link = new URL(href, page.url);
    } catch {
      failures.push(`${page.relative}: invalid URL ${href}`);
      continue;
    }
    if (!['http:', 'https:'].includes(link.protocol) || link.origin !== siteUrl.origin) continue;
    checked++;
    if (!link.pathname.startsWith(basePath)) {
      failures.push(`${page.relative}: local link is outside BASE_PATH: ${href}`);
      continue;
    }
    let target;
    let fragment;
    try {
      target = path.resolve(output, decodeURIComponent(link.pathname.slice(basePath.length)));
      fragment = decodeURIComponent(link.hash.slice(1));
    } catch {
      failures.push(`${page.relative}: invalid URL encoding: ${href}`);
      continue;
    }
    const relative = path.relative(output, target);
    if (relative.startsWith('..') || path.isAbsolute(relative)) {
      failures.push(`${page.relative}: local link escapes website output: ${href}`);
      continue;
    }
    try {
      if ((await stat(target)).isDirectory()) target = path.join(target, 'index.html');
      if (!(await stat(target)).isFile()) throw new Error('Not a file');
    } catch {
      failures.push(`${page.relative}: missing local target: ${href}`);
      continue;
    }
    const targetPage = pages.get(target);
    if (fragment && targetPage && !targetPage.ids.has(fragment)) {
      failures.push(`${page.relative}: missing fragment: ${href}`);
    }
  }
}
if (failures.length) {
  throw new Error(`Website validation failed:\n${[...new Set(failures)].sort().join('\n')}`);
}
console.log(`Website validation passed: ${pages.size} HTML pages, ${checked} local links/assets, no broken fragments.`);
