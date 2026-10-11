//! 通用 HTTP 分片下载引擎。
//!
//! 职责边界（ADR-0001）：只懂「从 URL 下载文件」——分片、重试、逐段校验、
//! 总长校验、进度事件、取消。不感知夸克或任何具体网盘；调用方提供全部请求头。
//!
//! 可靠性规则（spec R1.2 / R1.3）：
//! - 每个分片完成后校验字节数等于区间长度，不足即按指数退避重试（至多 3 次），
//!   重试从已收到的偏移续传而非从头重来；
//! - 重试耗尽仍不足 → 整个任务显式失败，无残留临时文件；
//! - 合并后校验最终文件大小 == content-length，不符则删除并报错。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_SEGMENT_COUNT: usize = 12;
pub const MAX_SEGMENT_COUNT: usize = 64;
pub const MIN_MULTITHREAD_SIZE: u64 = 10 * 1024 * 1024;
const MAX_REDIRECTS: u32 = 5;
const SEGMENT_RETRIES: u32 = 3;
const RETRY_BASE_DELAY: Duration = Duration::from_secs(1);
/// 退避上限，避免高次重试等待过久
const RETRY_MAX_DELAY: Duration = Duration::from_secs(16);

/// 指数退避：1s、2s、4s…（attempt 从 1 起），封顶 [`RETRY_MAX_DELAY`]
fn backoff_delay(attempt: u32) -> Duration {
    let exp = RETRY_BASE_DELAY.saturating_mul(1u32 << attempt.min(5).saturating_sub(1));
    exp.min(RETRY_MAX_DELAY)
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("请求失败: {0}")]
    Request(String),
    #[error("重定向次数过多")]
    TooManyRedirects,
    #[error("CDN 返回错误 {status}: {body}")]
    CdnError { status: u16, body: String },
    #[error("分片 {index} 在 {retries} 次重试后仍不完整: 已收 {received}/{expected} 字节")]
    SegmentIncomplete {
        index: usize,
        retries: u32,
        received: u64,
        expected: u64,
    },
    #[error("合并后文件大小不符: 实际 {actual} / 期望 {expected}，已删除损坏文件")]
    SizeMismatch { actual: u64, expected: u64 },
    #[error("IO 错误: {0}")]
    Io(String),
    #[error("下载已取消")]
    Cancelled,
}

/// 单次下载的全部输入。调用方负责把保存路径解析成不冲突的完整文件路径。
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub url: String,
    /// 逐条原始请求头（如 Cookie、Referer、User-Agent），引擎不感知其语义。
    pub headers: Vec<(String, String)>,
    pub save_path: PathBuf,
    /// 期望的分片数；实际是否分片还取决于文件大小与 CDN 是否支持 Range。
    pub segment_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressEvent {
    /// 已确定总大小、开始传输
    Started { total: u64 },
    /// 传输中（约每 500ms 一次）
    Progress {
        downloaded: u64,
        total: u64,
        speed: f64,
    },
    /// 分片全部落盘，正在合并
    Merging,
    /// 成功完成，size 为最终文件大小（已通过总长校验）
    Done { size: u64 },
}

/// 计算分片字节区间。最后一段以 `total - 1` 收尾，保证不丢尾字节。
pub fn segment_ranges(total: u64, segments: usize) -> Vec<(u64, u64)> {
    let segments = segments.max(1) as u64;
    let seg = total / segments;
    (0..segments)
        .map(|i| {
            let start = i * seg;
            let end = if i == segments - 1 {
                total - 1
            } else {
                start + seg - 1
            };
            (start, end)
        })
        .collect()
}

fn build_client() -> Result<reqwest::Client, DownloadError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(30))
        .tcp_keepalive(Duration::from_secs(60))
        .pool_idle_timeout(Duration::from_secs(90))
        .http1_only()
        .build()
        .map_err(|e| DownloadError::Request(e.to_string()))
}

fn apply_headers(builder: reqwest::RequestBuilder, headers: &[(String, String)]) -> reqwest::RequestBuilder {
    let mut b = builder;
    for (k, v) in headers {
        b = b.header(k, v);
    }
    b
}

/// 手动跟随 302（下载 URL 常带时效签名，跟随过程中保持调用方请求头）。
async fn resolve_final_url(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
    cancel: &CancellationToken,
) -> Result<String, DownloadError> {
    let mut current = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let resp = cancel
            .run_until_cancelled(
                apply_headers(client.get(&current), headers).send(),
            )
            .await
            .ok_or(DownloadError::Cancelled)?
            .map_err(|e| DownloadError::Request(e.to_string()))?;
        if resp.status().is_redirection() {
            let loc = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| DownloadError::Request("重定向缺少 Location".into()))?
                .to_string();
            current = resolve_relative(&current, &loc);
            continue;
        }
        return Ok(current);
    }
    Err(DownloadError::TooManyRedirects)
}

fn resolve_relative(base: &str, loc: &str) -> String {
    match url::Url::parse(base).and_then(|u| u.join(loc)) {
        Ok(u) => u.to_string(),
        Err(_) => loc.to_string(),
    }
}

async fn probe(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
    cancel: &CancellationToken,
) -> Result<(u64, bool), DownloadError> {
    let resp = cancel
        .run_until_cancelled(apply_headers(client.get(url), headers).send())
        .await
        .ok_or(DownloadError::Cancelled)?
        .map_err(|e| DownloadError::Request(e.to_string()))?;
    let status = resp.status().as_u16();
    if status >= 400 {
        let body = resp.text().await.unwrap_or_default();
        return Err(DownloadError::CdnError {
            status,
            body: truncate_utf8(&body, 200),
        });
    }
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let ranges = resp
        .headers()
        .get("accept-ranges")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("bytes"))
        .unwrap_or(false);
    Ok((total, ranges))
}

fn truncate_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// 执行一次下载。完成后（无论成败）清理临时目录；取消时同样清理。
pub async fn download(
    options: DownloadOptions,
    cancel: CancellationToken,
    mut on_event: impl FnMut(ProgressEvent) + Send,
) -> Result<u64, DownloadError> {
    if cancel.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }
    let client = build_client()?;
    let final_url = resolve_final_url(&client, &options.url, &options.headers, &cancel).await?;
    let (total, ranges_ok) = probe(&client, &final_url, &options.headers, &cancel).await?;
    on_event(ProgressEvent::Started { total });

    let segment_count = options.segment_count.clamp(1, MAX_SEGMENT_COUNT);
    let size = if ranges_ok && total >= MIN_MULTITHREAD_SIZE && segment_count > 1 {
        download_segmented(
            &client,
            &final_url,
            &options,
            total,
            segment_count,
            &cancel,
            &mut on_event,
        )
        .await?
    } else {
        download_single_stream(&client, &final_url, &options, total, &cancel, &mut on_event).await?
    };

    if cancel.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }
    on_event(ProgressEvent::Done { size });
    Ok(size)
}

/// 分片区间计算的自检：引擎自身逻辑必须先保证数学正确。
fn validate_ranges(total: u64, segments: usize) -> Result<(), DownloadError> {
    let ranges = segment_ranges(total, segments);
    let expected_len = ranges
        .last()
        .map(|(_, end)| *end + 1)
        .ok_or(DownloadError::Request("空区间".into()))?;
    if expected_len != total {
        return Err(DownloadError::Request(format!(
            "区间划分错误: 覆盖 {expected_len} / 总长 {total}"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_segmented(
    client: &reqwest::Client,
    url: &str,
    options: &DownloadOptions,
    total: u64,
    segment_count: usize,
    cancel: &CancellationToken,
    on_event: &mut (impl FnMut(ProgressEvent) + Send),
) -> Result<u64, DownloadError> {
    validate_ranges(total, segment_count)?;
    let ranges = segment_ranges(total, segment_count);

    let temp_dir = options
        .save_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".dl_temp_{}", &uuid::Uuid::new_v4().to_string()[..8]));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| DownloadError::Io(format!("创建临时目录失败: {e}")))?;

    let result = run_segments(client, url, options, &ranges, &temp_dir, total, cancel, on_event).await;

    // 无论成败都清理临时目录；成 FName 则最终文件已就位。
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn run_segments(
    client: &reqwest::Client,
    url: &str,
    options: &DownloadOptions,
    ranges: &[(u64, u64)],
    temp_dir: &Path,
    total: u64,
    cancel: &CancellationToken,
    on_event: &mut (impl FnMut(ProgressEvent) + Send),
) -> Result<u64, DownloadError> {
    enum Msg {
        Progress(ProgressEvent),
        SegmentDone(Result<(), DownloadError>),
    }

    let progresses: Vec<Arc<AtomicU64>> = ranges.iter().map(|_| Arc::new(AtomicU64::new(0))).collect();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    // 进度聚合协程：每 500ms 汇报一次，速度按窗口差分；经通道交给主循环转发，
    // 回调引用因此不必跨协程（'static）存活。
    let agg_progresses = progresses.clone();
    let agg_cancel = cancel.clone();
    let agg_tx = tx.clone();
    let agg = tokio::spawn(async move {
        let mut last_bytes: u64 = 0;
        let mut last_instant = std::time::Instant::now();
        loop {
            if agg_cancel.is_cancelled() {
                return;
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                _ = agg_cancel.cancelled() => return,
            }
            let downloaded: u64 = agg_progresses.iter().map(|a| a.load(Ordering::Relaxed)).sum();
            let now = std::time::Instant::now();
            let dt = now.duration_since(last_instant).as_secs_f64();
            let speed = if dt > 0.0 { (downloaded - last_bytes) as f64 / dt } else { 0.0 };
            last_bytes = downloaded;
            last_instant = now;
            let _ = agg_tx.send(Msg::Progress(ProgressEvent::Progress { downloaded, total, speed }));
        }
    });

    for (index, &(start, end)) in ranges.iter().enumerate() {
        let client = client.clone();
        let url = url.to_string();
        let headers = options.headers.clone();
        let chunk_path = temp_dir.join(format!("chunk_{index}"));
        let progress = Arc::clone(&progresses[index]);
        let cancel = cancel.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = download_segment_with_retry(
                &client, &url, &headers, &chunk_path, start, end, index, progress, &cancel,
            )
            .await;
            let _ = tx.send(Msg::SegmentDone(result));
        });
    }
    // 主循环持有一个发送端克隆之外的接收端；显式丢掉本端的发送端，
    // 使全部分片完成后通道能自然收尾（聚合器被 abort 兜底）。
    drop(tx);

    let mut completed = 0usize;
    let mut failure: Option<DownloadError> = None;
    while completed < ranges.len() {
        let msg = tokio::select! {
            maybe = rx.recv() => maybe,
            _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
        };
        match msg {
            Some(Msg::Progress(ev)) => on_event(ev),
            Some(Msg::SegmentDone(result)) => {
                completed += 1;
                if result.is_err() && failure.is_none() {
                    failure = result.err();
                }
            }
            None => break,
        }
    }
    agg.abort();

    if let Some(e) = failure {
        return Err(e);
    }
    if cancel.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }

    on_event(ProgressEvent::Merging);
    merge_chunks(&options.save_path, temp_dir, ranges.len(), total).await
}

/// 合并已下载的分片并校验总长。公开给调用方复用（如断点恢复后的手动合并）。
pub async fn merge_chunks(
    save_path: &Path,
    temp_dir: &Path,
    chunk_count: usize,
    expected_total: u64,
) -> Result<u64, DownloadError> {
    let final_file = tokio::fs::File::create(save_path)
        .await
        .map_err(|e| DownloadError::Io(format!("创建最终文件失败: {e}")))?;
    let mut writer = tokio::io::BufWriter::with_capacity(8 * 1024 * 1024, final_file);
    let mut actual: u64 = 0;
    for i in 0..chunk_count {
        let chunk_path = temp_dir.join(format!("chunk_{i}"));
        let mut chunk = tokio::fs::File::open(&chunk_path)
            .await
            .map_err(|e| DownloadError::Io(format!("打开分片 {i} 失败: {e}")))?;
        let n = tokio::io::copy(&mut chunk, &mut writer)
            .await
            .map_err(|e| DownloadError::Io(format!("合并分片 {i} 失败: {e}")))?;
        actual += n;
    }
    writer
        .flush()
        .await
        .map_err(|e| DownloadError::Io(format!("flush 失败: {e}")))?;
    drop(writer);

    if actual != expected_total {
        // R1.2：总长不符 → 删除损坏文件并显式报错。
        let _ = tokio::fs::remove_file(save_path).await;
        return Err(DownloadError::SizeMismatch {
            actual,
            expected: expected_total,
        });
    }
    Ok(actual)
}

/// 单个分片：带重试的续传下载。重试从「已收到字节数」处用 Range 续传，
/// 耗尽 `SEGMENT_RETRIES` 次仍不足区间长度 → `SegmentIncomplete`。
#[allow(clippy::too_many_arguments)]
async fn download_segment_with_retry(
    client: &reqwest::Client,
    url: &str,
    headers: &[(String, String)],
    chunk_path: &Path,
    start: u64,
    end: u64,
    index: usize,
    progress: Arc<AtomicU64>,
    cancel: &CancellationToken,
) -> Result<(), DownloadError> {
    let expected = end - start + 1;
    let mut attempt: u32 = 0;
    let mut received: u64 = 0;

    loop {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }

        let range = format!("bytes={}-{}", start + received, end);
        let resp = cancel
            .run_until_cancelled(
                apply_headers(client.get(url), headers)
                    .header("Range", &range)
                    .send(),
            )
            .await
            .ok_or(DownloadError::Cancelled)?
            .map_err(|e| DownloadError::Request(e.to_string()))?;

        let status = resp.status().as_u16();
        if status == 200 {
            return Err(DownloadError::Io(format!(
                "段{index}: CDN 不支持 Range（返回 200 而非 206）"
            )));
        }
        if status != 206 {
            return Err(DownloadError::CdnError {
                status,
                body: format!("段{index} 期望 206"),
            });
        }

        // 续传：追加写；首尝试则新建。
        let file = if received == 0 {
            tokio::fs::File::create(chunk_path).await
        } else {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(chunk_path)
                .await
        }
        .map_err(|e| DownloadError::Io(format!("段{index} 打开临时文件失败: {e}")))?;
        let mut writer = tokio::io::BufWriter::with_capacity(1024 * 1024, file);
        let mut resp = resp;

        let mut truncated = false;
        loop {
            if cancel.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }
            match resp.chunk().await {
                Ok(Some(bytes)) => {
                    writer
                        .write_all(&bytes)
                        .await
                        .map_err(|e| DownloadError::Io(format!("段{index} 写入失败: {e}")))?;
                    received += bytes.len() as u64;
                    progress.store(received, Ordering::Relaxed);
                    if received >= expected {
                        break;
                    }
                }
                Ok(None) => {
                    // 连接正常收尾：若还没收满说明被提前截断。
                    truncated = received < expected;
                    break;
                }
                Err(e) => {
                    // 传输中断：记录已收字节数，走重试续传。
                    let _ = writer.flush().await;
                    let _ = e;
                    truncated = true;
                    break;
                }
            }
        }
        writer
            .flush()
            .await
            .map_err(|e| DownloadError::Io(format!("段{index} flush 失败: {e}")))?;

        if !truncated && received == expected {
            return Ok(());
        }

        attempt += 1;
        if attempt > SEGMENT_RETRIES {
            return Err(DownloadError::SegmentIncomplete {
                index,
                retries: SEGMENT_RETRIES,
                received,
                expected,
            });
        }
        // 指数退避后从 received 偏移续传。
        tokio::time::sleep(backoff_delay(attempt)).await;
    }
}

/// 单流下载（Range 不可用或小文件）：同样支持中断续传与总长校验。
async fn download_single_stream(
    client: &reqwest::Client,
    url: &str,
    options: &DownloadOptions,
    total: u64,
    cancel: &CancellationToken,
    on_event: &mut (impl FnMut(ProgressEvent) + Send),
) -> Result<u64, DownloadError> {
    let mut attempt: u32 = 0;
    let mut received: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut last_bytes: u64 = 0;

    loop {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        let builder = apply_headers(client.get(url), &options.headers);
        let builder = if received > 0 {
            builder.header("Range", format!("bytes={received}-"))
        } else {
            builder
        };
        let resp = cancel
            .run_until_cancelled(builder.send())
            .await
            .ok_or(DownloadError::Cancelled)?
            .map_err(|e| DownloadError::Request(e.to_string()))?;
        let status = resp.status().as_u16();
        if status >= 400 {
            return Err(DownloadError::CdnError {
                status,
                body: String::new(),
            });
        }

        let file = if received == 0 {
            tokio::fs::File::create(&options.save_path).await
        } else {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&options.save_path)
                .await
        }
        .map_err(|e| DownloadError::Io(format!("创建文件失败: {e}")))?;
        let mut writer = tokio::io::BufWriter::with_capacity(8 * 1024 * 1024, file);
        let mut resp = resp;

        let mut ended = false;
        loop {
            if cancel.is_cancelled() {
                let _ = tokio::fs::remove_file(&options.save_path).await;
                return Err(DownloadError::Cancelled);
            }
            match resp.chunk().await {
                Ok(Some(bytes)) => {
                    if let Err(e) = writer.write_all(&bytes).await {
                        let _ = tokio::fs::remove_file(&options.save_path).await;
                        return Err(DownloadError::Io(format!("写入失败: {e}")));
                    }
                    received += bytes.len() as u64;
                    if last_emit.elapsed() >= Duration::from_millis(500) {
                        let now = std::time::Instant::now();
                        let dt = now.duration_since(last_emit).as_secs_f64();
                        let speed = if dt > 0.0 { (received - last_bytes) as f64 / dt } else { 0.0 };
                        last_bytes = received;
                        last_emit = now;
                        on_event(ProgressEvent::Progress {
                            downloaded: received,
                            total,
                            speed,
                        });
                    }
                }
                Ok(None) => {
                    ended = true;
                    break;
                }
                Err(_) => {
                    writer.flush().await.ok();
                    break; // 中断 → 走重试续传
                }
            }
        }
        writer
            .flush()
            .await
            .map_err(|e| DownloadError::Io(format!("flush 失败: {e}")))?;

        let complete = ended && (total == 0 || received >= total);
        if complete {
            return Ok(received);
        }

        attempt += 1;
        if attempt > SEGMENT_RETRIES {
            // R1.2：失败不留残留——单流是直接写最终路径的，必须删掉半成品
            let _ = tokio::fs::remove_file(&options.save_path).await;
            return Err(DownloadError::SegmentIncomplete {
                index: 0,
                retries: SEGMENT_RETRIES,
                received,
                expected: total,
            });
        }
        tokio::time::sleep(backoff_delay(attempt)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_cover_exactly() {
        // 10 / 3 → (0,2)(3,5)(6,9)，最后一段收在 total-1
        assert_eq!(segment_ranges(10, 3), vec![(0, 2), (3, 5), (6, 9)]);
        // 整除边界：9 / 3 → (0,2)(3,5)(6,8)
        assert_eq!(segment_ranges(9, 3), vec![(0, 2), (3, 5), (6, 8)]);
        // 单段
        assert_eq!(segment_ranges(7, 1), vec![(0, 6)]);
        // 校验器不误报
        assert!(validate_ranges(10, 3).is_ok());
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        assert_eq!(backoff_delay(1), Duration::from_secs(1));
        assert_eq!(backoff_delay(2), Duration::from_secs(2));
        assert_eq!(backoff_delay(3), Duration::from_secs(4));
        assert_eq!(backoff_delay(4), Duration::from_secs(8));
        // 封顶：极高次数不再无限增长
        assert_eq!(backoff_delay(9), RETRY_MAX_DELAY);
        assert_eq!(backoff_delay(30), RETRY_MAX_DELAY);
    }

    #[test]
    fn utf8_truncation_is_boundary_safe() {
        let s = "中文字符串".repeat(100);
        let cut = truncate_utf8(&s, 200);
        assert!(cut.is_char_boundary(cut.len()));
        assert!(cut.len() <= 200);
    }
}
