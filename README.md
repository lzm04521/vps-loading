# VPS 库存监控（VPS Monitor）

基于 [kyoa.de](https://kyoa.de) 公开数据做的本地 VPS 库存监控桌面应用：定时拉取源站发布的 `data.js`，与本地 SQLite 快照逐条对比，记录补货、售罄和价格变化，并按自定义规则发送告警。

技术形态为 **Tauri 2 混合桌面应用**：Rust 主进程承载全部业务（采集 / 存储 / 告警），React WebView 做展示层，NSIS 安装包分发。

## 功能

- **桌面应用 + 托盘常驻**：关窗即隐藏到托盘，托盘菜单可打开主窗口、立即采集、退出；支持开机自启动（静默进托盘）；单实例，重复启动唤起已有窗口。
- **库存看板**：400+ 款套餐、20+ 家商家，按商家、地区、有货状态、价格、关键词、关注筛选，六种排序，分页浏览。
- **长期历史**：源站只保留最近 30 条补货动态，本地持续积累每一款套餐的补货 / 售罄 / 涨价 / 降价 / 新品事件。
- **自定义告警**：按商家、地区、名称、线路、价格区间、是否关注设置规则；同一套餐在冷却期内不重复发送。
- **三种通道**：本地看板、Telegram Bot（token 存 Windows 凭据管理器，不落配置文件）、通用 Webhook。
- **采集可观测**：每次采集的 HTTP 状态、耗时、变化、错误都落库，应用内可查；采集状态实时推送到总览页。
- **容错**：支持 ETag / Last-Modified 条件请求（304 不重复解析）与内容哈希短路；解析失败、套餐数量异常下降时拒绝更新，不会误判"全部下架"。
- **中英双语界面**。

## 安装与升级

从 GitHub 仓库 Releases 页下载 `VPS Monitor_x.y.z_x64-setup.exe`，双击安装（per-user 安装，无需管理员）。

- 数据与配置保存在 `%APPDATA%\com.vpsmonitor.app`（数据库）与 `%LOCALAPPDATA%\com.vpsmonitor.app\logs`（日志），覆盖安装、卸载都不受影响。
- 升级：应用内自动更新——启动时自动检测新版本并在"关于"页手动检查，下载经签名校验的安装包自动完成升级；也可手动下载新安装包覆盖安装。
- **从旧版（v0.2 及更早的绿色 exe）迁移**：打开新应用 → 设置 → 导入旧版数据 → 选择旧版安装目录（含 `config.json` 与 `data\monitor.db`），导入告警配置（含 Telegram token）并沿用历史数据；旧版程序不受影响。

## 开发

要求：Node 22+ / pnpm 11 / Rust stable（含 MSVC 工具链）。

```bash
pnpm install          # 前端依赖
pnpm tauri dev        # 开发模式（热更新）
pnpm test             # 前端 Vitest
pnpm build            # 前端 tsc + vite 构建
cd src-tauri
cargo test            # Rust 单元测试
cargo clippy --all-targets -- -D warnings
```

打包安装包：`pnpm tauri build`（产物在 `src-tauri/target/release/bundle/nsis/`）。本地打包需设置 `TAURI_SIGNING_PRIVATE_KEY` 环境变量（更新签名私钥，见下文"发版"），否则 bundle 阶段会因无法签名失败。

图标源图：`python scripts/gen_icon.py` 生成 `app-icon.png`，再 `pnpm tauri icon app-icon.png` 出全套。

数据库巡检：`python scripts/check_db.py`（读取 `%APPDATA%` 下的库，输出产品/事件/采集摘要）。

## 架构

```
├── src/                     # React 19 + antd 5 前端（纯展示层）
│   ├── api/ipc.ts           # 唯一后端通道：invoke 命令 + listen("monitor://status") + IpcError
│   ├── pages/               # 8 页：总览/库存/补货动态/告警/规则/采集日志/设置/关于
│   ├── components/          # ProductCard、EventTag
│   ├── hooks/useUpdater.ts  # 应用内更新状态机（检查/下载/安装/重启）
│   ├── i18n/                # zh-CN / en 资源 + IPC 错误码文案映射
│   └── types.ts             # 与 Rust DTO 镜像的唯一类型源
└── src-tauri/src/           # Rust 主进程（业务核心）
    ├── lib.rs               # 装配点：插件/托盘/命令注册/调度器启动
    ├── commands.rs          # IPC 命令层（DTO 校验 + 错误码化）
    ├── model.rs             # 领域模型 + DTO + 事件/通道枚举单源
    ├── config.rs            # config.json 原子读写存储
    ├── secrets.rs           # keyring（Windows 凭据管理器）存取 token
    ├── logging.rs           # tracing 滚动日志 + token 打码 + panic hook
    ├── store/               # SQLite（rusqlite，沿用旧版 10 表 schema，按表拆文件）
    ├── provider/            # 数据源抽象（SourceProvider trait + kyoa.de 实现）
    └── monitor/             # 核心域：normalize / diff / engine（单次采集循环）/
                             #   service（常驻调度）/ alerts（规则与通道）
```

分层纪律与 gist_auto_sync 同构：引擎不感知 Tauri（经 `StatusSink` trait 反转依赖）、外部服务经 `SourceProvider` 端口替换、错误三层契约（领域错误 → IPC 错误码 → 前端 i18n）。

## 发版

```bash
# 版本号两处同步：src-tauri/Cargo.toml 与 src-tauri/tauri.conf.json
git tag vX.Y.Z
git push --tags   # 触发 .github/workflows/ci.yml 的 release job，出 NSIS 安装包并挂 Release
```

应用内自动更新已启用：CI 发布时用 minisign 私钥签名安装包并在 Release 附 `latest.json`，应用从公开仓库 Release 拉取更新、以内置公钥（`tauri.conf.json` 的 `plugins.updater.pubkey`）校验后安装。

- 签名私钥存于本机 `C:\Users\lzm04\.tauri\vps-loading_updater.key`（空密码），并已配置为仓库 Actions secret `TAURI_SIGNING_PRIVATE_KEY`；**私钥丢失将无法再发自动更新**，请妥善备份。
- 注意：v0.3.1 及更早版本未内置更新器，需手动覆盖安装一次新版本后才能收到后续自动更新。

## 数据说明

- 数据源：kyoa.de 公开发布的 `data.js`，采集默认间隔 180 秒（可调，最小 60）。
- 本地 SQLite 十张表：主数据（vendors/series/products）、事件（events）、采集日志（polls）、源站动态（source_history）、规则（rules）、关注（watchlist）、告警记录（alerts）、游标（meta）。
