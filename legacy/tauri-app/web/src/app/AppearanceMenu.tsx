// The Appearance popover: the same four knobs Settings owns, one click from
// the home field. It writes through the host (prefs + applyTheme), so the
// panel, Settings and the next boot all agree.

import { useEffect, useRef, useState } from "react";
import { IconChevronDown, IconDeviceLaptop, IconMoon, IconSun } from "@tabler/icons-react";

import { getHost, type Appearance } from "../bridge";

const BG_DARK = ["", "#0b0b0c", "#11141a", "#0f1512", "#191410"];
const BG_LIGHT = ["", "#ffffff", "#f7f7f8", "#f3f6fb", "#f4f9f5"];
const ASCII = ["", "#00ff99", "#5eead4", "#7dd3fc", "#fbbf24", "#f4f4f5"];
const TEXT_DARK = ["", "#f5f5f6", "#d4d4d8", "#a1a1aa", "#d1fae5"];
const TEXT_LIGHT = ["", "#18181b", "#3f3f46", "#52525b", "#166534"];

const THEMES = [
  { id: "system", label: "System", icon: IconDeviceLaptop },
  { id: "light", label: "Light", icon: IconSun },
  { id: "dark", label: "Dark", icon: IconMoon },
] as const;

function read(host: ReturnType<typeof getHost>): Appearance {
  const a = host.appearance() as Partial<Appearance> | undefined;
  return {
    accent: "",
    accent_dark: "",
    bg: "",
    fg: "",
    ascii: "",
    ui_font: "",
    code_font: "",
    translucent: true,
    contrast: 1.4,
    ...a,
  };
}

function Swatches({
  label,
  values,
  value,
  onPick,
  dark,
}: {
  label: string;
  values: string[];
  value: string;
  onPick: (v: string) => void;
  dark?: boolean;
}) {
  return (
    <div className="bs-ap-row">
      <span className="bs-ap-label">{label}</span>
      <div className="bs-ap-swatches" role="group" aria-label={label}>
        {values.map((v) => (
          <button
            key={v || "default"}
            type="button"
            aria-label={v || `Default ${label.toLowerCase()}`}
            aria-pressed={value === v}
            onClick={() => onPick(v)}
            className={`bs-ap-swatch ${v ? "" : "is-default"} ${dark ? "is-dark" : ""}`}
            style={v ? { background: v } : undefined}
          >
            {v ? null : <i aria-hidden />}
          </button>
        ))}
      </div>
    </div>
  );
}

export function AppearanceMenu() {
  const host = getHost();
  const [open, setOpen] = useState(false);
  const [appearance, setAppearance] = useState<Appearance>(() => read(host));
  const [theme, setTheme] = useState(() => host.theme());
  const [systemDark, setSystemDark] = useState(() => matchMedia("(prefers-color-scheme: dark)").matches);
  const box = useRef<HTMLDivElement>(null);

  // "System" has no colour of its own, so the swatch rows follow what the
  // shell resolved: dark text on light paper, light text on dark paper.
  useEffect(() => {
    const m = matchMedia("(prefers-color-scheme: dark)");
    const on = () => setSystemDark(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  const dark = theme === "dark" || (theme === "system" && systemDark);

  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const show = () => {
    setAppearance(read(host));
    setTheme(host.theme());
    setOpen((o) => !o);
  };

  const save = (patch: Partial<Appearance>) => {
    const next = { ...appearance, ...patch };
    setAppearance(next);
    void host.setAppearance(next);
  };

  const pickTheme = (t: "system" | "light" | "dark") => {
    setTheme(t);
    void host.setTheme(t);
  };

  const contrastOn = (appearance.contrast || 1.4) >= 1.15;
  const changed = !!(appearance.bg || appearance.fg || appearance.ascii);

  return (
    <div className="bs-ap" ref={box}>
      <button type="button" className="bs-ap-btn" aria-haspopup="dialog" aria-expanded={open} onClick={show}>
        <IconSun size={14} stroke={1.8} />
        Appearance
        <IconChevronDown size={12} className={open ? "rotate-180" : ""} />
      </button>
      {open && (
        <div className="bs-ap-panel" role="dialog" aria-label="Appearance">
          <div className="bs-ap-head">
            <span>Appearance</span>
            {changed && (
              <button type="button" className="bs-ap-reset" onClick={() => save({ bg: "", fg: "", ascii: "" })}>
                Reset
              </button>
            )}
          </div>

          <div className="bs-ap-row">
            <span className="bs-ap-label">Theme</span>
            <div className="bs-ap-themes" role="group" aria-label="Theme">
              {THEMES.map((t) => (
                <button
                  key={t.id}
                  type="button"
                  aria-label={t.label}
                  aria-pressed={theme === t.id}
                  onClick={() => pickTheme(t.id)}
                  className={`bs-ap-theme ${t.id} ${theme === t.id ? "is-on" : ""}`}
                  title={t.label}
                >
                  <t.icon size={13} stroke={1.8} />
                </button>
              ))}
            </div>
          </div>

          <Swatches label="Background" values={dark ? BG_DARK : BG_LIGHT} value={appearance.bg} onPick={(bg) => save({ bg })} dark={dark} />
          <Swatches label="ASCII" values={ASCII} value={appearance.ascii} onPick={(ascii) => save({ ascii })} />
          <Swatches label="Text" values={dark ? TEXT_DARK : TEXT_LIGHT} value={appearance.fg} onPick={(fg) => save({ fg })} />

          <div className="bs-ap-row bs-ap-last">
            <span className="bs-ap-label">High contrast</span>
            <button
              type="button"
              role="switch"
              aria-checked={contrastOn}
              aria-label="High contrast"
              onClick={() => save({ contrast: contrastOn ? 1 : 1.4 })}
              className={`bs-ap-switch ${contrastOn ? "is-on" : ""}`}
            >
              <i />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
