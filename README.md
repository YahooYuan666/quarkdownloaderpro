<div align="center">

# Quark Downloader Pro

**夸克网盘分享链接批量下载工具（非官方）**

[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-blue?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React 19](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.7-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Vite](https://img.shields.io/badge/Vite-6-646CFF?logo=vite&logoColor=white)](https://vite.dev/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](./LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)]()

一个基于 **Tauri 2 + React 19 + Rust** 构建的跨平台桌面应用，用于解析夸克网盘分享链接并批量下载文件。
无内嵌服务、无本地监听端口，全部逻辑在单一 Rust 进程内完成。

安装包请在 [Releases](../../releases) 页面下载。

</div>

---

## ✨ 功能特性

- **内嵌网页登录**（推荐）— 应用内打开夸克登录页，正常登录后自动捕获会话；也支持手动粘贴 Cookie 作为兜底
- **会话长期记住** — 登录凭据存入操作系统凭据库（Windows 凭据管理器 / macOS 钥匙串 / Linux Secret Service），重启免登录
- **二维码图片识别** — 直接把分享二维码截图拖入、`Ctrl+V` 粘贴或点击选择图片，自动识别出链接并解析
- **智能链接解析** — 粘贴分享链接自动提取，支持带提取码 / 子目录的链接格式
- **递归目录扫描** — 自动遍历分享中的所有子目录，完整还原文件树结构，支持展开/折叠与勾选下载
- **Rust 分片下载引擎** — 默认 12 分片、上限 64，≥10MiB 文件自动启用 HTTP Range 分片，小文件回退单流；逐片完整性校验 + 指数退避重试 + 断点续传
- **批量并发处理** — 1–10 文件并发度可调，自动完成「转存 → 获取直链 → 下载到本地 → 清理转存文件」全流程；任务状态实时推送（转存中/下载中/合并中）
- **下载任务中心** — 与解析页并列的独立任务页：实时进度/速度/剩余时间、单任务与全部取消、失败重试、同名文件策略（询问/自动改名/跳过）
- **进度小窗** — 单任务下载时弹出独立迷你窗口，关闭不影响后台下载
- **崩溃自愈** — 转存账本记录下载中间态，启动时自动清理异常残留的转存文件
- **深浅双主题** — Nord 家族语义化配色，跟随系统/手动切换
- **跨平台** — Windows x64、macOS (Apple Silicon/Intel)、Linux x64

---

## 🏗 架构说明

```
┌────────────────────────────────────────────────────┐
│                Tauri 2（单一 Rust 进程）             │
│                                                    │
│  ┌──────────────┐      ┌────────────────────────┐  │
│  │  WebView     │ IPC  │  Rust Cargo 工作区      │  │
│  │  React 19    │◄────►│  ├─ download-engine    │  │
│  │  frontend/   │      │  ├─ quark-core         │  │
│  └──────────────┘      │  └─ src-tauri 外壳      │  │
│                        └───────────┬────────────┘  │
└────────────────────────────────────┼───────────────┘
                                     │ HTTPS
                              夸克网盘 API / CDN
                    (pan.quark.cn / drive.quark.cn)
```

| 层级 | 技术 | 说明 |
|------|------|------|
| **download-engine** | Rust · tokio · reqwest | 通用分片下载引擎：分片调度、重试退避、断点续传、合并校验、取消。**不认识「夸克」**，只接收 URL + 请求头 + 保存路径 |
| **quark-core** | Rust · reqwest | 夸克协议层：API 路由、CookieJar（同名覆盖 + 失效判定）、扫码/会话校验。**不认识「下载」** |
| **src-tauri 外壳** | Rust · Tauri 2 | 窗口管理、任务中心（转存编排）、系统凭据库、设置、进度小窗、日志 |
| **Frontend** | React 19 + TypeScript + Vite 6 + TailwindCSS + Zustand | 单页应用，全部通过 Tauri IPC（invoke/listen）与 Rust 通信 |

**数据流**：前端 → Tauri IPC → 任务中心（转存 → 轮询 → 取直链 → 删转存）→ download-engine 多线程分片下载 → CDN

---

## 🚀 快速开始

### 环境要求

| 工具 | 版本要求 |
|------|----------|
| [Node.js](https://nodejs.org/) | >= 18.x |
| [Rust](https://www.rust-lang.org/tools/install) | >= 1.77 |
| WebView 运行时 | Windows: WebView2（Win11 自带）/ macOS: WKWebView / Linux: WebKitGTK |

> **Windows 用户**：需安装 [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
> **Linux 用户**：需安装 `libwebkit2gtk-4.1-dev` 等依赖，见 [Tauri 文档](https://v2.tauri.app/start/prerequisites/)

### 安装依赖

```bash
git clone https://github.com/<YOUR_USERNAME>/quarkdownloaderpro.git
cd quarkdownloaderpro

npm install                # 根目录工具依赖
cd frontend && npm install # 前端依赖
```

### 开发模式

```bash
npm run tauri:dev
```

### 构建生产版本

```bash
npm run tauri:build
```

支持交叉编译目标：`npm run tauri:build:win-x64` / `win-arm64` / `mac-x64` / `mac-arm64` / `mac-universal`（需先 `rustup target add <target>`）。

**产物路径**（Cargo 工作区统一在根 `target/`）：

| 平台 | 产物 |
|------|------|
| Windows | `target/release/bundle/nsis/*.exe`（安装包） |
| macOS | `target/release/bundle/macos/`、`bundle/dmg/` |
| Linux | `target/release/bundle/deb/`、`bundle/rpm/`、`bundle/appimage/` |

---

## 📖 使用指南

### 1. 登录账号

- **网页登录（推荐）**：点击「立即登录」，应用内弹出夸克登录页，正常登录后窗口自动关闭、会话自动保存
- **手动 Cookie**：打开 [pan.quark.cn](https://pan.quark.cn) 登录后，从浏览器开发者工具复制 Cookie 粘贴（保存前自动校验有效性）

### 2. 解析分享链接

在输入框粘贴夸克分享链接，或直接把分享二维码截图**拖入 / 粘贴 / 点击选择**，点击「解析提取」。

支持的链接格式：

```
https://pan.quark.cn/s/xxxxxxxxx
https://pan.quark.cn/s/xxxxxxxxx?pwd=xxxx
https://pan.quark.cn/s/xxxxxxxxx#/list/share/xxxx
```

### 3. 选择并下载

- 勾选文件（支持「全选」），点击「开始任务」后自动跳转「下载任务」页
- 全流程自动：转存网盘 → 获取直链 → 多线程分片下载 → 清理转存文件（不占用网盘空间）
- 支持单任务/全部取消、失败重试；同名文件可选「每次询问 / 自动改名 / 跳过」

### 4. 下载目录与设置

在「软件设置」页可修改默认下载目录、记住上次目录、同名文件策略、外观主题、完成时是否弹出进度小窗。

---

## 📁 项目结构

```
quarkdownloaderpro/
├── crates/
│   ├── download-engine/        # 通用分片下载引擎（不认识夸克，ADR-0001）
│   └── quark-core/             # 夸克协议层（protocol / cookie / login）
├── src-tauri/                  # Tauri 桌面外壳
│   ├── src/
│   │   ├── main.rs             # 入口：窗口、日志、关闭拦截、会话恢复
│   │   ├── api.rs              # 夸克协议 Tauri 命令
│   │   ├── tasks.rs            # 任务中心：转存编排、进度事件、取消/重试
│   │   ├── cookie_store.rs     # 系统凭据库托管会话（keyring）
│   │   ├── settings.rs         # 设置、文件夹选择、打开文件/文件夹
│   │   ├── journal.rs          # 转存账本（启动清理残留转存）
│   │   ├── progress_window.rs  # 单任务进度小窗
│   │   └── login_window.rs     # 内嵌网页登录窗口
│   ├── capabilities/           # Tauri 权限配置
│   ├── icons/                  # 应用图标
│   └── tauri.conf.json
├── frontend/                   # React 19 SPA
│   └── src/
│       ├── components/ui/      # 基础组件（Button/Card/Input...）
│       ├── features/           # auth / share-parse / file-tree / downloads / settings / help / progress-window
│       ├── store/              # Zustand 全局状态
│       ├── services/           # Tauri IPC 封装
│       └── dev/tauri-mock.ts   # 浏览器预览用 IPC 模拟（仅 DEV）
├── docs/adr/                   # 架构决策记录
├── package.json                # 根工作区脚本
└── README.md
```

---

## 🛠 技术栈

### Rust（Cargo 工作区）

| 技术 | 用途 |
|------|------|
| [Tauri](https://v2.tauri.app/) 2 | 桌面应用框架、多窗口、IPC |
| [tokio](https://tokio.rs/) | 异步运行时 |
| [reqwest](https://github.com/seanmonstar/reqwest) | HTTPS 客户端（协议请求与分片下载） |
| [keyring](https://github.com/hwchen/keyring-rs) 3 | 系统凭据库（会话持久化） |
| serde / serde_json | 序列化 |
| [tracing](https://github.com/tokio-rs/tracing) | 结构化日志（日志中不落任何凭据） |

### 前端

| 技术 | 版本 | 用途 |
|------|------|------|
| [React](https://react.dev/) | 19 | UI 框架 |
| [TypeScript](https://www.typescriptlang.org/) | 5.7 | 类型安全 |
| [Vite](https://vite.dev/) | 6 | 构建工具 |
| [TailwindCSS](https://tailwindcss.com/) | 3.4 | 语义化主题令牌 |
| [Zustand](https://zustand.docs.pmnd.rs/) | 5 | 状态管理 |
| [Radix UI](https://www.radix-ui.com/) | latest | 无障碍基础组件 |
| [Framer Motion](https://motion.dev/) | 12 | 动画 |
| [jsQR](https://github.com/cozmo/jsQR) | 1.4 | 二维码图片识别（纯前端解码） |

---

## 🤝 贡献指南

欢迎贡献代码！

1. **Fork** 本仓库
2. 创建功能分支：`git checkout -b feature/your-feature`
3. 提交更改：`git commit -m 'feat: add your-feature'`（遵循 [Conventional Commits](https://www.conventionalcommits.org/)）
4. 提交 **Pull Request**

开发检查：`npm run lint`（前端）、`cargo check --workspace` / `cargo test --workspace`（Rust）。

---

## 🙏 致谢

本项目的「转存 → 取直链 → 删转存 → 下载」核心思路与最初实现，完全来自原作者的开创性工作。上游停更后，本仓库在原思路基础上按自维护标准重构延续。**向原作者致敬。**

- **[OP404OP/quarkdownloaderpro](https://github.com/OP404OP/quarkdownloaderpro)**（作者 IsNull）🫡 —— 本项目的前身；原作者的 MIT 版权声明已在 [LICENSE](./LICENSE) 中完整保留
- [muyan556/gopeed-extension-quark](https://github.com/muyan556/gopeed-extension-quark) — 上游核心解析逻辑的参考来源
- [zerx-lab/FluxDown](https://github.com/zerx-lab/FluxDown) — 进度小窗与主题引擎的设计灵感
- [lerdb/HotDownloader](https://github.com/lerdb/HotDownloader)、[agalwood/Motrix](https://github.com/agalwood/Motrix) — 下载目录、同名策略与界面交互参考

---

## ⚠️ 免责声明

使用前请务必阅读并理解以下内容：

1. **非官方声明**：本项目与夸克网盘 / 夸克 App / 阿里巴巴及其关联公司**无任何关联、未获任何授权**；名称中的「Quark」仅为描述工具的作用对象，不构成商标意义上的使用。
2. **用途限制**：本项目仅供**个人学习与技术交流**使用，禁止用于任何商业用途或违法用途。
3. **账号风险自担**：使用本工具需登录**您自己的**夸克账号。使用非官方第三方工具可能违反夸克用户协议，由此可能导致的账号风控、功能限制等一切后果，由使用者自行承担。
4. **无任何破解**：本项目**不提供、不接受**任何破解限速、绕过会员、绕过容量限制的功能或请求；下载速度与可用性完全由夸克官方服务决定，夸克接口变更可能导致功能失效（属预期内，不承诺维护）。
5. **内容责任**：使用者对通过本工具下载的**内容**负全部责任。请仅下载您有权获取的文件；严禁下载、传播侵犯他人知识产权或违反法律法规的内容。
6. **按现状提供**：本软件不提供任何明示或默示的担保（详见 [MIT 许可证](./LICENSE)），开发者不对使用本软件造成的任何直接或间接损失负责。
7. **侵权处理**：如本项目或其内容侵犯您的合法权益，请通过 Issue 联系，确认后将及时处理。

---

## 📄 License

[MIT](./LICENSE) © 2026 IsNull（原作者）与本仓库贡献者
