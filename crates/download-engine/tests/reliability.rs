//! 引擎可靠性测试：手写最小 mock HTTP 服务器，重放真实 CDN 剧本——
//! 分片被提前截断（网络抖动）、重试续传、重试耗尽、总长校验。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use download_engine::{download, segment_ranges, DownloadError, DownloadOptions, ProgressEvent};

#[derive(Default)]
struct Behavior {
    /// 首个 Range 请求只回 `truncate_at` 字节后硬断连接（模拟传输中断）
    truncate_first_range_at: Option<u64>,
    /// 所有 Range 请求都只回 10 字节即断（重试也救不回来）
    truncate_every_range: bool,
    /// 是否回 Accept-Ranges 头
    accept_ranges: bool,
}

async fn spawn_mock(payload: Arc<Vec<u8>>, behavior: Arc<Behavior>) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let first_range_served = Arc::new(AtomicBool::new(false));
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else { break };
            let payload = Arc::clone(&payload);
            let behavior = Arc::clone(&behavior);
            let first = Arc::clone(&first_range_served);
            tokio::spawn(handle(stream, payload, behavior, first));
        }
    });
    addr
}

async fn handle(mut stream: TcpStream, payload: Arc<Vec<u8>>, behavior: Arc<Behavior>, first: Arc<AtomicBool>) {
    let mut buf = Vec::new();
    // 读到请求头结束
    loop {
        let mut chunk = [0u8; 4096];
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let range = text
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("range:"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| parse_range(v.trim()));

    match range.flatten() {
        Some((a, b)) => {
            let is_first = !first.swap(true, Ordering::SeqCst);
            let slice = &payload[a as usize..=(b as usize).min(payload.len() - 1)];
            if (behavior.truncate_every_range)
                || (behavior.truncate_first_range_at.is_some() && is_first)
            {
                let cut = if behavior.truncate_every_range {
                    10u64.min(slice.len() as u64)
                } else {
                    behavior.truncate_first_range_at.unwrap().min(slice.len() as u64)
                };
                // 回完整声明长度，但只发一半内容就硬断 → 客户端读到截断
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\n\r\n",
                    slice.len(), a, b, payload.len()
                );
                stream.write_all(head.as_bytes()).await.ok();
                stream.write_all(&slice[..cut as usize]).await.ok();
                stream.flush().await.ok();
                stream.shutdown().await.ok();
                return;
            }
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\n\r\n",
                slice.len(), a, b, payload.len()
            );
            stream.write_all(head.as_bytes()).await.ok();
            stream.write_all(slice).await.ok();
            stream.flush().await.ok();
        }
        None => {
            let accept = if behavior.accept_ranges { "Accept-Ranges: bytes\r\n" } else { "" };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{accept}\r\n", payload.len());
            stream.write_all(head.as_bytes()).await.ok();
            stream.write_all(&payload).await.ok();
            stream.flush().await.ok();
        }
    }
}

fn parse_range(v: &str) -> Option<(u64, u64)> {
    let rest = v.strip_prefix("bytes=")?;
    let (a, b) = rest.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn options(url: String, dir: &std::path::Path, segments: usize) -> DownloadOptions {
    DownloadOptions {
        url,
        headers: vec![("User-Agent".into(), "engine-test".into())],
        save_path: dir.join("out.bin"),
        segment_count: segments,
    }
}

fn collect(events: &[ProgressEvent]) -> (Option<u64>, Option<u64>) {
    (
        events.iter().find_map(|e| match e {
            ProgressEvent::Started { total } => Some(*total),
            _ => None,
        }),
        events.iter().find_map(|e| match e {
            ProgressEvent::Done { size } => Some(*size),
            _ => None,
        }),
    )
}

#[tokio::test]
async fn truncated_segment_retries_and_completes() {
    // 12MB，4 段，每段 3MB；首个 Range 请求在 1KB 处被硬断 → 续传重试后应完整
    let payload: Vec<u8> = (0..(12 * 1024 * 1024)).map(|i| (i % 251) as u8).collect();
    let payload = Arc::new(payload);
    let addr = spawn_mock(
        Arc::clone(&payload),
        Arc::new(Behavior {
            truncate_first_range_at: Some(1024),
            accept_ranges: true,
            ..Default::default()
        }),
    )
    .await;

    let dir = tempfile_dir();
    let opts = options(format!("http://{addr}/file.bin"), &dir, 4);
    let cancel = CancellationToken::new();
    let mut events = Vec::new();
    let size = tokio::time::timeout(Duration::from_secs(120), async {
        download(opts, cancel, |e| events.push(e)).await
    })
    .await
    .expect("测试超时")
    .expect("下载应成功");

    assert_eq!(size as usize, payload.len());
    let (started, done) = collect(&events);
    assert_eq!(started, Some(payload.len() as u64));
    assert_eq!(done, Some(payload.len() as u64));
    let on_disk = std::fs::read(dir.join("out.bin")).unwrap();
    assert_eq!(on_disk.len(), payload.len());
    assert!(on_disk == *payload, "内容必须逐字节一致");
    // 临时目录已清理
    let leftovers: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(".dl_temp_"))
        .collect();
    assert!(leftovers.is_empty(), "不应残留临时目录: {leftovers:?}");
}

#[tokio::test]
async fn single_stream_small_file() {
    // 5KB 无 Range 支持 → 单流路径
    let payload: Vec<u8> = (0..5 * 1024).map(|i| (i % 199) as u8).collect();
    let payload = Arc::new(payload);
    let addr = spawn_mock(Arc::clone(&payload), Arc::new(Behavior::default())).await;

    let dir = tempfile_dir();
    let opts = options(format!("http://{addr}/small.bin"), &dir, 8);
    let size = download(opts, CancellationToken::new(), |_| {}).await.unwrap();
    assert_eq!(size as usize, payload.len());
    assert_eq!(std::fs::read(dir.join("out.bin")).unwrap(), *payload);
}

#[tokio::test]
async fn segment_incomplete_after_retries_fails_cleanly() {
    // 每次 Range 都只给 10 字节 → 3 次重试后必须显式失败，且不留任何文件
    let payload: Vec<u8> = vec![7u8; 12 * 1024 * 1024];
    let addr = spawn_mock(
        Arc::new(payload),
        Arc::new(Behavior {
            truncate_every_range: true,
            accept_ranges: true,
            ..Default::default()
        }),
    )
    .await;

    let dir = tempfile_dir();
    let opts = options(format!("http://{addr}/doomed.bin"), &dir, 4);
    let result = tokio::time::timeout(Duration::from_secs(180), async {
        download(opts, CancellationToken::new(), |_| {}).await
    })
    .await
    .expect("测试超时");

    match result {
        Err(DownloadError::SegmentIncomplete { .. }) => {}
        other => panic!("应报 SegmentIncomplete，实际: {other:?}"),
    }
    assert!(!dir.join("out.bin").exists(), "失败时不得留下最终文件");
    let leftovers: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(".dl_temp_"))
        .collect();
    assert!(leftovers.is_empty(), "失败时不得残留临时目录: {leftovers:?}");
}

#[tokio::test]
async fn merge_size_mismatch_deletes_file() {
    // 对合并阶段的直接验证：分片合计与期望总长不符 → 删除产物并报 SizeMismatch
    let dir = tempfile_dir();
    let temp = dir.join(".dl_temp_test");
    std::fs::create_dir_all(&temp).unwrap();
    std::fs::write(temp.join("chunk_0"), b"0123456789").unwrap();
    std::fs::write(temp.join("chunk_1"), b"abcdefghij").unwrap();
    // 预先放一个"旧文件"，验证损坏产物会被删除而非被旧内容冒充
    std::fs::write(dir.join("out.bin"), b"stale").unwrap();

    let result = download_engine::merge_chunks(&dir.join("out.bin"), &temp, 2, 100).await;
    match result {
        Err(DownloadError::SizeMismatch { actual, expected }) => {
            assert_eq!((actual, expected), (20, 100));
        }
        other => panic!("应报 SizeMismatch，实际: {other:?}"),
    }
    assert!(!dir.join("out.bin").exists(), "大小不符的产物必须被删除");
}

#[test]
fn ranges_math_uses_last_segment_closure() {
    // 12MB / 5 段：数学上确认最后一段收在 total-1
    let total = 12u64 * 1024 * 1024;
    let ranges = segment_ranges(total, 5);
    assert_eq!(ranges.len(), 5);
    assert_eq!(ranges.last().unwrap().1, total - 1);
    let covered: u64 = ranges.iter().map(|(a, b)| b - a + 1).sum();
    assert_eq!(covered, total);
}

fn tempfile_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("engine-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}
