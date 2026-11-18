use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use super::PLAYBACK_PATH;

pub(super) struct AuthState {
    ps5_ip: Ipv4Addr,
    pub callback_token: String,
    pub api_token: String,
    pub relay_token: String,
    session: Mutex<Admission>,
}

#[derive(Default)]
struct Admission {
    owner: Option<Uuid>,
    closed: bool,
}

#[derive(Deserialize)]
pub(super) struct AuthRequest {
    pub ip: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: String,
    pub action: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub protocol: String,
    pub id: Option<Uuid>,
}

impl AuthState {
    pub fn new(ps5_ip: Ipv4Addr) -> Self {
        Self {
            ps5_ip,
            callback_token: Uuid::new_v4().to_string(),
            api_token: Uuid::new_v4().to_string(),
            relay_token: Uuid::new_v4().to_string(),
            session: Mutex::new(Admission::default()),
        }
    }

    pub fn owner(&self) -> Option<Uuid> {
        self.session.lock().unwrap_or_else(|p| p.into_inner()).owner
    }
    pub fn close(&self) {
        let mut s = self.session.lock().unwrap_or_else(|p| p.into_inner());
        s.closed = true;
        s.owner = None;
    }

    pub fn reconcile(&self, before: Option<Uuid>, active: &[Uuid]) {
        let mut s = self.session.lock().unwrap_or_else(|p| p.into_inner());
        if before.is_some() && s.owner == before && !active.contains(&before.unwrap()) {
            s.owner = None;
        }
    }

    fn permits(&self, request: &AuthRequest) -> bool {
        let Ok(ip) = request.ip.parse::<IpAddr>() else {
            return false;
        };
        // API authentication must remain independent of the input admission lock.
        if request.action == "api" {
            return ip.is_loopback()
                && request.user == "playstreamer"
                && request.password == self.api_token;
        }
        if request.protocol != "rtmp" {
            return false;
        }
        if request.path == PLAYBACK_PATH {
            return ip.is_loopback()
                && match request.action.as_str() {
                    "read" => true,
                    "publish" => {
                        request.user == "playstreamer-relay" && request.password == self.relay_token
                    }
                    _ => false,
                };
        }
        if ip != IpAddr::V4(self.ps5_ip) || request.action != "publish" {
            return false;
        }
        let Some(key) = request.path.strip_prefix("app/") else {
            return false;
        };
        if key.is_empty()
            || key.len() > 2048
            || key.contains('/')
            || key.chars().any(char::is_control)
        {
            return false;
        }
        let Some(id) = request.id else {
            return false;
        };
        let mut s = self.session.lock().unwrap_or_else(|p| p.into_inner());
        if s.closed {
            return false;
        }
        match s.owner {
            Some(owner) => owner == id,
            None => {
                s.owner = Some(id);
                true
            }
        }
    }
}

pub(super) async fn authenticate(
    State(state): State<Arc<AuthState>>,
    Path(token): Path<String>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(request): Json<AuthRequest>,
) -> StatusCode {
    if !peer.ip().is_loopback() || token != state.callback_token {
        return StatusCode::FORBIDDEN;
    }
    if state.permits(&request) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::FORBIDDEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(id: Uuid, key: &str) -> AuthRequest {
        AuthRequest {
            ip: "192.168.1.10".into(),
            user: String::new(),
            password: String::new(),
            action: "publish".into(),
            path: format!("app/{key}"),
            protocol: "rtmp".into(),
            id: Some(id),
        }
    }

    #[test]
    fn different_keys_cannot_compete_and_only_absence_releases_owner() {
        let state = AuthState::new(Ipv4Addr::new(192, 168, 1, 10));
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert!(state.permits(&input(first, "first")));
        assert!(state.permits(&input(first, "first")));
        assert!(!state.permits(&input(second, "second")));
        state.reconcile(Some(first), &[first]);
        assert!(!state.permits(&input(second, "second")));
        state.reconcile(Some(first), &[]);
        assert!(state.permits(&input(second, "second")));
        state.reconcile(Some(first), &[]);
        assert_eq!(state.owner(), Some(second));
    }

    #[test]
    fn simultaneous_publishers_reserve_exactly_one_slot() {
        let state = Arc::new(AuthState::new(Ipv4Addr::new(192, 168, 1, 10)));
        let barrier = Arc::new(std::sync::Barrier::new(16));
        let attempts: Vec<_> = (0..16)
            .map(|index| {
                let state = state.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    state.permits(&input(Uuid::new_v4(), &format!("key{index}")))
                })
            })
            .collect();
        let accepted = attempts
            .into_iter()
            .map(|attempt| usize::from(attempt.join().unwrap()))
            .sum::<usize>();
        assert_eq!(accepted, 1);
    }

    #[test]
    fn api_and_relay_require_separate_credentials() {
        let state = AuthState::new(Ipv4Addr::LOCALHOST);
        let mut request = input(Uuid::new_v4(), "test");
        request.ip = "127.0.0.1".into();
        request.path = PLAYBACK_PATH.into();
        assert!(!state.permits(&request));
        request.user = "playstreamer-relay".into();
        request.password = state.relay_token.clone();
        assert!(state.permits(&request));
        request.action = "api".into();
        assert!(!state.permits(&request));
        request.user = "playstreamer".into();
        request.password = state.api_token.clone();
        assert!(state.permits(&request));
    }

    #[test]
    fn wrong_ip_missing_id_and_closed_session_are_rejected() {
        let state = AuthState::new(Ipv4Addr::new(192, 168, 1, 10));
        let mut request = input(Uuid::new_v4(), "test");
        request.ip = "192.168.1.11".into();
        assert!(!state.permits(&request));
        request.ip = "192.168.1.10".into();
        request.id = None;
        assert!(!state.permits(&request));
        request.id = Some(Uuid::new_v4());
        state.close();
        assert!(!state.permits(&request));
    }
}
