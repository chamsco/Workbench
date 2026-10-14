use crate::model::{data_dir, CreateSpec, Workspace};
use crate::powershell::{ps, ps_json, quote};
use crate::providers::Provider;
use anyhow::{bail, Result};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const PREFIX: &str = "visor-";
const SWITCH: &str = "Default Switch";

pub struct HyperVProvider;

impl HyperVProvider {
    fn vm_name(name: &str) -> Result<String> {
        validate_name(name)?;
        Ok(format!("{PREFIX}{name}"))
    }

    fn ws_dir(name: &str) -> Result<PathBuf> {
        Ok(data_dir()?.join("hyperv").join(name))
    }

    fn require_ready(&self) -> Result<()> {
        if !crate::powershell::hyperv_present() {
            bail!("Hyper-V is not enabled. Run (admin, then reboot):\n  Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V -All");
        }
        if !crate::powershell::elevated() {
            bail!("Hyper-V operations require elevation — run visor-core/agentd as Administrator");
        }
        Ok(())
    }

    /// Build the cloud-init seed disk: fixed VHD, FAT32, volume label CIDATA.
    fn build_seed_vhd(&self, spec: &CreateSpec) -> Result<PathBuf> {
        let dir = Self::ws_dir(&spec.name)?;
        let files = dir.join("seed-files");
        crate::seed::write_seed_files(spec, &files)?;
        let vhd = dir.join("seed.vhdx");
        if vhd.exists() {
            std::fs::remove_file(&vhd)?;
        }
        ps(&format!(
            r#"
$ErrorActionPreference = 'Stop'
New-VHD -Path {vhd} -SizeBytes 64MB -Fixed | Out-Null
$disk = Mount-VHD -Path {vhd} -Passthru | Get-Disk
Initialize-Disk -Number $disk.Number -PartitionStyle MBR -Confirm:$false
Start-Sleep -Milliseconds 500
$part = New-Partition -DiskNumber $disk.Number -UseMaximumSize -AssignDriveLetter
Format-Volume -DriveLetter $part.DriveLetter -FileSystem FAT32 -NewFileSystemLabel 'CIDATA' -Confirm:$false | Out-Null
Copy-Item (Join-Path {files} '*') ($part.DriveLetter + ':\') -Force
Dismount-VHD -Path {vhd}
"#,
            vhd = quote(&vhd.to_string_lossy()),
            files = quote(&files.to_string_lossy()),
        ))?;
        Ok(vhd)
    }

    fn wait_ip(&self, vm: &str, timeout: Duration) -> Result<Option<String>> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            let ip = ps(&format!(
                "((Get-VMNetworkAdapter -VMName {}).IPAddresses | Where-Object {{ $_ -match '^\\d+\\.' }} | Select-Object -First 1)",
                quote(vm)
            ))?;
            if !ip.is_empty() {
                return Ok(Some(ip));
            }
            std::thread::sleep(Duration::from_secs(3));
        }
        Ok(None)
    }
}

fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && name.chars().next().unwrap().is_ascii_alphanumeric();
    if !ok {
        bail!("invalid workspace name '{name}' (lowercase alnum + '-', max 32)");
    }
    Ok(())
}

impl Provider for HyperVProvider {
    fn name(&self) -> &'static str {
        "hyperv"
    }

    fn available(&self) -> bool {
        crate::powershell::hyperv_present()
    }

    fn create(&self, spec: &CreateSpec) -> Result<Workspace> {
        self.require_ready()?;
        let vm = Self::vm_name(&spec.name)?;
        if ps(&format!("(Get-VM -Name {} -ErrorAction SilentlyContinue) -ne $null", quote(&vm)))?
            .eq_ignore_ascii_case("true")
        {
            bail!("workspace '{}' already exists", spec.name);
        }
        if spec.os.starts_with("windows") {
            bail!("Windows guests are not supported yet (phase 1 ships Linux only)");
        }

        let dir = Self::ws_dir(&spec.name)?;
        std::fs::create_dir_all(&dir)?;
        let base = crate::images::ensure_hyperv_base(&spec.os)?;
        let vhdx = dir.join("disk.vhdx");
        let seed = self.build_seed_vhd(spec)?;

        ps(&format!(
            r#"
$ErrorActionPreference = 'Stop'
New-VHD -Path {disk} -ParentPath {base} -Differencing | Out-Null
New-VM -Name {vm} -Generation 2 -MemoryStartupBytes {mem}MB -VHDPath {disk} -SwitchName {switch} | Out-Null
Set-VMProcessor -VMName {vm} -Count {cpus}
Set-VMFirmware -VMName {vm} -EnableSecureBoot Off
Add-VMHardDiskDrive -VMName {vm} -Path {seed}
Set-VM -VMName {vm} -AutomaticStopAction ShutDown -AutomaticCheckpointsEnabled $false
"#,
            disk = quote(&vhdx.to_string_lossy()),
            base = quote(&base.to_string_lossy()),
            vm = quote(&vm),
            mem = spec.memory_mib,
            cpus = spec.cpus,
            switch = quote(SWITCH),
            seed = quote(&seed.to_string_lossy()),
        ))?;

        Ok(Workspace {
            name: spec.name.clone(),
            provider: "hyperv".into(),
            os: spec.os.clone(),
            state: "off".into(),
            ip: None,
            ssh_user: Some("visor".into()),
            ssh_port: Some(22),
            cpus: spec.cpus,
            memory_mib: spec.memory_mib,
            disk_gib: spec.disk_gib,
        })
    }

    fn start(&self, name: &str) -> Result<Workspace> {
        self.require_ready()?;
        let vm = Self::vm_name(name)?;
        ps(&format!("Start-VM -Name {}", quote(&vm)))?;
        let ip = self.wait_ip(&vm, Duration::from_secs(240))?;
        let mut ws = self.status(name)?;
        ws.ip = ip.or(ws.ip);
        Ok(ws)
    }

    fn stop(&self, name: &str, force: bool) -> Result<Workspace> {
        self.require_ready()?;
        let vm = Self::vm_name(name)?;
        if force {
            ps(&format!("Stop-VM -Name {} -Force", quote(&vm)))?;
        } else {
            ps(&format!("Stop-VM -Name {}", quote(&vm)))?;
        }
        self.status(name)
    }

    fn delete(&self, name: &str) -> Result<()> {
        self.require_ready()?;
        let vm = Self::vm_name(name)?;
        ps(&format!(
            "Stop-VM -Name {0} -Force -ErrorAction SilentlyContinue; Remove-VM -Name {0} -Force -ErrorAction SilentlyContinue",
            quote(&vm)
        ))?;
        let dir = Self::ws_dir(name)?;
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    fn list(&self) -> Result<Vec<Workspace>> {
        if !self.available() {
            return Ok(vec![]);
        }
        #[derive(serde::Deserialize)]
        struct VmRow {
            #[serde(rename = "Name")]
            name: String,
            #[serde(rename = "State")]
            state: serde_json::Value,
        }
        let rows: Vec<VmRow> = ps_json(&format!(
            "Get-VM -Name '{}*' -ErrorAction SilentlyContinue | Select-Object Name,State | ConvertTo-Json -Compress",
            PREFIX
        ))?
        .unwrap_or_default();
        let mut out = Vec::new();
        for row in rows {
            if let Some(name) = row.name.strip_prefix(PREFIX) {
                out.push(self.status(name)?);
            }
        }
        Ok(out)
    }

    fn status(&self, name: &str) -> Result<Workspace> {
        let vm = Self::vm_name(name)?;
        let mut ws = crate::model::load_meta(name)?;
        #[derive(serde::Deserialize)]
        struct VmState {
            #[serde(rename = "State")]
            state: serde_json::Value,
        }
        let state: Option<VmState> = ps_json(&format!(
            "Get-VM -Name {} -ErrorAction SilentlyContinue | Select-Object State | ConvertTo-Json -Compress",
            quote(&vm)
        ))?;
        ws.state = match state {
            None => "missing".into(),
            Some(s) => match s.state {
                serde_json::Value::String(s) => s.to_lowercase(),
                // ConvertTo-Json serializes the enum as its numeric value sometimes
                serde_json::Value::Number(n) => match n.as_u64().unwrap_or(0) {
                    2 => "running".into(),
                    3 => "off".into(),
                    9 => "paused".into(),
                    6 => "saved".into(),
                    _ => "unknown".into(),
                },
                _ => "unknown".into(),
            },
        };
        if ws.state == "running" {
            ws.ip = ps(&format!(
                "((Get-VMNetworkAdapter -VMName {}).IPAddresses | Where-Object {{ $_ -match '^\\d+\\.' }} | Select-Object -First 1)",
                quote(&vm)
            ))
            .ok()
            .filter(|s| !s.is_empty());
        } else {
            ws.ip = None;
        }
        Ok(ws)
    }
}

impl HyperVProvider {
    /// Current guest IP (None if not running or not yet reported).
    pub fn ip(&self, name: &str) -> Result<Option<String>> {
        let vm = Self::vm_name(name)?;
        Ok(ps(&format!(
            "((Get-VMNetworkAdapter -VMName {}).IPAddresses | Where-Object {{ $_ -match '^\\d+\\.' }} | Select-Object -First 1)",
            quote(&vm)
        ))
        .ok()
        .filter(|s| !s.is_empty()))
    }
}
