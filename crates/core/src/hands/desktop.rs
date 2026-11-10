//! Desktop use: a screenshot of the main display for the agent's own
//! (vision) model, and the mouse and keyboard. Nothing here decides
//! anything: the agent looks and says where.
//!
//! Screenshots are scaled to at most [`MAX_W`] wide and the agent clicks in
//! that picture's pixels; `screen.json` keeps the scale so a click lands on
//! the same spot whatever the display's size or DPI.
//!
//! macOS needs Screen Recording and Accessibility for the app (System
//! Settings → Privacy & Security); without them the picture is the desktop
//! wallpaper only and input is ignored.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use image::GenericImageView;
use serde_json::{json, Value};

pub const MAX_W: u32 = 1280;

/// The main display, as PNG bytes at its native resolution.
fn grab() -> Result<Vec<u8>> {
    #[cfg(target_os = "macos")]
    {
        let f = std::env::temp_dir().join(format!("backspace-shot-{}.png", std::process::id()));
        let ok = std::process::Command::new("screencapture").args(["-x", "-m", "-t", "png"]).arg(&f).status()?.success();
        let bytes = std::fs::read(&f);
        let _ = std::fs::remove_file(&f);
        if !ok {
            bail!("screencapture failed; give Backspace Screen Recording in System Settings → Privacy & Security");
        }
        return Ok(bytes?);
    }
    #[cfg(windows)]
    {
        let m = xcap::Monitor::all()?.into_iter().find(|m| m.is_primary().unwrap_or(false)).ok_or_else(|| anyhow!("no display"))?;
        let img = m.capture_image().map_err(|e| anyhow!("screenshot failed: {e}"))?;
        let mut out = std::io::Cursor::new(vec![]);
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(img.width(), img.height(), img.into_raw()).ok_or_else(|| anyhow!("bad screenshot"))?)
            .write_to(&mut out, image::ImageFormat::Png)?;
        return Ok(out.into_inner());
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        // Linux: whichever screenshot tool is there (Wayland's grim, then X11's).
        let tools: [(&str, &[&str]); 3] = [("grim", &["-t", "png", "-"]), ("import", &["-window", "root", "png:-"]), ("maim", &[])];
        for (exe, args) in tools {
            if let Ok(o) = std::process::Command::new(exe).args(args).output() {
                if o.status.success() && o.stdout.starts_with(b"\x89PNG") {
                    return Ok(o.stdout);
                }
            }
        }
        bail!("no screenshot tool found; install grim (Wayland) or ImageMagick (X11)")
    }
}

/// The display's size in input coordinates (points on a Mac).
/// The titles of the windows that are open, as text, for models whose backend
/// can't look at the picture: they can still say what is on the screen.
pub fn open_windows() -> Option<String> {
    use std::process::Command;
    let out = if cfg!(target_os = "windows") {
        Command::new("powershell").args(["-NoProfile", "-Command", "Get-Process | Where-Object { $_.MainWindowTitle } | ForEach-Object { $_.ProcessName + ': ' + $_.MainWindowTitle }"]).output().ok()?
    } else if cfg!(target_os = "macos") {
        Command::new("osascript").args(["-e", "tell application \"System Events\" to get name of (processes where background only is false)"]).output().ok()?
    } else {
        Command::new("wmctrl").arg("-l").output().ok()?
    };
    let text = String::from_utf8_lossy(&out.stdout).replace(", ", "\n");
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).take(40).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn display() -> Result<(i32, i32)> {
    let e = Enigo::new(&Settings::default()).map_err(|e| anyhow!("can't reach the mouse and keyboard: {e}"))?;
    e.main_display().map_err(|e| anyhow!("{e}"))
}

/// Take a screenshot scaled for the model; returns (PNG bytes, its size), and
/// writes the scale to `dir/screen.json` for the clicks that follow.
pub fn screenshot(dir: &Path) -> Result<(Vec<u8>, u32, u32)> {
    let raw = grab()?;
    let img = image::load_from_memory(&raw).context("reading the screenshot")?;
    let (w, h) = img.dimensions();
    let img = if w > MAX_W { img.resize(MAX_W, (h as u64 * MAX_W as u64 / w as u64) as u32, image::imageops::FilterType::Triangle) } else { img };
    let (sw, sh) = img.dimensions();
    let (dw, dh) = display().unwrap_or((w as i32, h as i32));
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("screen.json"), json!({"shot": [sw, sh], "display": [dw, dh]}).to_string())?;
    let mut png = std::io::Cursor::new(vec![]);
    img.write_to(&mut png, image::ImageFormat::Png)?;
    // The live view takes a smaller JPEG.
    let mut jpg = std::io::Cursor::new(vec![]);
    if img.to_rgb8().write_to(&mut jpg, image::ImageFormat::Jpeg).is_ok() {
        let _ = std::fs::write(dir.join("desktop.jpg"), jpg.into_inner());
    }
    Ok((png.into_inner(), sw, sh))
}

/// Map a point in the last screenshot to input coordinates.
fn to_screen(dir: &Path, x: f64, y: f64) -> Result<(i32, i32)> {
    let s: Value = std::fs::read_to_string(dir.join("screen.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or_else(|| anyhow!("take a desktop_screenshot first, then click in its pixels"))?;
    let f = |i: usize, k: &str| s[k][i].as_f64().unwrap_or(1.0).max(1.0);
    let (sx, sy) = (f(0, "display") / f(0, "shot"), f(1, "display") / f(1, "shot"));
    if x < 0.0 || y < 0.0 || x > f(0, "shot") || y > f(1, "shot") {
        bail!("({x}, {y}) is outside the {}×{} screenshot", f(0, "shot"), f(1, "shot"));
    }
    Ok(((x * sx).round() as i32, (y * sy).round() as i32))
}

fn enigo() -> Result<Enigo> {
    Enigo::new(&Settings::default()).map_err(|e| anyhow!("can't reach the mouse and keyboard ({e}); on a Mac, give Backspace Accessibility in System Settings → Privacy & Security"))
}

pub fn click(dir: &Path, x: f64, y: f64, button: &str, double: bool) -> Result<String> {
    let (sx, sy) = to_screen(dir, x, y)?;
    let mut e = enigo()?;
    e.move_mouse(sx, sy, Coordinate::Abs).map_err(|e| anyhow!("{e}"))?;
    let b = match button {
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => Button::Left,
    };
    for _ in 0..if double { 2 } else { 1 } {
        e.button(b, Direction::Click).map_err(|e| anyhow!("{e}"))?;
    }
    Ok(format!("{}clicked {button} at ({x}, {y})", if double { "double-" } else { "" }))
}

pub fn type_text(text: &str) -> Result<String> {
    if text.is_empty() {
        bail!("nothing to type");
    }
    enigo()?.text(text).map_err(|e| anyhow!("{e}"))?;
    Ok(format!("typed {} characters", text.chars().count()))
}

pub fn scroll(dy: i32) -> Result<String> {
    enigo()?.scroll(dy, Axis::Vertical).map_err(|e| anyhow!("{e}"))?;
    Ok(format!("scrolled {dy}"))
}

/// A key or chord: "enter", "cmd+c", "ctrl+shift+t", "f5", "a".
pub fn key(chord: &str) -> Result<String> {
    let parts: Vec<String> = chord.split('+').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect();
    let Some((last, mods)) = parts.split_last() else { bail!("say which key") };
    let mods: Vec<Key> = mods.iter().map(|m| parse_key(m)).collect::<Result<_>>()?;
    let k = parse_key(last)?;
    let mut e = enigo()?;
    for m in &mods {
        e.key(*m, Direction::Press).map_err(|e| anyhow!("{e}"))?;
    }
    let r = e.key(k, Direction::Click).map_err(|e| anyhow!("{e}"));
    for m in mods.iter().rev() {
        let _ = e.key(*m, Direction::Release);
    }
    r?;
    Ok(format!("pressed {chord}"))
}

fn parse_key(k: &str) -> Result<Key> {
    Ok(match k {
        "cmd" | "command" | "meta" | "super" | "win" => Key::Meta,
        "ctrl" | "control" => Key::Control,
        "alt" | "option" | "opt" => Key::Alt,
        "shift" => Key::Shift,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "esc" | "escape" => Key::Escape,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        s if s.chars().count() == 1 => Key::Unicode(s.chars().next().unwrap()),
        s => bail!("unknown key {s:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_land_in_display_coordinates() {
        let d = std::env::temp_dir().join(format!("bs-desk-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("screen.json"), json!({"shot": [1280, 800], "display": [1440, 900]}).to_string()).unwrap();
        assert_eq!(to_screen(&d, 640.0, 400.0).unwrap(), (720, 450));
        assert!(to_screen(&d, 1300.0, 10.0).is_err());
        assert!(parse_key("cmd").is_ok() && parse_key("x").is_ok() && parse_key("hyper").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
