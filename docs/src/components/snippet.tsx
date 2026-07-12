import { promises as fs } from "node:fs";
import path from "node:path";
import {
  ServerCodeBlock,
  type ServerCodeBlockProps,
} from "fumadocs-ui/components/codeblock.rsc";

type SnippetProps = {
  language: ServerCodeBlockProps["lang"];
  source: string;
  title: string;
  from?: number;
  to?: number;
};

export async function Snippet({
  language,
  source,
  title,
  from,
  to,
}: SnippetProps) {
  const snippetsRoot = path.resolve(process.cwd(), "snippets");
  const file = path.resolve(snippetsRoot, source);
  const relativePath = path.relative(snippetsRoot, file);

  // Prevent absolute paths and ../ traversal outside snippets/.
  if (
    relativePath === "" ||
    relativePath.startsWith("..") ||
    path.isAbsolute(relativePath)
  ) {
    throw new Error(`Snippet source must stay inside snippets/: ${source}`);
  }

  const contents = await fs.readFile(file, "utf8");

  // Normalize Windows line endings and remove one final newline.
  const normalized = contents.replace(/\r\n?/g, "\n");
  const lines = (
    normalized.endsWith("\n") ? normalized.slice(0, -1) : normalized
  ).split("\n");

  const first = from ?? 1;
  const last = to ?? lines.length;

  if (
    !Number.isInteger(first) ||
    !Number.isInteger(last) ||
    first < 1 ||
    last < first ||
    last > lines.length
  ) {
    throw new Error(
      `Invalid snippet line range ${first}-${last} for ${source}`,
    );
  }

  const code = lines.slice(first - 1, last).join("\n");

  return (
    <ServerCodeBlock
      lang={language}
      code={code}
      codeblock={{
        title,
        className: "my-4",
      }}
    />
  );
}
