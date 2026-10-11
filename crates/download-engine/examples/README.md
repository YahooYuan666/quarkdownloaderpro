# 独立验证工具

这一对工具用于**端到端验证下载引擎**，与 `tests/reliability.rs` 里那个进程内 mock 是
**两个独立实现**——真实 socket、真实 HTTP 解析、真实连接中断，用来交叉验证单元测试
没有自说自话。

## 用法

```bash
# 1) 起一个会故意截断分片的 mock CDN（truncate-once / truncate-always）
python examples/mock_cdn.py truncate-once

# 2) 另开一个终端，按它打印的端口跑下载
cargo run --release --example e2e_download -- \
  "http://127.0.0.1:<port>/big.bin" /tmp/out.bin 4

# 3) 比对 SHA256（服务端启动时会打印期望值）
sha256sum /tmp/out.bin
```

## 两种剧本

| 模式 | 服务端行为 | 引擎应当 |
| --- | --- | --- |
| `truncate-once` | 首个 Range 请求声明完整长度、只发 4KB 后硬断连接 | 重试并从断点续传，最终文件**逐字节一致** |
| `truncate-always` | 每个 Range 请求都截断 | 3 次重试后**显式失败**，不留最终文件、不留临时目录 |

`mock_stats.json` 会实时记录 `range_requests` / `truncated` / `full` 计数——用它证明
"截断真的发生过"，否则测试是空转（例如 `truncate-once` 下 4 分片应出现 5 次请求）。
