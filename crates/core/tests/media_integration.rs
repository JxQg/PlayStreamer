//! Run explicitly with pinned MediaMTX and FFmpeg binaries; never contacts a streaming platform.
use anyhow::{Context, Result, bail};
use playstreamer_core::media::MediaSession;
use std::{
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    time::{Instant, sleep, timeout},
};

fn command(binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    command
}

fn publish(ffmpeg: &Path, key: &str) -> Result<Child> {
    command(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x180:rate=15",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=1000:sample_rate=44100",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-pix_fmt",
            "yuv420p",
            "-g",
            "15",
            "-c:a",
            "aac",
            "-f",
            "flv",
        ])
        .arg(format!("rtmp://127.0.0.1:1935/app/{key}"))
        .spawn()
        .context("cannot start synthetic publisher")
}

async fn wait_ready(media: &mut MediaSession, expected: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if media.inspect().await?.ready == expected {
            return Ok(());
        }
        sleep(Duration::from_millis(150)).await;
    }
    bail!("media readiness did not become {expected}")
}

#[tokio::test]
#[ignore = "requires PLAYSTREAMER_MEDIAMTX and PLAYSTREAMER_FFMPEG; binds local port 1935"]
async fn synthetic_ingest_alias_competition_recovery_and_cleanup() -> Result<()> {
    let mediamtx = PathBuf::from(
        std::env::var_os("PLAYSTREAMER_MEDIAMTX").context("set PLAYSTREAMER_MEDIAMTX")?,
    );
    let ffmpeg =
        PathBuf::from(std::env::var_os("PLAYSTREAMER_FFMPEG").context("set PLAYSTREAMER_FFMPEG")?);
    let runtime = tempfile::tempdir()?;
    let mut media = MediaSession::start(&mediamtx, runtime.path(), Ipv4Addr::LOCALHOST).await?;
    let mut first = publish(&ffmpeg, "live_integration_private_first")?;
    wait_ready(&mut media, true).await?;
    let first_stats = media.inspect().await?;
    assert!(first_stats.bytes_received > 0);
    assert!(!first_stats.tracks.is_empty());

    let mut competitor = publish(&ffmpeg, "live_integration_private_second")?;
    let rejected = timeout(Duration::from_secs(10), competitor.wait())
        .await
        .context("second input was not rejected")??;
    assert!(
        !rejected.success(),
        "second path must not compete with the reserved input"
    );
    assert!(media.inspect().await?.ready);

    let mut reader = command(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-rw_timeout",
            "10000000",
            "-i",
            "rtmp://127.0.0.1:1935/live/playstation",
            "-t",
            "2",
            "-f",
            "null",
            "-",
        ])
        .spawn()?;
    let reader_deadline = Instant::now() + Duration::from_secs(8);
    let mut observed_reader = false;
    while Instant::now() < reader_deadline {
        if media.inspect().await?.readers > 0 {
            observed_reader = true;
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    assert!(
        observed_reader,
        "playback clients must appear in the status snapshot"
    );
    let read_status = timeout(Duration::from_secs(15), reader.wait())
        .await
        .context("fixed playback address did not yield frames")??;
    assert!(
        read_status.success(),
        "fixed playback address must decode synthetic audio/video"
    );

    first.kill().await?;
    first.wait().await?;
    wait_ready(&mut media, false).await?;
    // A successful empty connection snapshot releases the first publisher's reservation.
    sleep(Duration::from_millis(250)).await;
    media.inspect().await?;
    let mut replacement = publish(&ffmpeg, "live_integration_private_replacement")?;
    wait_ready(&mut media, true).await?;
    media.stop().await?;
    media.stop().await?;
    let _ = replacement.kill().await;
    let _ = replacement.wait().await;
    assert!(!media.take_logs().join("\n").contains("integration_private"));
    assert_eq!(std::fs::read_dir(runtime.path())?.count(), 0);
    let port = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 1935)).await?;
    assert!(
        MediaSession::start(&mediamtx, runtime.path(), Ipv4Addr::LOCALHOST)
            .await
            .is_err(),
        "occupied media port must roll back startup"
    );
    assert_eq!(std::fs::read_dir(runtime.path())?.count(), 0);
    drop(port);
    // RAII also stops the child if a caller abandons a live session without awaiting stop.
    let abandoned = MediaSession::start(&mediamtx, runtime.path(), Ipv4Addr::LOCALHOST).await?;
    drop(abandoned);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(port) = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 1935)).await {
            drop(port);
            break;
        }
        if Instant::now() >= deadline {
            bail!("dropped session left media listener running");
        }
        sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}

#[tokio::test]
async fn missing_sidecar_does_not_create_runtime_files() -> Result<()> {
    let runtime = tempfile::tempdir()?;
    assert!(
        MediaSession::start(
            &runtime.path().join("missing-mediamtx"),
            runtime.path(),
            Ipv4Addr::LOCALHOST
        )
        .await
        .is_err()
    );
    assert_eq!(std::fs::read_dir(runtime.path())?.count(), 0);
    Ok(())
}
