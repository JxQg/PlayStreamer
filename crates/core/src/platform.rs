use crate::types::{DiagnosticCheck, NetworkInterface};
use anyhow::{Context, Result, bail};
#[cfg(windows)]
use std::process::Command;
use std::{net::Ipv4Addr, path::Path};

#[cfg(windows)]
fn powershell(script: &str) -> Result<String> {
    use std::os::windows::process::CommandExt;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(0x08000000)
        .output()
        .context("无法运行 Windows 网络检查")?;
    if !output.status.success() {
        bail!(
            "Windows 操作失败：{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?
        .trim_start_matches('\u{feff}')
        .trim()
        .to_owned())
}

pub fn list_interfaces() -> Result<Vec<NetworkInterface>> {
    #[cfg(windows)]
    {
        let text = powershell(include_str!("network-interfaces.ps1"))?;
        serde_json::from_str(&text).context("无法读取网卡信息")
    }
    #[cfg(not(windows))]
    {
        Ok(vec![])
    }
}

pub fn ensure_supported() -> Result<()> {
    if !cfg!(windows) {
        bail!("此版本仅支持 Windows 接流，macOS 接流适配尚未开放");
    }
    Ok(())
}

pub fn network_environment(
    lan: Option<Ipv4Addr>,
    ps5: Option<Ipv4Addr>,
    upstreams: Vec<Ipv4Addr>,
) -> Result<Vec<DiagnosticCheck>> {
    #[cfg(windows)]
    {
        let body = include_str!("network-environment.ps1");
        let parameters = format!(
            "-Ps5Ip '{}' -LanIp '{}' -Upstreams @({})",
            ps5.map(|ip| ip.to_string()).unwrap_or_default(),
            lan.map(|ip| ip.to_string()).unwrap_or_default(),
            upstreams
                .iter()
                .map(|ip| format!("'{ip}'"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let text = powershell(&format!("& {{ {body} }} {parameters}"))?;
        serde_json::from_str(&text).context("无法读取 TUN / VPN 网络诊断")
    }
    #[cfg(not(windows))]
    {
        let _ = (lan, ps5, upstreams);
        Ok(vec![])
    }
}

pub fn obs_installed() -> bool {
    #[cfg(windows)]
    {
        let candidates = [
            std::env::var_os("ProgramFiles"),
            std::env::var_os("ProgramFiles(x86)"),
        ];
        candidates.into_iter().flatten().any(|p| {
            Path::new(&p)
                .join("obs-studio/bin/64bit/obs64.exe")
                .is_file()
        })
    }
    #[cfg(not(windows))]
    {
        Path::new("/Applications/OBS.app").is_dir()
    }
}

pub fn configure_firewall(
    exe: &Path,
    sidecar: &Path,
    lan: Ipv4Addr,
    ps5: Ipv4Addr,
) -> Result<String> {
    ensure_supported()?;
    #[cfg(windows)]
    {
        let exe = exe.canonicalize()?.to_string_lossy().replace('\'', "''");
        let sidecar = sidecar
            .canonicalize()?
            .to_string_lossy()
            .replace('\'', "''");
        let generation = uuid::Uuid::new_v4();
        let script = format!(
            r#"$ErrorActionPreference='Stop'
$rules=@(
@{{Name='PlayStreamer.DNS.UDP.{generation}';Protocol='UDP';LocalPort=53;Program='{exe}'}},
@{{Name='PlayStreamer.DNS.TCP.{generation}';Protocol='TCP';LocalPort=53;Program='{exe}'}},
@{{Name='PlayStreamer.RTMP.{generation}';Protocol='TCP';LocalPort=1935;Program='{sidecar}'}})
$added=@()
try {{ foreach($r in $rules) {{ New-NetFirewallRule -Name $r.Name -DisplayName $r.Name -Group 'PlayStreamer' -Direction Inbound -Action Allow -Protocol $r.Protocol -LocalPort $r.LocalPort -LocalAddress '{lan}' -RemoteAddress '{ps5}' -Program $r.Program -Profile Private | Out-Null; $added+=$r.Name }} }} catch {{ foreach($name in $added) {{Remove-NetFirewallRule -Name $name -ErrorAction SilentlyContinue}}; throw }}
Get-NetFirewallRule -Group 'PlayStreamer' -ErrorAction SilentlyContinue | Where-Object {{ $_.Name -like 'PlayStreamer.*' -and $_.Name -notin $added }} | Remove-NetFirewallRule"#
        );
        use std::io::Write;
        let file = std::env::temp_dir().join(format!(
            "playstreamer-firewall-{}.ps1",
            uuid::Uuid::new_v4()
        ));
        let mut handle = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&file)?;
        handle.write_all(&[0xef, 0xbb, 0xbf])?;
        handle.write_all(script.as_bytes())?;
        drop(handle);
        let quoted = file.to_string_lossy().replace('\'', "''");
        let command = format!(
            r#"$p=Start-Process powershell.exe -WindowStyle Hidden -Verb RunAs -Wait -PassThru -ArgumentList '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{quoted}"'; if($p.ExitCode -ne 0){{throw '防火墙配置未完成'}}"#
        );
        let result = powershell(&command);
        let _ = std::fs::remove_file(file);
        result?;
        Ok("已为所选 PS5 配置专用网络的 DNS 和 RTMP 入站规则".into())
    }
    #[cfg(not(windows))]
    {
        let _ = (exe, sidecar, lan, ps5);
        bail!("macOS 防火墙适配尚未开放")
    }
}

pub fn firewall_ready() -> Result<bool> {
    #[cfg(windows)]
    {
        let result = powershell(
            "$r=@(Get-NetFirewallRule -Group 'PlayStreamer' -ErrorAction SilentlyContinue | Where-Object {$_.Enabled -eq 'True'}); $r.Count",
        )?;
        Ok(result.trim().parse::<usize>().unwrap_or(0) >= 3)
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}
