import { readFile } from 'node:fs/promises'
import path from 'node:path'
import ts from 'typescript'
import { compile as compileSvelte } from 'svelte/compiler'
import { compileScript, compileTemplate, parse } from '@vue/compiler-sfc'

const root = process.cwd()
const failures = []

async function source(relative) {
  return readFile(path.join(root, 'snippets', relative), 'utf8')
}

function fail(file, error) {
  failures.push(`${file}: ${error instanceof Error ? error.message : String(error)}`)
}

try {
  const file = 'react/src/Pages/Home.tsx'
  const result = ts.transpileModule(await source(file), {
    compilerOptions: {
      jsx: ts.JsxEmit.ReactJSX,
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: file,
    reportDiagnostics: true,
  })
  for (const diagnostic of result.diagnostics ?? []) {
    fail(file, ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'))
  }
} catch (error) {
  fail('react/src/Pages/Home.tsx', error)
}

try {
  const file = 'svelte/src/Pages/Home.svelte'
  compileSvelte(await source(file), { filename: file, generate: 'client' })
} catch (error) {
  fail('svelte/src/Pages/Home.svelte', error)
}

try {
  const file = 'vue/src/Pages/Home.vue'
  const parsed = parse(await source(file), { filename: file })
  for (const error of parsed.errors) fail(file, error)
  if (parsed.descriptor.scriptSetup) compileScript(parsed.descriptor, { id: file })
  if (parsed.descriptor.template) {
    const result = compileTemplate({
      filename: file,
      id: file,
      source: parsed.descriptor.template.content,
    })
    for (const error of result.errors) fail(file, error)
  }
} catch (error) {
  fail('vue/src/Pages/Home.vue', error)
}

if (failures.length > 0) {
  console.error(failures.map((failure) => `- ${failure}`).join('\n'))
  process.exitCode = 1
} else {
  console.log('Compiled the Svelte, React, and Vue documentation snippets.')
}
