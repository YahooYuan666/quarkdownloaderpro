# ADR-0001: 保持 Tauri 2 + React，自研 Rust 下载引擎，不引入通用下载器内核

- 状态：Accepted
- 日期：2026-10-03
- 决策人：用户（对话中明确「基本同意」ZCode 的选型分析）
- 依据：`docs/plans/research/ANALYSIS-重构技术栈与UI重点.md`（三项目实测对比）

## 背景

重构版需要确定界面框架与下载内核来源。候选：现有 Tauri 2 + React 演进、纯 Rust GPUI（FluxDown 路线）、Electron + aria2（Motrix 路线）、Flutter（gopeed 路线）、引入现成 Rust 下载库（trauma / hotdownloader-core）。

## 决策

1. **界面**：保持 Tauri 2 + React 19。零迁移成本，Web UI 生态（shadcn/ui 已在用）全保留。
2. **内核**：保留并修复自研 Rust 引擎（约 400 行，已完成分片/临时文件/合并/进度/取消的 80%），不引入通用下载器。
3. **架构边界**：抽 `quark-core`（夸克协议）与 `download-engine`（通用下载，不认识「夸克」）两个 crate；砍掉内嵌 axum HTTP 层，统一 Tauri command。

## 理由

- 夸克下载器只需要：多连接 Range、重试、续传、限速、队列、进度、取消。通用下载器 70% 的功能（BT/磁力/ed2k/FTP/HLS/插件沙箱）用不上。
- FluxDown（AGPL-3.0）、gopeed（GPL-3.0）有传染性许可证约束，代码不可抄。
- 本项目瓶颈是功能正确性与交互（P0 全在 Rust 侧逻辑），不是语言性能；换框架不解决任何 P0。
- 「转存 → 取直链 → 删转存 → 下载」是本项目核心竞争力，自研内核是其载体。

## 后果

- 正面：许可证完全自主；改动量可控（引擎修复约百行级）；移动端预留通过 core/shell 分层实现，不封死路线。
- 负面：重试/续传/限速需自己实现（通用下载器现成）；团队放弃 GPUI/Flutter 的体验上限。
- 否决的备选：GPUI（生态不成熟、学习曲线陡）、Electron+aria2（150MB+ 体积、GPL-2.0+ 分发问题）、Flutter（内核是 Go + GPL）、trauma/hotdownloader-core（小社区依赖风险，可日后重新评估）。
