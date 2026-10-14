use anyhow::{Context, Result};
use std::process::Command;

/// Run a PowerShell snippet, return stdout trimmed. Errors carry stderr.
pub fn ps(script: &str) -> Result<String> {
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .context("failed to launch powershell.exe")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        anyhow::bail!("powershell failed ({}): {}{}", out.status, stderr.trim(), stdout.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Run PowerShell and parse stdout as JSON.
pub fn ps_json<T: serde::de::DeserializeOwned>(script: &str) -> Result<Option<T>> {
    let s = ps(script)?;
    if s.is_empty() {
        return Ok(None);
    }
    let v = serde_json::from_str(&s).with_context(|| format!("bad JSON from powershell: {s}"))?;
    Ok(Some(v))
}

pub fn elevated() -> bool {
    ps("([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn hyperv_present() -> bool {
    ps("(Get-Service vmms -ErrorAction SilentlyContinue) -ne $null")
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Escape a string for embedding in a single-quoted PowerShell literal.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
