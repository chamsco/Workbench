use crate::model::CreateSpec;
use anyhow::Result;
use std::path::PathBuf;

/// Render cloud-init NoCloud seed files into `dir` (user-data, meta-data).
/// The caller is responsible for packing them onto a disk labeled CIDATA.
pub fn write_seed_files(spec: &CreateSpec, dir: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(dir)?;

    let key_line = match &spec.ssh_public_key {
        Some(k) => format!("      - {}", k.trim()),
        None => "      []".into(),
    };
    let packages: String = spec
        .packages
        .iter()
        .map(|p| format!("  - {p}\n"))
        .collect();

    let user_data = format!(
        r#"#cloud-config
hostname: {name}
manage_etc_hosts: true
ssh_pwauth: false
users:
  - name: visor
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: true
    ssh_authorized_keys:
{key_line}
package_update: true
packages:
  - linux-cloud-tools-virtual
{packages}"#,
        name = spec.name,
    );

    let meta_data = format!(
        "instance-id: visor-{name}\nlocal-hostname: {name}\n",
        name = spec.name
    );

    std::fs::write(dir.join("user-data"), user_data)?;
    std::fs::write(dir.join("meta-data"), meta_data)?;
    Ok(())
}
