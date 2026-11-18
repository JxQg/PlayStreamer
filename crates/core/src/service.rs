use crate::{
    dns::{DnsService, DnsSettings},
    media::MediaSession,
    platform,
    types::*,
};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use std::{
    collections::VecDeque,
    net::{Ipv4Addr, SocketAddrV4},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{Mutex, watch},
    time::{Duration, Instant},
};

struct Running {
    dns: Option<DnsService>,
    media: Option<MediaSession>,
    lan: Option<Ipv4Addr>,
    connected: Option<Instant>,
    last_sample: Option<(Instant, u64)>,
    resume_media: bool,
}

pub struct Engine {
    config: Mutex<AppConfig>,
    config_path: PathBuf,
    binary: PathBuf,
    runtime: PathBuf,
    running: Mutex<Running>,
    state: watch::Sender<Snapshot>,
    logs: StdMutex<VecDeque<String>>,
    monitor_started: AtomicBool,
}

impl Engine {
    pub async fn new(config_path: PathBuf, binary: PathBuf, runtime: PathBuf) -> Result<Arc<Self>> {
        let config = match tokio::fs::read(&config_path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).context("配置文件损坏，请检查配置文件")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
            Err(e) => return Err(e.into()),
        };
        tokio::fs::create_dir_all(&runtime).await?;
        let (state, _) = watch::channel(Snapshot::default());
        Ok(Arc::new(Self {
            config: Mutex::new(config),
            config_path,
            binary,
            runtime,
            running: Mutex::new(Running {
                dns: None,
                media: None,
                lan: None,
                connected: None,
                last_sample: None,
                resume_media: false,
            }),
            state,
            logs: StdMutex::new(VecDeque::new()),
            monitor_started: AtomicBool::new(false),
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.state.subscribe()
    }
    pub async fn config(&self) -> AppConfig {
        self.config.lock().await.clone()
    }
    pub fn binary(&self) -> &Path {
        &self.binary
    }
    fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        self.state.send_modify(f);
    }
    fn log(&self, message: impl Into<String>) {
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        if logs.len() >= 300 {
            logs.pop_front();
        }
        logs.push_back(format!("{} {}", Utc::now().to_rfc3339(), message.into()));
    }
    pub async fn save_config(&self, config: AppConfig) -> Result<()> {
        let running = self.running.lock().await;
        let mut current = self.config.lock().await;
        if running.dns.is_some()
            && (config.network_interface_id != current.network_interface_id
                || config.ps5_ip != current.ps5_ip
                || config.upstream_dns != current.upstream_dns)
        {
            bail!("DNS 正在为 PS5 服务。请先恢复 PS5 自动 DNS 并退出应用，再修改网络配置");
        }
        if config.setup_completed {
            validate(&config, &interfaces().await?)?;
        }
        if let Some(parent) = self.config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let tmp = self.config_path.with_extension("json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(&config)?).await?;
        // Windows rename cannot replace an existing destination.
        #[cfg(windows)]
        {
            if self.config_path.exists() {
                use std::os::windows::ffi::OsStrExt;
                let source: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
                let dest: Vec<u16> = self
                    .config_path
                    .as_os_str()
                    .encode_wide()
                    .chain(Some(0))
                    .collect();
                if unsafe {
                    windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                        source.as_ptr(),
                        dest.as_ptr(),
                        0x1 | 0x8,
                    )
                } == 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
            } else {
                tokio::fs::rename(&tmp, &self.config_path).await?;
            }
        }
        #[cfg(not(windows))]
        tokio::fs::rename(&tmp, &self.config_path).await?;
        *current = config;
        Ok(())
    }
    pub async fn start_receiving(&self) -> Result<Snapshot> {
        platform::ensure_supported()?;
        let mut running = self.running.lock().await;
        if running.media.is_some() {
            return Ok(self.snapshot());
        }
        let config = self.config().await;
        let (lan, ps5, upstreams) = validate(&config, &interfaces().await?)?;
        if running.dns.is_some() && running.lan != Some(lan) {
            if let Some(dns) = running.dns.take() {
                dns.stop().await;
            }
            self.update(|s| {
                s.status = ServiceStatus::NetworkChanged;
                s.dns_running = false;
                s.message = "电脑地址已变化，请更新 PS5 的 DNS 设置后重新开启".into();
            });
            bail!("电脑地址已变化，请更新 PS5 的 DNS 设置后重新开启");
        }
        self.update(|s| {
            s.status = ServiceStatus::Starting;
            s.message = "正在检查网络并启动接收服务".into();
            s.last_error = None;
        });
        let new_dns = if running.dns.is_none() {
            match DnsService::bind(DnsSettings {
                bind: SocketAddrV4::new(lan, 53),
                answer_ip: lan,
                ps5_ip: ps5,
                upstreams: upstreams
                    .into_iter()
                    .map(|ip| SocketAddrV4::new(ip, 53))
                    .collect(),
            })
            .await
            {
                Ok(dns) => Some(dns),
                Err(e) => {
                    self.start_failed(running.dns.is_some(), &e);
                    return Err(e);
                }
            }
        } else {
            None
        };
        let media = match MediaSession::start(&self.binary, &self.runtime, ps5).await {
            Ok(media) => media,
            Err(e) => {
                drop(new_dns);
                self.start_failed(running.dns.is_some(), &e);
                return Err(e);
            }
        };
        if let Some(mut dns) = new_dns {
            dns.activate();
            running.dns = Some(dns);
        }
        running.media = Some(media);
        running.lan = Some(lan);
        running.connected = None;
        running.last_sample = None;
        self.update(|s| {
            s.status = ServiceStatus::Waiting;
            s.message = "已就绪，请在 PS5 上开始广播".into();
            s.dns_running = true;
            s.lan_ip = Some(lan.to_string());
            s.last_error = None;
        });
        self.log("接收服务已启动");
        Ok(self.snapshot())
    }
    fn start_failed(&self, dns: bool, error: &anyhow::Error) {
        let message = format!("接收服务启动失败：{error}");
        self.log(&message);
        self.update(|s| {
            s.status = if dns {
                ServiceStatus::DnsOnly
            } else {
                ServiceStatus::MediaError
            };
            s.message = message.clone();
            s.last_error = Some(message);
            s.dns_running = dns;
        });
    }
    pub async fn stop_receiving(&self) -> Result<Snapshot> {
        let mut running = self.running.lock().await;
        let result = stop_media(&mut running).await;
        running.resume_media = false;
        let error = result.as_ref().err().map(ToString::to_string);
        let dns_running = running.dns.is_some();
        self.update(|s| {
            s.status = if dns_running {
                ServiceStatus::DnsOnly
            } else if error.is_some() {
                ServiceStatus::MediaError
            } else {
                ServiceStatus::Idle
            };
            s.message = if dns_running {
                "接收已停止，DNS 保持运行。请同时停止 PS5 广播"
            } else {
                "接收已停止"
            }
            .into();
            if let Some(error) = &error {
                s.message.push_str(&format!("；媒体回收检查失败：{error}"));
            }
            s.dns_running = dns_running;
            s.last_error = error;
            s.stream = StreamInfo::default();
        });
        self.log(self.snapshot().message);
        result?;
        Ok(self.snapshot())
    }
    pub async fn shutdown(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        let media_result = stop_media(&mut running).await;
        if let Some(dns) = running.dns.take() {
            dns.stop().await;
        }
        running.resume_media = false;
        running.lan = None;
        self.update(|s| {
            *s = Snapshot::default();
            if let Err(error) = &media_result {
                s.message = format!("服务已停止，但媒体回收检查失败：{error}");
                s.last_error = Some(error.to_string());
            }
        });
        media_result
    }
    pub async fn suspend(&self) -> Result<()> {
        let mut r = self.running.lock().await;
        if self.snapshot().status == ServiceStatus::Suspended {
            return Ok(());
        }
        r.resume_media = r.media.is_some();
        let result = stop_media(&mut r).await;
        if let Some(dns) = r.dns.take() {
            dns.stop().await;
        }
        self.update(|s| {
            s.status = ServiceStatus::Suspended;
            s.dns_running = false;
            s.stream = StreamInfo::default();
            s.message = "电脑已挂起，PS5 DNS 暂不可用".into();
            s.last_error = result.as_ref().err().map(ToString::to_string);
            if let Some(error) = &s.last_error {
                s.message.push_str(&format!("；媒体回收检查失败：{error}"));
            }
        });
        if let Err(error) = &result {
            self.log(format!("挂起时回收媒体服务失败：{error}"));
        }
        result
    }
    pub async fn resume(&self) -> Result<()> {
        let mut r = self.running.lock().await;
        if self.snapshot().status != ServiceStatus::Suspended {
            return Ok(());
        }
        let config = self.config().await;
        let networks = match interfaces().await {
            Ok(networks) => networks,
            Err(error) => {
                self.resume_failed(&error);
                return Err(error);
            }
        };
        let result = validate(&config, &networks);
        let Ok((lan, ps5, upstreams)) = result else {
            drop(r);
            self.network_changed().await;
            return Ok(());
        };
        if Some(lan) != r.lan {
            drop(r);
            self.network_changed().await;
            return Ok(());
        }
        let should_resume = r.resume_media;
        let dns = DnsService::bind(DnsSettings {
            bind: SocketAddrV4::new(lan, 53),
            answer_ip: lan,
            ps5_ip: ps5,
            upstreams: upstreams
                .into_iter()
                .map(|ip| SocketAddrV4::new(ip, 53))
                .collect(),
        })
        .await;
        let mut dns = match dns {
            Ok(dns) => dns,
            Err(error) => {
                self.resume_failed(&error);
                return Err(error);
            }
        };
        dns.activate();
        r.dns = Some(dns);
        r.resume_media = false;
        self.update(|s| {
            s.status = ServiceStatus::DnsOnly;
            s.dns_running = true;
            s.message = "DNS 已恢复".into();
            s.last_error = None;
        });
        drop(r);
        if should_resume {
            if let Err(error) = self.start_receiving().await {
                self.resume_failed(&error);
                return Err(error);
            }
        }
        Ok(())
    }
    fn resume_failed(&self, error: &anyhow::Error) {
        let message = format!("恢复服务失败：{error}。请检查网络后重新开启接收");
        self.log(&message);
        self.update(|s| {
            s.message = message;
            s.last_error = Some(error.to_string());
        });
    }
    async fn network_changed(&self) {
        let mut r = self.running.lock().await;
        r.resume_media = false;
        if let Err(e) = stop_media(&mut r).await {
            self.log(format!("停止媒体服务失败：{e}"));
        }
        if let Some(dns) = r.dns.take() {
            dns.stop().await;
        }
        self.update(|s| {
            s.status = ServiceStatus::NetworkChanged;
            s.dns_running = false;
            s.stream = StreamInfo::default();
            s.message = "网络地址已变化，请重新检查网卡并更新 PS5 的 DNS 设置".into();
        });
        self.log("网络配置变化，已停止服务");
    }
    pub fn start_monitor(self: &Arc<Self>) {
        if self.monitor_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            let mut tick = 0u32;
            let mut last = std::time::SystemTime::now();
            loop {
                interval.tick().await;
                let Some(engine) = weak.upgrade() else {
                    break;
                };
                let now = std::time::SystemTime::now();
                let gap = now.duration_since(last).unwrap_or_default();
                last = now;
                if gap > Duration::from_secs(20) && engine.snapshot().dns_running {
                    if let Err(e) = engine.suspend().await {
                        engine.log(format!("挂起处理失败：{e}"));
                    }
                    if let Err(e) = engine.resume().await {
                        engine.log(format!("恢复失败：{e}"));
                        engine.update(|s| {
                            s.last_error = Some(e.to_string());
                        });
                    }
                }
                tick = tick.wrapping_add(1);
                if tick % 3 == 0 && engine.snapshot().dns_running {
                    let config = engine.config().await;
                    match interfaces().await {
                        Ok(list) => {
                            let changed = {
                                let r = engine.running.lock().await;
                                validate(&config, &list)
                                    .map(|v| Some(v.0) != r.lan)
                                    .unwrap_or(true)
                            };
                            if changed {
                                engine.network_changed().await;
                            }
                        }
                        Err(e) => {
                            engine.log(format!("网络检查失败：{e}"));
                            engine.network_changed().await;
                        }
                    }
                }
                engine.sample().await;
            }
        });
    }
    async fn sample(&self) {
        let mut r = self.running.lock().await;
        if let Some(dns) = &r.dns {
            let (q, i) = dns.counts();
            self.update(|s| {
                s.dns_queries = q;
                s.dns_intercepts = i;
            });
        }
        let Some(media) = r.media.as_mut() else {
            return;
        };
        let result = media.inspect().await;
        let logs = media.take_logs();
        for log in logs {
            self.log(log);
        }
        match result {
            Ok(stats) => {
                let now = Instant::now();
                let bitrate = r
                    .last_sample
                    .map(|(at, b)| {
                        ((stats.bytes_received.saturating_sub(b) as f64 * 8.0)
                            / (now.duration_since(at).as_secs_f64().max(0.01) * 1000.0))
                            as u64
                    })
                    .unwrap_or(0);
                r.last_sample = Some((now, stats.bytes_received));
                if stats.ready {
                    r.connected.get_or_insert(now);
                } else {
                    r.connected = None;
                }
                let seconds = r
                    .connected
                    .map(|at| now.duration_since(at).as_secs())
                    .unwrap_or(0);
                self.update(|s| {
                    s.status = if stats.ready {
                        ServiceStatus::Receiving
                    } else {
                        ServiceStatus::Waiting
                    };
                    s.message = if stats.ready {
                        "正在接收 PS5 画面"
                    } else {
                        "已就绪，请在 PS5 上开始广播"
                    }
                    .into();
                    s.stream = StreamInfo {
                        bitrate_kbps: bitrate,
                        tracks: stats.tracks,
                        connected_seconds: seconds,
                        readers: stats.readers,
                    };
                });
            }
            Err(e) => {
                let text = format!("媒体服务异常：{e}");
                self.log(&text);
                if let Err(e) = stop_media(&mut r).await {
                    self.log(format!("回收媒体服务失败：{e}"));
                }
                self.update(|s| {
                    s.status = ServiceStatus::MediaError;
                    s.message = "媒体服务异常，DNS 保持运行，可重新开启接收".into();
                    s.last_error = Some(text);
                    s.stream = StreamInfo::default();
                });
            }
        }
    }
    pub async fn diagnostics(&self) -> DiagnosticReport {
        let config = self.config().await;
        let snapshot = self.snapshot();
        let mut checks = vec![];
        let mut add = |id: &str, title: &str, status: &str, detail: String| {
            checks.push(DiagnosticCheck {
                id: id.into(),
                title: title.into(),
                status: status.into(),
                detail,
            })
        };
        add(
            "platform",
            "系统支持",
            if cfg!(windows) { "pass" } else { "warn" },
            if cfg!(windows) {
                "Windows 接流客户端"
            } else {
                "macOS 仅预留核心编译支持"
            }
            .into(),
        );
        let networks = interfaces().await;
        let mut environment = (None, None, Vec::new());
        match &networks {
            Ok(n) => match validate(&config, n) {
                Ok((lan, ps5, up)) => {
                    add(
                        "network",
                        "局域网配置",
                        "pass",
                        format!("电脑 {lan}，PS5 {ps5}，{} 个上游 DNS", up.len()),
                    );
                    environment = (Some(lan), Some(ps5), up);
                }
                Err(e) => add("network", "局域网配置", "fail", e.to_string()),
            },
            Err(e) => add("network", "局域网配置", "fail", e.to_string()),
        }
        add(
            "sidecar",
            "媒体组件",
            if self.binary.is_file() {
                "pass"
            } else {
                "fail"
            },
            if self.binary.is_file() {
                "MediaMTX 组件已安装"
            } else {
                "缺少 MediaMTX，请重新安装或运行组件准备脚本"
            }
            .into(),
        );
        let obs = tokio::task::spawn_blocking(platform::obs_installed)
            .await
            .unwrap_or(false);
        add(
            "obs",
            "OBS 安装",
            if obs { "pass" } else { "warn" },
            if obs {
                "在默认路径找到 OBS；请按教程添加媒体源"
            } else {
                "未在默认路径找到 OBS，可从官网安装或使用已有便携版"
            }
            .into(),
        );
        let firewall = tokio::task::spawn_blocking(platform::firewall_ready).await;
        let ok = matches!(firewall, Ok(Ok(true)));
        add(
            "firewall",
            "Windows 防火墙",
            if ok { "pass" } else { "warn" },
            if ok {
                "找到应用防火墙规则，请确认当前网络为专用网络"
            } else {
                "尚未确认入站规则，可点击配置防火墙"
            }
            .into(),
        );
        add(
            "dns",
            "PS5 DNS 请求",
            if snapshot.dns_intercepts > 0 {
                "pass"
            } else {
                "warn"
            },
            format!(
                "本次会话 {} 次解析，{} 次直播入口命中",
                snapshot.dns_queries, snapshot.dns_intercepts
            ),
        );
        add(
            "player",
            "播放器连接",
            if snapshot.stream.readers > 0 {
                "pass"
            } else {
                "warn"
            },
            format!(
                "{} 个本地播放器正在读取；此检查不识别具体播放器",
                snapshot.stream.readers
            ),
        );
        add(
            "twitch",
            "Twitch 开播网络",
            "warn",
            "需在 PS5 上验证账号绑定和开始广播；电脑网络测试无法证明主机的 Twitch 连通性".into(),
        );
        let (lan, ps5, upstreams) = environment;
        let environment =
            tokio::task::spawn_blocking(move || platform::network_environment(lan, ps5, upstreams))
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
        match environment {
            Ok(mut environment) => checks.append(&mut environment),
            Err(error) => {
                checks.push(DiagnosticCheck {
                    id: "network_environment".into(),
                    title: "TUN 与网络路由".into(),
                    status: "warn".into(),
                    detail: format!("无法完成网络环境检查：{error}"),
                });
            }
        }
        DiagnosticReport { checks }
    }
    pub async fn export_diagnostics(&self, dir: &Path) -> Result<PathBuf> {
        tokio::fs::create_dir_all(dir).await?;
        let report = self.diagnostics().await;
        let config = self.config().await;
        let logs = self.logs.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let path = dir.join(format!(
            "PlayStreamer-diagnostics-{}.json",
            Utc::now().format("%Y%m%d-%H%M%S")
        ));
        let doc = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"createdAt":Utc::now(),"config":config,"snapshot":self.snapshot(),"checks":report.checks,"logs":logs});
        tokio::fs::write(&path, serde_json::to_vec_pretty(&doc)?).await?;
        Ok(path)
    }
}

async fn stop_media(r: &mut Running) -> Result<()> {
    r.connected = None;
    r.last_sample = None;
    if let Some(mut media) = r.media.take() {
        media.stop().await?;
    }
    Ok(())
}
pub async fn interfaces() -> Result<Vec<NetworkInterface>> {
    tokio::task::spawn_blocking(platform::list_interfaces).await?
}

pub fn validate(
    config: &AppConfig,
    interfaces: &[NetworkInterface],
) -> Result<(Ipv4Addr, Ipv4Addr, Vec<Ipv4Addr>)> {
    let network = interfaces
        .iter()
        .find(|n| n.id == config.network_interface_id && n.eligible)
        .context("请选择已连接的物理网卡")?;
    let lan: Ipv4Addr = network.ipv4.parse()?;
    let ps5: Ipv4Addr = config
        .ps5_ip
        .parse()
        .context("请输入有效的 PS5 IPv4 地址")?;
    if ps5 == lan
        || ps5.is_loopback()
        || ps5.is_unspecified()
        || ps5.is_multicast()
        || ps5 == Ipv4Addr::BROADCAST
    {
        bail!("PS5 地址必须是局域网中另一台设备的地址");
    }
    if network.prefix_length == 0 || network.prefix_length > 32 {
        bail!("网卡子网信息无效");
    }
    let mask = u32::MAX
        .checked_shl(32 - u32::from(network.prefix_length))
        .unwrap_or(0);
    if (u32::from(lan) & mask) != (u32::from(ps5) & mask) {
        bail!("PS5 与电脑必须在同一 IPv4 子网");
    }
    if network.prefix_length < 31
        && [u32::from(lan) & mask, (u32::from(lan) & mask) | !mask].contains(&u32::from(ps5))
    {
        bail!("PS5 地址不能是子网或广播地址");
    }
    let candidates = if config.upstream_dns.is_empty() {
        &network.dns_servers
    } else {
        &config.upstream_dns
    };
    let mut upstreams = vec![];
    for raw in candidates {
        let ip: Ipv4Addr = raw.parse().context("上游 DNS 必须是 IPv4 地址")?;
        if ip.is_loopback()
            || ip.is_unspecified()
            || ip.is_multicast()
            || ip == Ipv4Addr::BROADCAST
            || interfaces.iter().any(|n| n.ipv4 == ip.to_string())
        {
            continue;
        }
        if !upstreams.contains(&ip) {
            upstreams.push(ip);
        }
    }
    if upstreams.is_empty() {
        bail!("没有可用的上游 DNS，请填写路由器或其他 DNS 地址，不能指向本机");
    }
    Ok((lan, ps5, upstreams))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn network() -> Vec<NetworkInterface> {
        vec![NetworkInterface {
            id: "guid".into(),
            name: "Ethernet".into(),
            ipv4: "192.168.1.2".into(),
            prefix_length: 24,
            dns_servers: vec!["192.168.1.1".into()],
            eligible: true,
        }]
    }
    fn config() -> AppConfig {
        AppConfig {
            network_interface_id: "guid".into(),
            ps5_ip: "192.168.1.8".into(),
            ..Default::default()
        }
    }
    #[test]
    fn rejects_unsafe_network_config() {
        for ip in [
            "192.168.2.8",
            "192.168.1.2",
            "192.168.1.255",
            "192.168.1.0",
            "127.0.0.1",
            "oops",
        ] {
            let mut c = config();
            c.ps5_ip = ip.into();
            assert!(validate(&c, &network()).is_err(), "{ip}");
        }
    }
    #[test]
    fn filters_recursive_dns_without_public_fallback() {
        let mut c = config();
        c.upstream_dns = vec!["192.168.1.2".into(), "127.0.0.1".into()];
        assert!(validate(&c, &network()).is_err());
        assert!(validate(&config(), &network()).is_ok());
    }
    #[tokio::test]
    async fn repeated_suspend_preserves_pending_media_resume() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(
            dir.path().join("config.json"),
            dir.path().join("mediamtx"),
            dir.path().join("runtime"),
        )
        .await
        .unwrap();
        engine.update(|s| s.status = ServiceStatus::Suspended);
        engine.running.lock().await.resume_media = true;

        engine.suspend().await.unwrap();
        engine.suspend().await.unwrap();
        assert!(engine.running.lock().await.resume_media);

        engine.stop_receiving().await.unwrap();
        assert!(!engine.running.lock().await.resume_media);
        assert_eq!(engine.snapshot().status, ServiceStatus::Idle);
    }
    #[tokio::test]
    async fn shutdown_releases_dns_and_clears_resume_state() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(
            dir.path().join("config.json"),
            dir.path().join("mediamtx"),
            dir.path().join("runtime"),
        )
        .await
        .unwrap();
        let probe = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = probe.local_addr().unwrap();
        drop(probe);
        let mut dns = DnsService::bind(DnsSettings {
            bind: SocketAddrV4::new(Ipv4Addr::LOCALHOST, address.port()),
            answer_ip: Ipv4Addr::LOCALHOST,
            ps5_ip: Ipv4Addr::LOCALHOST,
            upstreams: vec![SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1)],
        })
        .await
        .unwrap();
        dns.activate();
        {
            let mut running = engine.running.lock().await;
            running.dns = Some(dns);
            running.lan = Some(Ipv4Addr::LOCALHOST);
            running.resume_media = true;
            running.connected = Some(Instant::now());
            running.last_sample = Some((Instant::now(), 1024));
        }
        engine.update(|s| {
            s.status = ServiceStatus::DnsOnly;
            s.dns_running = true;
        });

        engine.shutdown().await.unwrap();
        let _tcp = tokio::net::TcpListener::bind(address).await.unwrap();
        let _udp = tokio::net::UdpSocket::bind(address).await.unwrap();
        let running = engine.running.lock().await;
        assert!(running.dns.is_none());
        assert!(running.lan.is_none());
        assert!(!running.resume_media);
        assert!(running.connected.is_none());
        assert!(running.last_sample.is_none());
        assert_eq!(engine.snapshot().status, ServiceStatus::Idle);
        assert!(!engine.snapshot().dns_running);
    }
}
