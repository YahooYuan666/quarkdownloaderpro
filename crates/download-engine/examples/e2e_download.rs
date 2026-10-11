//! 独立端到端验证工具：从真实 URL 下载并输出结果。
//! 用法：e2e_download <url> <out_path> <segments>

use std::path::PathBuf;

use download_engine::{DownloadOptions, ProgressEvent};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("用法: e2e_download <url> <out_path> <segments>");
        std::process::exit(2);
    }
    let url = args[1].clone();
    let out = PathBuf::from(&args[2]);
    let segments: usize = args[3].parse().unwrap_or(4);

    let options = DownloadOptions {
        url,
        headers: vec![("User-Agent".into(), "e2e-verifier".into())],
        save_path: out.clone(),
        segment_count: segments,
    };

    let cancel = CancellationToken::new();
    let mut events = 0usize;
    let result = download_engine::download(options, cancel, |ev| {
        events += 1;
        if let ProgressEvent::Merging = ev {
            eprintln!("[e2e] 合并中");
        }
    })
    .await;

    match result {
        Ok(size) => {
            println!("OK size={size} events={events} path={}", out.display());
        }
        Err(e) => {
            println!("FAIL error={e}");
            std::process::exit(1);
        }
    }
}
