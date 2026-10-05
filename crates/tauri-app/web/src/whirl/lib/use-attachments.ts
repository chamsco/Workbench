// Backspace's take on Whirl's lib/use-attachments.ts (MIT, see ../LICENSE):
// the same draft shape the composer tray renders, but files are read in
// the window and sent with the message instead of uploaded to Convex.

import { useCallback, useRef, useState } from "react";

import { makeAttachmentId } from "@/lib/attachments";

export type AttachmentDraft = {
  id: string;
  name: string;
  size: number;
  type: string;
  status: "uploading" | "reading" | "ready" | "error";
  progress: number;
  previewUrl?: string;
  error?: string;
  /** The file as a data: URL, sent with the message. */
  data?: string;
};

const MAX_BYTES = 10 * 1024 * 1024;

export function useAttachments() {
  const [drafts, setDrafts] = useState<AttachmentDraft[]>([]);
  const ref = useRef<AttachmentDraft[]>([]);
  const commit = useCallback((f: (d: AttachmentDraft[]) => AttachmentDraft[]) => {
    setDrafts((prev) => {
      const next = f(prev);
      ref.current = next;
      return next;
    });
  }, []);

  const addFiles = useCallback(
    (files: FileList | File[]) => {
      for (const file of Array.from(files)) {
        const id = makeAttachmentId();
        const type = file.type || "application/octet-stream";
        const tooBig = file.size > MAX_BYTES;
        const draft: AttachmentDraft = {
          id,
          name: file.name,
          size: file.size,
          type,
          status: tooBig ? "error" : "reading",
          progress: 0.3,
          previewUrl: type.startsWith("image/") ? URL.createObjectURL(file) : undefined,
          error: tooBig ? "Files are limited to 10 MB." : undefined,
        };
        commit((d) => [...d, draft]);
        if (tooBig) continue;
        const reader = new FileReader();
        reader.onload = () =>
          commit((d) =>
            d.map((x) => (x.id === id ? { ...x, status: "ready", progress: 1, data: String(reader.result) } : x)),
          );
        reader.onerror = () =>
          commit((d) => d.map((x) => (x.id === id ? { ...x, status: "error", error: "Couldn't read the file." } : x)));
        reader.readAsDataURL(file);
      }
    },
    [commit],
  );

  const remove = useCallback(
    (id: string) =>
      commit((d) => {
        const gone = d.find((x) => x.id === id);
        if (gone?.previewUrl) URL.revokeObjectURL(gone.previewUrl);
        return d.filter((x) => x.id !== id);
      }),
    [commit],
  );

  const clear = useCallback(() => {
    ref.current.forEach((x) => x.previewUrl && URL.revokeObjectURL(x.previewUrl));
    commit(() => []);
  }, [commit]);

  return { drafts, addFiles, remove, clear, current: () => ref.current };
}
