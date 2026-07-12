import { promises as fs } from 'node:fs'
import path from 'node:path'

type SnippetProps = {
  language: string
  source: string
  title: string
  from?: number
  to?: number
}

export async function Snippet({ language, source, title, from, to }: SnippetProps) {
  const snippetsRoot = path.resolve(process.cwd(), 'snippets')
  const file = path.resolve(snippetsRoot, source)

  if (file !== snippetsRoot && !file.startsWith(`${snippetsRoot}${path.sep}`)) {
    throw new Error(`Snippet source must stay inside docs/snippets: ${source}`)
  }

  const contents = await fs.readFile(file, 'utf8')
  const lines = contents.replace(/\n$/, '').split('\n')
  const first = from ?? 1
  const last = to ?? lines.length

  if (!Number.isInteger(first) || !Number.isInteger(last) || first < 1 || last < first || last > lines.length) {
    throw new Error(`Invalid snippet line range ${first}-${last} for ${source}`)
  }

  return (
    <figure className="my-4 overflow-hidden rounded-lg border bg-fd-card">
      <figcaption className="border-b px-4 py-2 font-mono text-xs text-fd-muted-foreground">
        {title}
      </figcaption>
      <pre className="overflow-x-auto p-4 text-sm">
        <code className={`language-${language}`}>{lines.slice(first - 1, last).join('\n')}</code>
      </pre>
    </figure>
  )
}
