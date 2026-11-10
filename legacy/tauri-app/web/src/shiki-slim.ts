// A smaller `shiki` for @streamdown/code: the same three exports it
// imports, but ~25 common languages instead of every grammar shiki ships
// (which added ~10 MB of chunks to the app). Others render as plain text.

import { createHighlighterCore, type LanguageInput, type ThemeInput } from "shiki/core";

const LANGS: Record<string, () => Promise<unknown>> = {
  bash: () => import("shiki/langs/bash.mjs"),
  c: () => import("shiki/langs/c.mjs"),
  cpp: () => import("shiki/langs/cpp.mjs"),
  csharp: () => import("shiki/langs/csharp.mjs"),
  css: () => import("shiki/langs/css.mjs"),
  diff: () => import("shiki/langs/diff.mjs"),
  docker: () => import("shiki/langs/docker.mjs"),
  go: () => import("shiki/langs/go.mjs"),
  html: () => import("shiki/langs/html.mjs"),
  java: () => import("shiki/langs/java.mjs"),
  javascript: () => import("shiki/langs/javascript.mjs"),
  json: () => import("shiki/langs/json.mjs"),
  jsx: () => import("shiki/langs/jsx.mjs"),
  kotlin: () => import("shiki/langs/kotlin.mjs"),
  lua: () => import("shiki/langs/lua.mjs"),
  markdown: () => import("shiki/langs/markdown.mjs"),
  php: () => import("shiki/langs/php.mjs"),
  python: () => import("shiki/langs/python.mjs"),
  ruby: () => import("shiki/langs/ruby.mjs"),
  rust: () => import("shiki/langs/rust.mjs"),
  sql: () => import("shiki/langs/sql.mjs"),
  swift: () => import("shiki/langs/swift.mjs"),
  toml: () => import("shiki/langs/toml.mjs"),
  tsx: () => import("shiki/langs/tsx.mjs"),
  typescript: () => import("shiki/langs/typescript.mjs"),
  xml: () => import("shiki/langs/xml.mjs"),
  yaml: () => import("shiki/langs/yaml.mjs"),
};

const ALIASES: Record<string, string[]> = {
  bash: ["sh", "shell", "zsh", "console"],
  cpp: ["c++"],
  csharp: ["cs", "c#"],
  docker: ["dockerfile"],
  javascript: ["js", "mjs", "cjs"],
  markdown: ["md"],
  python: ["py"],
  ruby: ["rb"],
  rust: ["rs"],
  typescript: ["ts", "mts", "cts"],
  yaml: ["yml"],
};

const THEMES: Record<string, () => Promise<unknown>> = {
  "github-light": () => import("shiki/themes/github-light.mjs"),
  "github-dark": () => import("shiki/themes/github-dark.mjs"),
  "github-dark-default": () => import("shiki/themes/github-dark-default.mjs"),
};

export const bundledLanguages = LANGS;
export const bundledLanguagesInfo = Object.keys(LANGS).map((id) => ({ id, name: id, aliases: ALIASES[id] ?? [] }));

export function createHighlighter(opts: { themes: (string | ThemeInput)[]; langs: string[]; engine: never }) {
  return createHighlighterCore({
    themes: opts.themes.map((t) => (typeof t === "string" ? (THEMES[t] ?? THEMES["github-light"])() : t)) as ThemeInput[],
    langs: opts.langs.filter((l) => l in LANGS).map((l) => LANGS[l]()) as LanguageInput[],
    engine: opts.engine,
  });
}
