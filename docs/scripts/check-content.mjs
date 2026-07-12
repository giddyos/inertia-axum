import { promises as fs } from 'node:fs'
import path from 'node:path'

const docsRoot = process.cwd()
const repositoryRoot = path.resolve(docsRoot, '..')
const contentRoot = path.join(docsRoot, 'content/docs')
const errors = []
const debt = []

async function walk(directory, extension) {
  const entries = await fs.readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const target = path.join(directory, entry.name)
    if (entry.isDirectory()) files.push(...(await walk(target, extension)))
    if (entry.isFile() && target.endsWith(extension)) files.push(target)
  }
  return files
}

function relative(file) {
  return path.relative(docsRoot, file).replaceAll(path.sep, '/')
}

function parseTomlString(source, field) {
  return source.match(new RegExp(`^${field}\\s*=\\s*"([^"]+)"`, 'm'))?.[1]
}

function packageVersion(manifest, dependency) {
  return manifest.dependencies?.[dependency] ?? manifest.devDependencies?.[dependency]
}

function normalizedVersion(value) {
  return typeof value === 'string' ? value.replace(/^[~^]/, '') : value
}

const workspaceToml = await fs.readFile(path.join(repositoryRoot, 'Cargo.toml'), 'utf8')
const workspaceVersion = parseTomlString(workspaceToml, 'version')
const msrv = parseTomlString(workspaceToml, 'rust-version')
const docsPackage = JSON.parse(await fs.readFile(path.join(docsRoot, 'package.json'), 'utf8'))

if (docsPackage.version !== workspaceVersion) {
  errors.push(`package.json: version ${docsPackage.version} does not match workspace ${workspaceVersion}`)
}

const frameworkManifests = {
  svelte: JSON.parse(await fs.readFile(path.join(repositoryRoot, 'examples/axum-svelte/svelte-app/package.json'), 'utf8')),
  react: JSON.parse(await fs.readFile(path.join(repositoryRoot, 'examples/axum-react/react-app/package.json'), 'utf8')),
  vue: JSON.parse(await fs.readFile(path.join(repositoryRoot, 'examples/axum-vue/vue-app/package.json'), 'utf8')),
}

for (const [framework, dependencies] of Object.entries({
  svelte: ['@inertiajs/svelte', 'svelte'],
  react: ['@inertiajs/react', 'react', 'react-dom'],
  vue: ['@inertiajs/vue3'],
})) {
  for (const dependency of dependencies) {
    const expected = normalizedVersion(packageVersion(frameworkManifests[framework], dependency))
    const actual = normalizedVersion(packageVersion(docsPackage, dependency))
    if (actual !== expected) {
      errors.push(`package.json: ${dependency} ${actual ?? 'is missing'}; maintained ${framework} example uses ${expected}`)
    }
  }
}

if (normalizedVersion(docsPackage.devDependencies.svelte) !== normalizedVersion(packageVersion(frameworkManifests.svelte, 'svelte'))) {
  errors.push('package.json: Svelte compiler version differs from the maintained Svelte example')
}
if (normalizedVersion(docsPackage.devDependencies['@vue/compiler-sfc']) !== normalizedVersion(packageVersion(frameworkManifests.vue, 'vue'))) {
  errors.push('package.json: Vue compiler version differs from the maintained Vue example')
}

const files = await walk(contentRoot, '.mdx')
const allDocumentation = (await Promise.all(files.map((file) => fs.readFile(file, 'utf8')))).join('\n')
const pages = new Map()
const stalePaths = ['/docs/installation', '/docs/quick-start', '/docs/server-side-rendering']
const fileLanguages = new Set(['css', 'html', 'js', 'jsx', 'json', 'rust', 'svelte', 'toml', 'ts', 'tsx', 'vue', 'yaml', 'yml'])

for (const file of files) {
  const source = await fs.readFile(file, 'utf8')
  const name = relative(file)
  const routePart = path.relative(contentRoot, file).replaceAll(path.sep, '/').replace(/\.mdx$/, '').replace(/\/?index$/, '')
  pages.set(routePart ? `/docs/${routePart}` : '/docs', file)

  for (const match of source.matchAll(/inertia-axum(?:-test)?[^\n`"]*?[=@" ](\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)/g)) {
    if (match[1] !== workspaceVersion) errors.push(`${name}: documented workspace version ${match[1]} does not match ${workspaceVersion}`)
  }
  for (const match of source.matchAll(/Rust\s+(\d+\.\d+)/g)) {
    if (match[1] !== msrv) errors.push(`${name}: documented MSRV ${match[1]} does not match ${msrv}`)
  }
  for (const stale of stalePaths) {
    if (source.includes(`${stale})`) || source.includes(`${stale}#`) || source.includes(`href="${stale}"`)) {
      errors.push(`${name}: stale documentation path ${stale}`)
    }
  }
  for (const block of source.matchAll(/^```(\w+)([^\n]*)\n([\s\S]*?)^```/gm)) {
    const [, language, metadata, code] = block
    if (language === 'rust' && /^# /m.test(code)) {
      debt.push(`${name}:${source.slice(0, block.index).split('\n').length}: remove visible Rustdoc-hidden lines`)
    }
    if (fileLanguages.has(language) && !/\btitle=/.test(metadata)) {
      debt.push(`${name}:${source.slice(0, block.index).split('\n').length}: add title metadata to the ${language} file example`)
    }
  }
  for (const match of source.matchAll(/<Snippet\s+[\s\S]*?source=["']([^"']+)["'][\s\S]*?\/>/g)) {
    const snippet = path.resolve(docsRoot, 'snippets', match[1])
    try {
      await fs.access(snippet)
    } catch {
      errors.push(`${name}: snippet source does not exist: ${match[1]}`)
    }
  }
}

async function validateNavigation(directory) {
  const metaFile = path.join(directory, 'meta.json')
  const meta = JSON.parse(await fs.readFile(metaFile, 'utf8'))
  for (const entry of meta.pages ?? []) {
    if (typeof entry !== 'string' || entry.startsWith('---')) continue
    const page = path.join(directory, `${entry}.mdx`)
    const child = path.join(directory, entry)
    try {
      await fs.access(page)
    } catch {
      try {
        await fs.access(path.join(child, 'meta.json'))
        await validateNavigation(child)
      } catch {
        errors.push(`${relative(metaFile)}: navigation entry ${entry} has no MDX page or section`)
      }
    }
  }
}

await validateNavigation(contentRoot)

const quickStartFile = path.join(contentRoot, 'getting-started/quick-start.mdx')
const quickStart = await fs.readFile(quickStartFile, 'utf8')
for (const framework of ['Svelte', 'React', 'Vue']) {
  if (!quickStart.includes(framework)) errors.push(`content/docs/getting-started/quick-start.mdx: missing ${framework} tab`)
}
if (/Home\.jsx/.test(quickStart)) errors.push('content/docs/getting-started/quick-start.mdx: quick-start React paths must use Home.tsx')

const deferredFile = path.join(contentRoot, 'guides/deferred-and-lazy-props.mdx')
const deferred = await fs.readFile(deferredFile, 'utf8')
for (const framework of ['Svelte', 'React', 'Vue']) {
  if (!deferred.includes(framework)) debt.push(`content/docs/guides/deferred-and-lazy-props.mdx: add the required ${framework} Deferred tab`)
}

const documentedVersion = new RegExp(workspaceVersion.replaceAll('.', '\\.'))
if (!documentedVersion.test(allDocumentation)) {
  errors.push(`documentation does not mention workspace version ${workspaceVersion}`)
}
if (!new RegExp(`Rust\\s+${msrv.replace('.', '\\.')}`).test(allDocumentation)) {
  errors.push(`documentation does not mention workspace MSRV Rust ${msrv}`)
}

for (const [dependency, expected] of Object.entries({
  '@inertiajs/svelte': packageVersion(frameworkManifests.svelte, '@inertiajs/svelte'),
  svelte: packageVersion(frameworkManifests.svelte, 'svelte'),
  '@inertiajs/react': packageVersion(frameworkManifests.react, '@inertiajs/react'),
  react: packageVersion(frameworkManifests.react, 'react'),
  '@inertiajs/vue3': packageVersion(frameworkManifests.vue, '@inertiajs/vue3'),
  vue: packageVersion(frameworkManifests.vue, 'vue'),
  vite: packageVersion(frameworkManifests.react, 'vite'),
})) {
  const escaped = dependency.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const mentions = [...allDocumentation.matchAll(new RegExp(`(?<![\\w/@-])${escaped}@([~^]?\\d+\\.\\d+\\.\\d+)`, 'g'))]
  for (const mention of mentions) {
    if (normalizedVersion(mention[1]) !== normalizedVersion(expected)) {
      errors.push(`documented ${dependency} version ${mention[1]} does not match maintained ${expected}`)
    }
  }
}

if (debt.length > 0) {
  console.warn(`Known content debt (${debt.length} actionable violations):\n${debt.map((item) => `- ${item}`).join('\n')}`)
}
if (errors.length > 0) {
  console.error(`Content consistency failures:\n${errors.map((item) => `- ${item}`).join('\n')}`)
  process.exitCode = 1
} else {
  console.log(`Validated versions, navigation, paths, framework parity, snippet sources, and ${files.length} MDX pages.`)
}
