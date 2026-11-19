use anyhow::{Context, Result, bail};
use hickory_proto::{
    op::{Message, MessageType, OpCode, ResponseCode},
    rr::{RData, Record, RecordType, rdata::A},
};
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Semaphore, watch},
    task::{JoinHandle, JoinSet},
    time::{Duration, timeout},
};

const TARGET: &str = "contribute.live-video.net";
const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct DnsSettings {
    pub bind: SocketAddrV4,
    pub answer_ip: Ipv4Addr,
    pub ps5_ip: Ipv4Addr,
    pub upstreams: Vec<SocketAddrV4>,
}

#[derive(Default)]
pub struct DnsCounters {
    pub queries: AtomicU64,
    pub intercepts: AtomicU64,
}

pub struct DnsService {
    udp: Option<UdpSocket>,
    tcp: Option<TcpListener>,
    settings: DnsSettings,
    pub counters: Arc<DnsCounters>,
    shutdown: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl DnsService {
    pub async fn bind(settings: DnsSettings) -> Result<Self> {
        if settings.upstreams.is_empty() {
            bail!("没有可用的上游 DNS，请在设置中填写");
        }
        let udp = UdpSocket::bind(settings.bind)
            .await
            .context("无法监听 UDP 53，请检查端口占用和防火墙")?;
        let tcp = TcpListener::bind(settings.bind)
            .await
            .context("无法监听 TCP 53，请检查端口占用和防火墙")?;
        let (shutdown, _) = watch::channel(false);
        Ok(Self {
            udp: Some(udp),
            tcp: Some(tcp),
            settings,
            counters: Arc::default(),
            shutdown,
            tasks: vec![],
        })
    }

    pub fn activate(&mut self) {
        let Some(udp) = self.udp.take() else {
            return;
        };
        let tcp = self.tcp.take().expect("paired DNS listeners");
        let udp = Arc::new(udp);
        let settings = Arc::new(self.settings.clone());
        let permits = Arc::new(Semaphore::new(64));
        let mut stop = self.shutdown.subscribe();
        let counters = self.counters.clone();
        let s = settings.clone();
        let p = permits.clone();
        self.tasks.push(tokio::spawn(async move {
            let mut packet = vec![0u8; 4096];
            let mut workers=JoinSet::new();
            loop {
                tokio::select! {
                    _ = stop.changed() => break,
                    _ = workers.join_next(), if !workers.is_empty() => {},
                    result = udp.recv_from(&mut packet) => {
                        let Ok((size, peer)) = result else { break; };
                        if !allowed(peer, &s) { continue; }
                        let Ok(permit) = p.clone().try_acquire_owned() else { continue; };
                        let bytes = packet[..size].to_vec();
                        let socket = udp.clone(); let s = s.clone(); let c = counters.clone();
                        let mut stopped = stop.clone();
                        workers.spawn(async move {
                            let _permit = permit;
                            tokio::select! {
                                _ = stopped.changed() => {},
                                result = resolve(&bytes, &s, &c, false) => {
                                    if let Ok(reply) = result { let _ = socket.send_to(&reply, peer).await; }
                                }
                            }
                        });
                    }
                }
            }
            workers.abort_all();
            while workers.join_next().await.is_some() {}
        }));
        let mut stop = self.shutdown.subscribe();
        let counters = self.counters.clone();
        self.tasks.push(tokio::spawn(async move {
            let mut workers=JoinSet::new();
            loop {
                tokio::select! {
                    _ = stop.changed() => break,
                    _ = workers.join_next(), if !workers.is_empty() => {},
                    result = tcp.accept() => {
                        let Ok((mut stream, peer)) = result else { break; };
                        if !allowed(peer, &settings) { continue; }
                        let Ok(permit) = permits.clone().try_acquire_owned() else { continue; };
                        let s = settings.clone(); let c = counters.clone(); let mut stopped = stop.clone();
                        workers.spawn(async move {
                            let _permit = permit;
                            tokio::select! {
                                _ = stopped.changed() => {},
                                _ = timeout(Duration::from_secs(10), async {
                                    let len = stream.read_u16().await? as usize;
                                    if len < 12 { bail!("invalid DNS frame"); }
                                    let mut bytes = vec![0; len]; stream.read_exact(&mut bytes).await?;
                                    let reply = resolve(&bytes, &s, &c, true).await?;
                                    stream.write_u16(reply.len() as u16).await?;
                                    stream.write_all(&reply).await?;
                                    Ok::<_, anyhow::Error>(())
                                }) => {}
                            }
                        });
                    }
                }
            }
            workers.abort_all();
            while workers.join_next().await.is_some() {}
        }));
    }

    pub fn counts(&self) -> (u64, u64) {
        (
            self.counters.queries.load(Ordering::Relaxed),
            self.counters.intercepts.load(Ordering::Relaxed),
        )
    }
    pub async fn stop(mut self) {
        let _ = self.shutdown.send(true);
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
    }
}

impl Drop for DnsService {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

fn allowed(peer: SocketAddr, settings: &DnsSettings) -> bool {
    peer.ip().is_loopback() || peer.ip() == settings.ps5_ip || peer.ip() == settings.answer_ip
}

pub fn is_target(name: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    name == TARGET
        || name
            .strip_suffix(TARGET)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn response(query: &Message, code: ResponseCode) -> Message {
    let mut result = Message::response(query.id, OpCode::Query);
    result.metadata.recursion_desired = query.recursion_desired;
    result.metadata.recursion_available = true;
    result.metadata.response_code = code;
    for q in &query.queries {
        result.add_query(q.clone());
    }
    result
}

async fn resolve(
    bytes: &[u8],
    settings: &DnsSettings,
    counters: &DnsCounters,
    tcp: bool,
) -> Result<Vec<u8>> {
    let query = Message::from_vec(bytes).context("invalid DNS packet")?;
    if query.message_type != MessageType::Query {
        bail!("unexpected DNS response");
    }
    counters.queries.fetch_add(1, Ordering::Relaxed);
    if query.op_code != OpCode::Query || query.queries.len() != 1 {
        return Ok(response(&query, ResponseCode::FormErr).to_vec()?);
    }
    let question = &query.queries[0];
    if is_target(&question.name().to_utf8()) {
        counters.intercepts.fetch_add(1, Ordering::Relaxed);
        let mut reply = response(&query, ResponseCode::NoError);
        if question.query_type() == RecordType::A {
            reply.add_answer(Record::from_rdata(
                question.name().clone(),
                10,
                RData::A(A(settings.answer_ip)),
            ));
        }
        return Ok(reply.to_vec()?);
    }
    for upstream in &settings.upstreams {
        if let Ok(Ok(mut reply)) = timeout(IO_TIMEOUT, forward(query.clone(), *upstream, tcp)).await
        {
            reply.metadata.id = query.id;
            let output = reply.to_vec()?;
            if !tcp && output.len() > 1232 {
                return Ok(response(&query, ResponseCode::NoError)
                    .truncate()
                    .to_vec()?);
            }
            return Ok(output);
        }
    }
    Ok(response(&query, ResponseCode::ServFail).to_vec()?)
}

async fn forward(mut query: Message, upstream: SocketAddrV4, tcp: bool) -> Result<Message> {
    let random = uuid::Uuid::new_v4();
    query.metadata.id = u16::from_be_bytes([random.as_bytes()[0], random.as_bytes()[1]]);
    let bytes = query.to_vec()?;
    let mut use_tcp = tcp;
    if !tcp {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
        socket.connect(upstream).await?;
        socket.send(&bytes).await?;
        let mut buffer = vec![0u8; 65535];
        let size = socket.recv(&mut buffer).await?;
        let reply = Message::from_vec(&buffer[..size])?;
        validate_reply(&query, &reply)?;
        if !reply.truncation {
            return Ok(reply);
        }
        use_tcp = true;
    }
    if use_tcp {
        let mut stream = TcpStream::connect(upstream).await?;
        stream.write_u16(bytes.len() as u16).await?;
        stream.write_all(&bytes).await?;
        let size = stream.read_u16().await? as usize;
        let mut buffer = vec![0u8; size];
        stream.read_exact(&mut buffer).await?;
        let reply = Message::from_vec(&buffer)?;
        validate_reply(&query, &reply)?;
        return Ok(reply);
    }
    bail!("DNS upstream unavailable")
}

fn validate_reply(query: &Message, reply: &Message) -> Result<()> {
    if reply.id != query.id
        || reply.message_type != MessageType::Response
        || reply.queries != query.queries
    {
        bail!("mismatched DNS reply");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::{op::Query, rr::Name};
    fn query(name: &str, kind: RecordType) -> Vec<u8> {
        let mut m = Message::new(27, MessageType::Query, OpCode::Query);
        m.add_query(Query::query(Name::from_ascii(name).unwrap(), kind));
        m.to_vec().unwrap()
    }
    fn settings() -> DnsSettings {
        DnsSettings {
            bind: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            answer_ip: Ipv4Addr::new(192, 168, 1, 5),
            ps5_ip: Ipv4Addr::LOCALHOST,
            upstreams: vec![SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9)],
        }
    }
    #[test]
    fn domain_boundaries() {
        for n in [TARGET, "x.CONTRIBUTE.live-video.net."] {
            assert!(is_target(n));
        }
        for n in [
            "evilcontribute.live-video.net",
            "contribute.live-video.net.evil",
            "api.twitch.tv",
        ] {
            assert!(!is_target(n));
        }
    }
    #[tokio::test]
    async fn overrides_a_but_never_forwards_target_aaaa_or_cname() {
        for kind in [RecordType::A, RecordType::AAAA, RecordType::CNAME] {
            let b = resolve(
                &query(TARGET, kind),
                &settings(),
                &DnsCounters::default(),
                false,
            )
            .await
            .unwrap();
            let m = Message::from_vec(&b).unwrap();
            assert_eq!(m.id, 27);
            assert_eq!(m.response_code, ResponseCode::NoError);
            assert_eq!(m.answers.len(), usize::from(kind == RecordType::A));
        }
    }
    #[tokio::test]
    async fn ordinary_query_is_forwarded_and_id_restored() {
        let upstream = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = upstream.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut buf = [0; 4096];
            let (n, peer) = upstream.recv_from(&mut buf).await.unwrap();
            let q = Message::from_vec(&buf[..n]).unwrap();
            let reply = response(&q, ResponseCode::NoError).to_vec().unwrap();
            upstream.send_to(&reply, peer).await.unwrap();
        });
        let mut s = settings();
        s.upstreams = vec![match addr {
            SocketAddr::V4(v) => v,
            _ => unreachable!(),
        }];
        let bytes = resolve(
            &query("example.org", RecordType::A),
            &s,
            &DnsCounters::default(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(Message::from_vec(&bytes).unwrap().id, 27);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn occupied_tcp_port_releases_udp_reservation() {
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = tcp.local_addr().unwrap();
        let mut s = settings();
        s.bind = match addr {
            SocketAddr::V4(v) => v,
            _ => unreachable!(),
        };
        assert!(DnsService::bind(s).await.is_err());
        assert!(UdpSocket::bind(addr).await.is_ok());
    }
    #[tokio::test]
    async fn truncated_upstream_udp_retries_tcp() {
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = tcp.local_addr().unwrap();
        let udp = UdpSocket::bind(address).await.unwrap();
        let responder = tokio::spawn(async move {
            let mut bytes = [0; 4096];
            let (n, peer) = udp.recv_from(&mut bytes).await.unwrap();
            let q = Message::from_vec(&bytes[..n]).unwrap();
            udp.send_to(
                &response(&q, ResponseCode::NoError)
                    .truncate()
                    .to_vec()
                    .unwrap(),
                peer,
            )
            .await
            .unwrap();
            let (mut socket, _) = tcp.accept().await.unwrap();
            let length = socket.read_u16().await.unwrap();
            let mut bytes = vec![0; length as usize];
            socket.read_exact(&mut bytes).await.unwrap();
            let q = Message::from_vec(&bytes).unwrap();
            let reply = response(&q, ResponseCode::NXDomain).to_vec().unwrap();
            socket.write_u16(reply.len() as u16).await.unwrap();
            socket.write_all(&reply).await.unwrap();
        });
        let mut s = settings();
        s.upstreams = vec![match address {
            SocketAddr::V4(a) => a,
            _ => unreachable!(),
        }];
        let bytes = resolve(
            &query("example.org", RecordType::A),
            &s,
            &DnsCounters::default(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            Message::from_vec(&bytes).unwrap().response_code,
            ResponseCode::NXDomain
        );
        responder.await.unwrap();
    }
    #[tokio::test]
    async fn wrong_transaction_id_is_not_accepted() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut b = [0; 1024];
            let (n, p) = socket.recv_from(&mut b).await.unwrap();
            let q = Message::from_vec(&b[..n]).unwrap();
            let mut reply = response(&q, ResponseCode::NoError);
            reply.metadata.id = q.id.wrapping_add(1);
            let bytes = reply.to_vec().unwrap();
            socket.send_to(&bytes, p).await.unwrap();
        });
        let mut s = settings();
        s.upstreams = vec![match address {
            SocketAddr::V4(a) => a,
            _ => unreachable!(),
        }];
        let bytes = resolve(
            &query("example.org", RecordType::A),
            &s,
            &DnsCounters::default(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            Message::from_vec(&bytes).unwrap().response_code,
            ResponseCode::ServFail
        );
        task.await.unwrap();
    }
    #[tokio::test]
    async fn stop_releases_tcp_and_udp_and_answers_over_both() {
        let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let mut s = settings();
        s.bind = match address {
            SocketAddr::V4(a) => a,
            _ => unreachable!(),
        };
        let mut service = DnsService::bind(s).await.unwrap();
        service.activate();
        let bytes = query(TARGET, RecordType::A);
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream.write_u16(bytes.len() as u16).await.unwrap();
        stream.write_all(&bytes).await.unwrap();
        let len = stream.read_u16().await.unwrap();
        let mut reply = vec![0; len as usize];
        stream.read_exact(&mut reply).await.unwrap();
        assert_eq!(Message::from_vec(&reply).unwrap().answers.len(), 1);
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        udp.send_to(&bytes, address).await.unwrap();
        let mut reply = [0; 1024];
        let (n, _) = timeout(Duration::from_secs(2), udp.recv_from(&mut reply))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(Message::from_vec(&reply[..n]).unwrap().answers.len(), 1);
        service.stop().await;
        assert!(TcpListener::bind(address).await.is_ok());
        assert!(UdpSocket::bind(address).await.is_ok());
    }
}
