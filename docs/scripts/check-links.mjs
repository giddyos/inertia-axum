import { promises as fs } from 'node:fs';
import path from 'node:path';
import GithubSlugger from 'github-slugger';

const root = path.resolve('content/docs');
const errors = [];
const expectedPageCount = 61;

async function walk(directory) {
  const entries = await fs.readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await walk(target)));
    if (entry.isFile() && /\.(md|mdx)$/.test(entry.name)) files.push(target);
  }

  return files;
}

function routeFor(file) {
  const relative = path.relative(root, file).replaceAll(path.sep, '/').replace(/\.(md|mdx)$/, '');
  const segments = relative.split('/');
  if (segments.at(-1) === 'index') segments.pop();
  return `/docs${segments.length > 0 ? `/${segments.join('/')}` : ''}`;
}

function cleanRoute(value) {
  if (value.length > 1 && value.endsWith('/')) return value.slice(0, -1);
  return value;
}

function visibleMarkdown(source) {
  return source
    .replace(/^---\n[\s\S]*?\n---\n/, '')
    .replace(/```[\s\S]*?```/g, '')
    .replace(/~~~[\s\S]*?~~~/g, '')
    .replace(/`[^`\n]*`/g, '');
}

function frontmatter(source, name) {
  const match = source.match(/^---\n([\s\S]*?)\n---/);
  if (!match) return undefined;
  const field = match[1].match(new RegExp(`^${name}:\\s*(.+)$`, 'm'));
  return field?.[1].trim().replace(/^['"]|['"]$/g, '');
}

function headings(source) {
  const slugger = new GithubSlugger();
  const anchors = new Set();
  const markdown = visibleMarkdown(source);

  for (const match of markdown.matchAll(/^#{2,6}\s+(.+?)\s*#*\s*$/gm)) {
    const text = match[1]
      .replace(/<[^>]+>/g, '')
      .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      .replace(/[\\*_~]/g, '')
      .trim();
    anchors.add(slugger.slug(text));
  }

  return anchors;
}

function links(source) {
  const markdown = visibleMarkdown(source);
  const found = [];

  for (const match of markdown.matchAll(/(?<!!)\[[^\]]*\]\(([^)\s]+)(?:\s+['"][^'"]*['"])?\)/g)) {
    found.push(match[1].replace(/^<|>$/g, ''));
  }
  for (const match of markdown.matchAll(/\bhref=['"]([^'"]+)['"]/g)) found.push(match[1]);

  return found;
}

async function validateMeta(directory) {
  const entries = await fs.readdir(directory, { withFileTypes: true });
  const metaPath = path.join(directory, 'meta.json');
  let meta;

  try {
    meta = JSON.parse(await fs.readFile(metaPath, 'utf8'));
  } catch (error) {
    errors.push(`${path.relative(root, directory) || '.'}: missing or invalid meta.json (${error.message})`);
    return;
  }

  if (!Array.isArray(meta.pages)) {
    errors.push(`${path.relative(root, metaPath)}: pages must be an array`);
    return;
  }

  const actual = entries
    .filter((entry) => entry.isDirectory() || (entry.isFile() && /\.(md|mdx)$/.test(entry.name)))
    .map((entry) => (entry.isDirectory() ? entry.name : entry.name.replace(/\.(md|mdx)$/, '')))
    .sort();
  const ordered = meta.pages.filter((entry) => typeof entry === 'string' && !entry.startsWith('---'));

  for (const duplicate of ordered.filter((entry, index) => ordered.indexOf(entry) !== index)) {
    errors.push(`${path.relative(root, metaPath)}: duplicate page entry ${duplicate}`);
  }
  for (const missing of actual.filter((entry) => !ordered.includes(entry))) {
    errors.push(`${path.relative(root, metaPath)}: ${missing} is not deliberately ordered`);
  }
  for (const unknown of ordered.filter((entry) => !actual.includes(entry))) {
    errors.push(`${path.relative(root, metaPath)}: ${unknown} does not exist`);
  }

  for (const entry of entries.filter((item) => item.isDirectory())) {
    await validateMeta(path.join(directory, entry.name));
  }
}

const files = await walk(root);
const pages = new Map();

for (const file of files) {
  const source = await fs.readFile(file, 'utf8');
  const route = routeFor(file);
  const anchors = headings(source);

  if (pages.has(route)) {
    errors.push(`${path.relative(root, file)}: duplicate page URL ${route}`);
  }
  if (!frontmatter(source, 'title')) errors.push(`${path.relative(root, file)}: missing title`);
  if (!frontmatter(source, 'description')) errors.push(`${path.relative(root, file)}: missing description`);
  if (anchors.size === 0) errors.push(`${path.relative(root, file)}: missing content heading`);

  pages.set(route, { file, source, anchors });
}

if (pages.size !== expectedPageCount) {
  errors.push(`expected ${expectedPageCount} documentation pages, found ${pages.size}`);
}

await validateMeta(root);

for (const [route, page] of pages) {
  for (const rawLink of links(page.source)) {
    if (/^(?:[a-z][a-z+.-]*:|\/\/)/i.test(rawLink)) continue;

    let decoded;
    try {
      decoded = decodeURI(rawLink);
    } catch {
      errors.push(`${path.relative(root, page.file)}: malformed URL ${rawLink}`);
      continue;
    }

    const [rawPath, anchor] = decoded.split('#', 2);
    const pathname = rawPath.split('?', 1)[0];
    let targetRoute;

    if (!pathname) {
      targetRoute = route;
    } else if (pathname === '/') {
      continue;
    } else if (pathname.startsWith('/docs')) {
      targetRoute = cleanRoute(pathname);
    } else if (pathname.startsWith('/')) {
      continue;
    } else if (/\.(md|mdx)$/.test(pathname)) {
      targetRoute = routeFor(path.resolve(path.dirname(page.file), pathname));
    } else {
      const base = new URL(`${route}/`, 'https://docs.invalid');
      targetRoute = cleanRoute(new URL(pathname, base).pathname);
    }

    const target = pages.get(targetRoute);
    if (!target) {
      errors.push(`${path.relative(root, page.file)}: broken link ${rawLink} (resolved to ${targetRoute})`);
      continue;
    }
    if (anchor && !target.anchors.has(anchor)) {
      errors.push(`${path.relative(root, page.file)}: missing anchor #${anchor} on ${targetRoute}`);
    }
  }
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join('\n'));
  process.exitCode = 1;
} else {
  console.log(`Validated ${pages.size} pages, their metadata ordering, internal links, and heading anchors.`);
}
