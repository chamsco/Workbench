// Voice mode in a bot's thread: talk, and it talks back. The bot's brain sits
// in the middle with its eyes turned into an equalizer that moves with the
// mic while you speak and with its own words while it answers.
//
// Hearing is the webview's speech recognition (WebKit on macOS, WebView2 on
// Windows; WebKitGTK has none, and then you type and it still speaks).
// Speaking is the system voice (speechSynthesis). Nothing leaves the machine
// that a typed message wouldn't send.

import { useEffect, useRef, useState } from "react";
import { IconMicrophone, IconX } from "@tabler/icons-react";

import { Orb, voiceLevel } from "./orb";
import type { Agent, Thread } from "../bridge";

type Phase = "listening" | "thinking" | "speaking" | "deaf";

type Recognition = {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  start(): void;
  stop(): void;
  abort(): void;
  onresult: ((e: { results: ArrayLike<ArrayLike<{ transcript: string }> & { isFinal: boolean }> }) => void) | null;
  onend: (() => void) | null;
  onerror: ((e: { error: string }) => void) | null;
};

const recognizer = (): Recognition | null => {
  const w = window as unknown as { SpeechRecognition?: new () => Recognition; webkitSpeechRecognition?: new () => Recognition };
  const R = w.SpeechRecognition ?? w.webkitSpeechRecognition;
  return R ? new R() : null;
};

/** Markdown read aloud: no fences, no symbols. */
const plain = (md: string) =>
  md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/[*_#>|~-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();

export function VoiceMode({ agent, thread, onSend, onClose }: { agent: Agent; thread: Thread; onSend: (text: string) => Promise<void>; onClose: () => void }) {
  const [phase, setPhase] = useState<Phase>("listening");
  const [heard, setHeard] = useState("");
  const rec = useRef<Recognition | null>(null);
  const sentAt = useRef(0);
  const spoken = useRef<string | null>(null);
  const alive = useRef(true);

  // The mic's loudness drives the eyes while listening.
  useEffect(() => {
    let stream: MediaStream | null = null;
    let ctx: AudioContext | null = null;
    let raf = 0;
    navigator.mediaDevices
      ?.getUserMedia({ audio: true })
      .then((s) => {
        if (!alive.current) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        ctx = new AudioContext();
        const an = ctx.createAnalyser();
        an.fftSize = 512;
        ctx.createMediaStreamSource(s).connect(an);
        const buf = new Uint8Array(an.fftSize);
        const loop = () => {
          an.getByteTimeDomainData(buf);
          let sum = 0;
          for (const b of buf) sum += ((b - 128) / 128) ** 2;
          if (phaseRef.current === "listening") voiceLevel.v = Math.min(1, Math.sqrt(sum / buf.length) * 6);
          raf = requestAnimationFrame(loop);
        };
        loop();
      })
      .catch(() => {
        /* no mic permission: the face keeps its own rhythm */
      });
    return () => {
      cancelAnimationFrame(raf);
      stream?.getTracks().forEach((t) => t.stop());
      void ctx?.close();
    };
  }, []);

  const phaseRef = useRef<Phase>(phase);
  phaseRef.current = phase;

  const listen = () => {
    const r = recognizer();
    if (!r) return setPhase("deaf");
    rec.current = r;
    r.lang = navigator.language || "en-US";
    r.interimResults = true;
    r.continuous = false;
    let final = "";
    r.onresult = (e) => {
      let interim = "";
      for (let i = 0; i < e.results.length; i++) {
        const res = e.results[i];
        if (res.isFinal) final += res[0].transcript;
        else interim += res[0].transcript;
      }
      setHeard((final + interim).trim());
    };
    r.onerror = (e) => {
      if (e.error === "not-allowed" || e.error === "service-not-allowed") setPhase("deaf");
    };
    r.onend = () => {
      if (!alive.current || phaseRef.current !== "listening") return;
      const text = final.trim();
      if (!text) return listen();
      sentAt.current = Date.now();
      setPhase("thinking");
      voiceLevel.v = -1;
      void onSend(text).catch(() => setPhase("listening"));
    };
    setPhase("listening");
    setHeard("");
    try {
      r.start();
    } catch {
      setPhase("deaf");
    }
  };

  useEffect(() => {
    alive.current = true;
    listen();
    return () => {
      alive.current = false;
      rec.current?.abort();
      speechSynthesis.cancel();
      voiceLevel.v = -1;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A finished reply that came after what you said is read aloud; then it
  // listens again. While deaf, any new reply is read.
  const last = thread.messages[thread.messages.length - 1];
  useEffect(() => {
    if (!last || last.role !== "assistant" || last.status !== "done" || !last.text) return;
    if (spoken.current === last.id) return;
    if (phaseRef.current === "listening" || (phaseRef.current !== "deaf" && last.at < sentAt.current - 1000)) return;
    spoken.current = last.id;
    const u = new SpeechSynthesisUtterance(plain(last.text));
    u.lang = navigator.language || "en-US";
    // The reply has no audio to measure: each word kicks the eyes, and they settle.
    let decay = 0;
    u.onboundary = () => {
      voiceLevel.v = 1;
      clearInterval(decay);
      decay = window.setInterval(() => {
        voiceLevel.v = Math.max(0.15, voiceLevel.v * 0.85);
      }, 40);
    };
    const done = () => {
      clearInterval(decay);
      voiceLevel.v = -1;
      if (!alive.current) return;
      if (phaseRef.current === "deaf") return setPhase("deaf");
      listen();
    };
    u.onend = done;
    u.onerror = done;
    const was = phaseRef.current;
    setPhase(was === "deaf" ? "deaf" : "speaking");
    speechSynthesis.speak(u);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [last?.id, last?.status]);

  const line =
    phase === "listening"
      ? heard || "Listening…"
      : phase === "thinking"
        ? `${agent.name} is thinking…`
        : phase === "speaking"
          ? `${agent.name} is talking`
          : "This window can't hear you: its webview has no speech recognition. Type below, and replies are read aloud.";

  return (
    <div className="voice-mode absolute inset-x-0 top-0 bottom-24 z-20 flex flex-col items-center justify-center gap-6 bg-surface/85 backdrop-blur-md">
      <Orb agent={agent} size={200} state="voice" />
      <p className="max-w-md px-6 text-center text-[15px] text-foreground" aria-live="polite">
        {line}
      </p>
      <div className="flex gap-2">
        {phase === "speaking" && (
          <button
            type="button"
            onClick={() => {
              speechSynthesis.cancel();
            }}
            className="inline-flex h-9 items-center gap-1.5 rounded-full bg-well px-4 text-[13px] font-medium"
          >
            <IconMicrophone className="size-4" /> Interrupt
          </button>
        )}
        <button type="button" onClick={onClose} className="inline-flex h-9 items-center gap-1.5 rounded-full bg-well px-4 text-[13px] font-medium">
          <IconX className="size-4" /> End voice
        </button>
      </div>
    </div>
  );
}
