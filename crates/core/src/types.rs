use serde::{Deserialize, Serialize};

pub const PLAYBACK_URL: &str = "rtmp://127.0.0.1:1935/live/playstation";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub network_interface_id: String,
    pub ps5_ip: String,
    pub upstream_dns: Vec<String>,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub setup_completed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    pub id: String,
    pub name: String,
    pub ipv4: String,
    pub prefix_length: u8,
    pub dns_servers: Vec<String>,
    pub eligible: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ServiceStatus {
    #[default]
    Idle,
    Starting,
    Waiting,
    Receiving,
    DnsOnly,
    NetworkChanged,
    MediaError,
    Suspended,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StreamInfo {
    pub bitrate_kbps: u64,
    pub tracks: Vec<String>,
    pub connected_seconds: u64,
    pub readers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: ServiceStatus,
    pub message: String,
    pub lan_ip: Option<String>,
    pub dns_running: bool,
    pub playback_url: String,
    pub stream: StreamInfo,
    pub last_error: Option<String>,
    pub dns_queries: u64,
    pub dns_intercepts: u64,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            status: ServiceStatus::Idle,
            message: "准备连接你的 PlayStation".into(),
            lan_ip: None,
            dns_running: false,
            playback_url: PLAYBACK_URL.into(),
            stream: StreamInfo::default(),
            last_error: None,
            dns_queries: 0,
            dns_intercepts: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheck {
    pub id: String,
    pub title: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub checks: Vec<DiagnosticCheck>,
}
