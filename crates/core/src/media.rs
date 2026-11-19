//! PS5 ingest and the private, fixed playback endpoint.

mod auth;
mod job;

use anyhow::{Context, Result, anyhow, bail};
use axum::{Router, extract::DefaultBodyLimit, routing::post};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::VecDeque,
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    net::TcpListener,
    process::{Child, Command},
    sync::oneshot,
    task::JoinHandle,
    time::{Instant, sleep, timeout},
};
use uuid::Uuid;

use auth::AuthState;

const PLAYBACK_PATH: &str = "live/playstation";
const LOG_LIMIT: usize = 200;
type Logs = Arc<Mutex<VecDeque<String>>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaStats {
    pub ready: bool,
    pub bytes_received: u64,
    pub tracks: Vec<String>,
    pub readers: usize,
}

pub struct MediaSession {
    child: Option<Child>,
    job: Option<job::ProcessJob>,
    auth_state: Arc<AuthState>,
    auth_shutdown: Option<oneshot::Sender<()>>,
    auth_task: Option<JoinHandle<std::io::Result<()>>>,
    log_tasks: Vec<JoinHandle<()>>,
    logs: Logs,
    client: reqwest::Client,
    api_url: String,
    config_path: PathBuf,
}

impl MediaSession {
    /// Returns only after the media listener and authenticated control API are ready.
    pub async fn start(binary: &Path, runtime_dir: &Path, ps5_ip: Ipv4Addr) -> Result<Self> {
        if !binary.is_file() {
            bail!("找不到附带的媒体服务程序");
        }
        if ps5_ip.is_unspecified() || ps5_ip.is_multicast() || ps5_ip.is_broadcast() {
            bail!("PS5 IPv4 地址无效");
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()?;
        tokio::fs::create_dir_all(runtime_dir)
            .await
            .context("无法创建媒体运行目录")?;
        let auth_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .context("无法启动本机媒体鉴权")?;
        let auth_address = auth_listener.local_addr()?;
        let api_reservation = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .context("无法分配媒体控制端口")?;
        let api_address = api_reservation.local_addr()?;
        let auth_state = Arc::new(AuthState::new(ps5_ip));
        let config_path = runtime_dir.join(format!("mediamtx-{}.json", Uuid::new_v4()));
        let config = make_config(&auth_state, auth_address, api_address);
        if let Err(error) =
            tokio::fs::write(&config_path, serde_json::to_vec_pretty(&config)?).await
        {
            let _ = tokio::fs::remove_file(&config_path).await;
            return Err(error).context("无法写入媒体运行配置");
        }
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let app = Router::new()
            .route("/auth/{token}", post(auth::authenticate))
            .layer(DefaultBodyLimit::max(16 * 1024))
            .with_state(auth_state.clone());
        let auth_task = tokio::spawn(async move {
            axum::serve(
                auth_listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
        });
        let mut session = Self {
            child: None,
            job: None,
            auth_state,
            auth_shutdown: Some(shutdown_tx),
            auth_task: Some(auth_task),
            log_tasks: vec![],
            logs,
            client,
            api_url: format!("http://{api_address}"),
            config_path,
        };
        // The sidecar must bind its own port. If another process wins the race, startup fails.
        drop(api_reservation);
        let startup = session.spawn_and_wait(binary).await;
        if let Err(error) = startup {
            let _ = session.stop().await;
            return Err(error);
        }
        push_log(&session.logs, "媒体服务已就绪，等待 PS5 开播");
        Ok(session)
    }

    async fn spawn_and_wait(&mut self, binary: &Path) -> Result<()> {
        let mut command = Command::new(binary);
        command
            .arg(&self.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // MediaMTX environment overrides must not weaken the generated access policy.
        for (name, _) in std::env::vars_os() {
            let name_upper = name.to_string_lossy().to_ascii_uppercase();
            if name_upper.starts_with("MTX_") || name_upper.starts_with("RTSP_") {
                command.env_remove(name);
            }
        }
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let child = command.spawn().context("无法启动媒体服务")?;
        self.child = Some(child);
        self.job = Some(job::ProcessJob::attach(
            self.child.as_ref().expect("spawned child"),
        )?);
        let child = self.child.as_mut().expect("spawned child");
        if let Some(stdout) = child.stdout.take() {
            self.log_tasks
                .push(tokio::spawn(read_logs(stdout, self.logs.clone())));
        }
        if let Some(stderr) = child.stderr.take() {
            self.log_tasks
                .push(tokio::spawn(read_logs(stderr, self.logs.clone())));
        }
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            if let Some(status) = self.child.as_mut().expect("spawned child").try_wait()? {
                bail!("媒体服务启动失败（退出状态：{status}），请检查 1935 端口占用");
            }
            if self.fetch_connections().await.is_ok() {
                // Confirm this process accepts loopback, including on Windows where
                // wildcard and address-specific listeners can otherwise coexist.
                let probe = timeout(
                    Duration::from_secs(1),
                    tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, 1935)),
                )
                .await;
                if let Ok(Ok(probe)) = probe {
                    let local = probe.local_addr()?.to_string();
                    for _ in 0..10 {
                        if self
                            .fetch_connections()
                            .await?
                            .iter()
                            .any(|connection| connection.remote_addr == local)
                        {
                            return Ok(());
                        }
                        sleep(Duration::from_millis(30)).await;
                    }
                    bail!("本机 1935 端口被其他程序占用，请停止冲突程序后重试");
                }
            }
            if Instant::now() >= deadline {
                bail!("媒体服务未能在规定时间内就绪");
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    /// A failed API snapshot never releases the input reservation.
    pub async fn inspect(&mut self) -> Result<MediaStats> {
        let Some(child) = self.child.as_mut() else {
            bail!("媒体服务已停止");
        };
        if let Some(status) = child.try_wait()? {
            self.auth_state.close();
            bail!("媒体服务已退出（{status}）");
        }
        if self
            .auth_task
            .as_ref()
            .is_none_or(|task| task.is_finished())
        {
            self.auth_state.close();
            bail!("本机媒体鉴权服务已停止");
        }
        let reserved_before = self.auth_state.owner();
        let (connections, paths) =
            match tokio::try_join!(self.fetch_connections(), self.fetch_paths()) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    self.auth_state.close();
                    return Err(error);
                }
            };
        self.auth_state.reconcile(
            reserved_before,
            &connections.iter().map(|c| c.id).collect::<Vec<_>>(),
        );
        let owner = self.auth_state.owner();
        let input = owner.and_then(|id| {
            connections
                .iter()
                .find(|c| c.id == id && c.state == "publish")
        });
        let output = paths.iter().find(|p| p.name == PLAYBACK_PATH);
        let ready = input.is_some() && output.is_some_and(|p| p.online.unwrap_or(p.ready));
        Ok(MediaStats {
            ready,
            bytes_received: input.map_or(0, |c| c.inbound_bytes),
            tracks: output.map_or_else(Vec::new, |p| {
                if p.tracks2.is_empty() {
                    p.tracks.clone()
                } else {
                    p.tracks2.iter().map(|t| t.codec.clone()).collect()
                }
            }),
            readers: output.map_or(0, |p| p.readers.len()),
        })
    }

    pub fn take_logs(&mut self) -> Vec<String> {
        self.logs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
            .collect()
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.auth_state.close();
        let mut stop_error = None;
        if let Some(mut child) = self.child.take() {
            if let Err(error) = child.start_kill() {
                stop_error = Some(anyhow!(error).context("无法停止媒体服务"));
            }
            self.job.take();
            match timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => stop_error = Some(anyhow!(error).context("无法回收媒体服务进程")),
                Err(_) => stop_error = Some(anyhow!("等待媒体服务退出超时")),
            }
        }
        self.job.take();
        if let Some(shutdown) = self.auth_shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(mut task) = self.auth_task.take()
            && timeout(Duration::from_secs(2), &mut task).await.is_err()
        {
            task.abort();
        }
        for task in self.log_tasks.drain(..) {
            task.abort();
        }
        match tokio::fs::remove_file(&self.config_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => push_log(&self.logs, "媒体运行配置清理失败，请检查运行目录权限"),
        }
        push_log(&self.logs, "媒体服务已停止");
        if let Some(error) = stop_error {
            return Err(error);
        }
        Ok(())
    }

    async fn fetch_connections(&self) -> Result<Vec<Connection>> {
        self.fetch_list("/v3/rtmp/conns/list").await
    }
    async fn fetch_paths(&self) -> Result<Vec<MediaPath>> {
        self.fetch_list("/v3/paths/list").await
    }

    async fn fetch_list<T: for<'de> Deserialize<'de>>(&self, endpoint: &str) -> Result<Vec<T>> {
        let mut result = Vec::new();
        for page in 0..32 {
            let response = self
                .client
                .get(format!("{}{endpoint}", self.api_url))
                .basic_auth("playstreamer", Some(&self.auth_state.api_token))
                .query(&[("page", page), ("itemsPerPage", 100)])
                .send()
                .await
                .map_err(|_| anyhow!("无法连接本机媒体控制服务"))?;
            if !response.status().is_success() {
                bail!("本机媒体控制服务拒绝请求（{}）", response.status().as_u16());
            }
            let data: ApiList<T> = response
                .json()
                .await
                .map_err(|_| anyhow!("媒体状态响应格式不兼容"))?;
            result.extend(data.items);
            if page + 1 >= data.page_count {
                return Ok(result);
            }
        }
        bail!("媒体连接数超出允许范围")
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.auth_state.close();
        if let Some(child) = &mut self.child {
            let _ = child.start_kill();
        }
        self.job.take();
        if let Some(shutdown) = self.auth_shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.auth_task.take() {
            task.abort();
        }
        for task in self.log_tasks.drain(..) {
            task.abort();
        }
        let _ = std::fs::remove_file(&self.config_path);
    }
}

fn make_config(state: &AuthState, auth: SocketAddr, api: SocketAddr) -> serde_json::Value {
    json!({
        "logLevel": "info", "logDestinations": ["stdout"], "dumpPackets": false,
        "readTimeout": "10s", "writeTimeout": "10s",
        "authMethod": "http", "authHTTPAddress": format!("http://{auth}/auth/{}", state.callback_token),
        "authHTTPExclude": [], "authInternalUsers": [],
        "api": true, "apiAddress": api.to_string(), "apiAllowOrigins": [],
        "metrics": false, "pprof": false, "playback": false,
        "rtsp": false, "rtmp": true, "rtmpEncryption": "no", "rtmpAddress": "0.0.0.0:1935",
        "hls": false, "webrtc": false, "srt": false, "moq": false,
        "pathDefaults": { "record": false, "overridePublisher": false },
        "paths": {
            "~^app/[^/]+$": { "source": "publisher", "forward": [{
                "dest": format!("rtmp://127.0.0.1:1935/live?user=playstreamer-relay&pass={}#playstation", state.relay_token)
            }] },
            "live/playstation": { "source": "publisher" }
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiList<T> {
    items: Vec<T>,
    page_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    id: Uuid,
    state: String,
    remote_addr: String,
    #[serde(default)]
    inbound_bytes: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MediaPath {
    name: String,
    #[serde(default)]
    ready: bool,
    online: Option<bool>,
    #[serde(default)]
    tracks: Vec<String>,
    #[serde(default)]
    tracks2: Vec<Track>,
    #[serde(default)]
    readers: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct Track {
    codec: String,
}

fn push_log(logs: &Logs, message: &str) {
    let mut logs = logs.lock().unwrap_or_else(|p| p.into_inner());
    if logs.len() == LOG_LIMIT {
        logs.pop_front();
    }
    logs.push_back(message.into());
}

// Sidecar messages can contain arbitrary publisher-controlled paths and query strings.
// Only known event categories cross into retained logs; raw lines are never persisted.
fn safe_log(line: &str) -> Option<&'static str> {
    if line.contains("ERR") || line.contains("error") {
        return Some("媒体服务报告连接错误，原始连接信息已隐藏");
    }
    if line.contains("WAR") {
        return Some("媒体服务报告连接警告，原始连接信息已隐藏");
    }
    if line.contains("is publishing") {
        return Some("媒体输入已连接");
    }
    if line.contains("is reading") {
        return Some("本机播放器已连接");
    }
    if line.contains("closed") {
        return Some("媒体连接已关闭");
    }
    None
}

async fn read_logs<R: AsyncRead + Unpin>(mut reader: R, logs: Logs) {
    let mut chunk = [0_u8; 2048];
    let mut line = Vec::with_capacity(4096);
    let mut oversized = false;
    loop {
        let count = match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(count) => count,
        };
        for &byte in &chunk[..count] {
            if byte == b'\n' {
                if !oversized && let Some(message) = safe_log(&String::from_utf8_lossy(&line)) {
                    push_log(&logs, message);
                }
                line.clear();
                oversized = false;
            } else if line.len() < 4096 && !oversized {
                line.push(byte);
            } else {
                oversized = true;
                line.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_config_has_no_recursive_forward_or_public_api() {
        let state = AuthState::new(Ipv4Addr::new(192, 168, 1, 10));
        let config = make_config(
            &state,
            "127.0.0.1:1234".parse().unwrap(),
            "127.0.0.1:4321".parse().unwrap(),
        );
        assert!(config["paths"][PLAYBACK_PATH].get("forward").is_none());
        assert!(config["paths"].get("all_others").is_none());
        assert_eq!(config["apiAddress"], "127.0.0.1:4321");
        assert_eq!(config["authHTTPExclude"], json!([]));
    }

    #[tokio::test]
    async fn publisher_controlled_text_never_enters_log_ring() {
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let raw = b"ERR path app/secret-key?pass=other-secret\nINF is publishing app/secret-key\nunknown-secret\n";
        read_logs(&raw[..], logs.clone()).await;
        let saved = logs.lock().unwrap().iter().cloned().collect::<String>();
        assert!(!saved.contains("secret"));
        assert_eq!(logs.lock().unwrap().len(), 2);
    }
}
