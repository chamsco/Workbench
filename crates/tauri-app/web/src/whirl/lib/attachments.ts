// From Whirl (MIT, see ../LICENSE): the pure helpers of lib/attachments.ts.
// The upload half talked to Convex; Backspace sends attachments with the
// message instead (see ../../bridge.ts).

import { DOCUMENT_MIME_TYPES_BY_EXTENSION } from "@/lib/document-formats";


/* v2's port of v1's attachment pipeline (apps/legacy/app/lib/
   attachment-upload.ts): type detection, the free-plan cap, image
   compression, document→Markdown conversion, and the XHR upload to Convex
   storage that reports real progress. Files upload the moment they're
   picked — send just waits for whatever's still in flight. */

export const MB = 1024 * 1024;

/* Mirrors FREE_MAX_FILE_BYTES in convex/inference/billing.ts — the server
   enforces it too; this is the friendly front door. */
export const FREE_MAX_FILE_BYTES = 1 * MB;

/* Per-file ceiling on any plan, matching v1's per-model caps (all 20 MB). */
const MAX_FILE_BYTES = 20 * MB;

const TARGET_COMPRESSED_BYTES = 4 * MB;

/* One conservative constraint set for every model — v1 varies these per
   tier, but 1568px / 4 MP is the strictest working bound and looks
   identical in practice. */
const IMAGE_MAX_LONG_EDGE = 1568;
const IMAGE_MAX_MEGAPIXELS = 4;

const COMPRESSIBLE_IMAGE_TYPES = new Set([
  "image/jpeg",
  "image/png",
  "image/webp",
  "image/bmp",
  "image/tiff",
]);

const TEXT_ATTACHMENT_EXTENSIONS = new Set([
  "c",
  "conf",
  "cpp",
  "cs",
  "css",
  "csv",
  "dart",
  "diff",
  "env",
  "go",
  "h",
  "html",
  "htm",
  "ini",
  "java",
  "js",
  "json",
  "jsonc",
  "jsx",
  "kt",
  "kts",
  "log",
  "lua",
  "md",
  "mdx",
  "patch",
  "php",
  "py",
  "r",
  "rb",
  "rs",
  "scala",
  "sh",
  "sql",
  "srt",
  "svelte",
  "svg",
  "swift",
  "toml",
  "ts",
  "tsx",
  "tsv",
  "txt",
  "vue",
  "vtt",
  "xml",
  "yaml",
  "yml",
]);

const MIME_TYPES_BY_EXTENSION: Record<string, string> = {
  ...DOCUMENT_MIME_TYPES_BY_EXTENSION,
  conf: "text/plain",
  csv: "text/csv",
  diff: "text/plain",
  htm: "text/html",
  html: "text/html",
  // Browsers report an empty type for .jsonc — without this it lands on
  // application/octet-stream and the model never gets the text.
  jsonc: "text/plain",
  log: "text/plain",
  md: "text/markdown",
  mdx: "text/markdown",
  patch: "text/plain",
  sql: "application/sql",
  svg: "text/xml",
  toml: "application/toml",
  tsv: "text/tab-separated-values",
  txt: "text/plain",
  xml: "application/xml",
  yaml: "application/yaml",
  yml: "application/yaml",
};

export function formatSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < MB) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / MB).toFixed(1)} MB`;
}

/** The fields the metadata/validation helpers need — a `File` and a draft
 * both satisfy this, so the same checks run before and after upload. */
export type FileLike = { name: string; type: string; size: number };

export function getFileExtension(name: string) {
  return name.includes(".") ? (name.split(".").pop()?.toLowerCase() ?? "") : "";
}

/* An SVG arrives from the OS as image/svg+xml, but it's markup, not
   pixels — no provider decodes it as an image, and handing over the source
   is what's actually useful ("tweak this icon"). Every image path in the
   app, client and server, keys off the type, so correcting it here is the
   whole story: no thumbnail, no @image tag, no vision-only gating, and it
   rides inline as text like any other code file. */
const TYPE_OVERRIDES: Record<string, string> = {
  "image/svg+xml": "text/xml",
};

export function getAttachmentType(file: Pick<FileLike, "name" | "type">) {
  const type =
    file.type ||
    MIME_TYPES_BY_EXTENSION[getFileExtension(file.name)] ||
    "application/octet-stream";
  return TYPE_OVERRIDES[type] ?? type;
}

export function isTextAttachment(file: Pick<FileLike, "name" | "type">) {
  return (
    getAttachmentType(file).startsWith("text/") ||
    TEXT_ATTACHMENT_EXTENSIONS.has(getFileExtension(file.name))
  );
}

export function makeAttachmentId() {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

/**
 * Why a file can't ride along with the chosen model, or `null` if it's
 * fine. Reactive to the selected model — the same file may be welcome on
 * one and rejected on another, so chips re-check on every model switch.
 */
