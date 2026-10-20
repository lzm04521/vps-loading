# VPS 库存监控（本地版）

基于 [kyoa.de](https://kyoa.de) 公开数据做的本地库存监控与查看软件。它只做一件事：定时拉取源站发布的 `data.js`，与本地 SQLite 快照逐条对比，记录补货、售罄和价格变化，并按自定义规则发送告警。

数据不依赖浏览器渲染，也不需要逐家爬 VPS 商家；源站更新一次，本地就对比一次。

## 功能

- **Windows 托盘常驻**：双击 `run.bat` 无控制台窗口驻留托盘；菜单可打开看板、立即采集、修改端口、开关开机自启动。
- **库存看板**：416 款套餐、23 家商家，按商家、地区、有货状态、价格、关键词筛选，支持关注列表。
- **长期历史**：源站只保留最近 30 条补货动态，本地会持续积累每一款套餐的补货/售罄/涨价/降价事件。
- **自定义告警**：按商家、地区、名称、线路、价格区间、是否关注设置规则；同一套餐在冷却期内不重复发送。
- **三种通道**：本地看板、Telegram Bot、通用 Webhook。
- **采集可观测**：每次采集的 HTTP 状态、耗时、变化、错误都落库，看板可直接查看。
- **容错**：支持 ETag / Last-Modified（304 时不重复解析）；解析失败、套餐数量异常下降时拒绝更新，不会误判"全部下架"。

## 快速开始

### 方式一：单文件 exe（推荐）

从 GitHub 仓库的 Releases 页下载 `vps-monitor.exe`，放到任意**可写**目录，双击即运行（无需安装 Python）：

- 托盘常驻、左键双击图标打开看板，右键图标可设置端口和开机自启动；
- `config.json` 与 `data/`（数据库、日志）自动生成在 exe 旁边；目录只读时退到 `%LOCALAPPDATA%\vps-monitor`；
- 升级：下载新版 exe 覆盖旧文件即可，配置与数据不受影响；开机自启动指向 exe 路径，路径不变无需重设。

### 方式二：源码运行

要求 Python 3.10+（本机已验证 3.11.9）。

```bash
cd D:\GitHub\vps-loading
pip install -r requirements.txt
python -m vps_monitor run
```

打开 <http://127.0.0.1:8787>。

Windows 也可以直接双击 `run.bat`：程序进入托盘模式（无控制台窗口），左键双击托盘图标打开看板，右键图标可打开看板、立即采集、设置端口和开机自启动、退出。重复启动会弹框提示，不会开第二个采集进程。

首次运行会自动：

1. 从 `config.example.json` 生成 `config.json`；
2. 创建 `data/monitor.db`；
3. 立即采集一次并建立基线（首次不产生补货事件，避免误报）；
4. 启动后台采集线程（默认每 180 秒）和 Web 看板。

## 配置

编辑 `config.json`（已被 `.gitignore` 忽略）：

```json
{
  "source_url": "https://kyoa.de/data.js",
  "poll_interval_seconds": 180,
  "db_path": "data/monitor.db",
  "web": { "host": "127.0.0.1", "port": 8787 },
  "request": {
    "timeout_seconds": 30,
    "user_agent": "vps-loading-local/0.1 (personal inventory monitor)"
  },
  "telegram": { "bot_token": "", "chat_id": "" },
  "webhook": { "url": "" }
}
```

- `poll_interval_seconds` 最小按 60 秒执行。源站实际大约 20 分钟才更新一次，180 秒足够，不建议设得过低。
- `web.port` 也可以在托盘「设置…」窗口里修改：保存后 Web 服务自动用新端口重启并写回本文件，无需重启程序。
- `web.host` 默认只监听 `127.0.0.1`。如需局域网或公网访问，改成 `0.0.0.0`，但请自行加反向代理和认证（本程序没有登录功能）。

### Telegram 告警

1. 在 Telegram 找 `@BotFather`，发送 `/newbot`，拿到 bot token；
2. 给新建的 bot 发一条消息；
3. 浏览器打开 `https://api.telegram.org/bot<你的token>/getUpdates`，取 `message.chat.id`；
4. 把 token 和 chat_id 填进 `config.json` 的 `telegram`；
5. 重启程序，在「规则」页新建规则，通知通道选 Telegram。

### Webhook 告警

在 `config.json` 填 `webhook.url`。命中规则时会 POST 如下 JSON：

```json
{
  "text": "🟢 补货提醒 · ...",
  "event": { "id": 1, "type": "restock", "created_at": "..." },
  "product": {
    "id": 1, "vendor": "56IDC", "name": "V1", "region": "德国",
    "price": "¥10", "price_usd": 1.4, "status": true, "buy_url": "https://..."
  }
}
```

## 命令行

```bash
python -m vps_monitor tray             # 托盘模式（采集 + Web 看板 + 托盘图标）
python -m vps_monitor run              # 采集循环 + Web 看板（控制台）
python -m vps_monitor serve            # 只启动 Web 看板
python -m vps_monitor collect --once   # 采集一次
python -m vps_monitor collect --loop   # 按配置间隔循环采集
python -m vps_monitor collect --force  # 忽略 ETag/内容哈希，强制重新比对
```

通用参数：`--config <路径>`、`--db <路径>`、`--host`、`--port`、`-v`（调试日志）。

`--force` 的用途：本地库被手动改过，或想立即重新核对一次状态时使用。

## 升级与发版

- **用户升级**：到 Releases 页下载新版 `vps-monitor.exe` 覆盖旧文件，重新双击即可。
- **开发发版**：改动代码后提交并推送，打 tag 触发自动构建：`git tag v0.x.0 && git push --tags`，GitHub Actions 会自动打包 exe 并发布到 Releases（见 `.github/workflows/release.yml`）。
- **本地打包**（不经过 CI）：`python scripts/build.py`，产物在 `dist/vps-monitor.exe`。

注意：单文件 exe 未做代码签名，首次运行可能被 Windows SmartScreen 提示，选「仍要运行」即可；个别杀毒软件可能误报，可加白名单。

## 常驻运行

### Windows 托盘 + 开机自启动（推荐）

右键托盘图标 → 勾选「开机自启动」，或打开「设置…」窗口勾选。实现方式是当前用户注册表 Run 键（`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`，值名 `VpsMonitor`），用 `pythonw.exe` 无窗口启动，无需管理员权限；取消勾选即删除该键。

托盘模式的日志写入 `data/vps_monitor.log`（滚动保留 3 份，每份 1MB）。

也可以继续用计划任务或服务方式运行（无托盘）：

### Windows 计划任务

```bat
schtasks /Create /TN "VPS库存监控" /SC ONSTART /RL HIGHEST ^
  /TR "cmd /c cd /d D:\GitHub\vps-loading && set PYTHONUTF8=1 && python -m vps_monitor run"
```

也可以用 [NSSM](https://nssm.cc/) 注册为服务：

```bat
nssm install VpsMonitor "C:\Users\<你>\AppData\Local\Programs\Python\Python311\python.exe" "-m vps_monitor run"
nssm set VpsMonitor AppDirectory "D:\GitHub\vps-loading"
nssm start VpsMonitor
```

### Linux systemd

```ini
[Unit]
Description=VPS inventory monitor
After=network.target

[Service]
WorkingDirectory=/opt/vps-loading
ExecStart=/usr/bin/python3 -m vps_monitor run
Restart=always
User=vps

[Install]
WantedBy=multi-user.target
```

注意：只运行一个进程/worker。多个进程同时采集会重复写事件和发告警。

## 数据与判定规则

- 产品唯一键：`(vendor_key, series_key, product.name)`。源站没有产品 ID，且不同系列存在同名套餐（例如多个 "Pro"），所以不能只用名称。
- `status` 从 `false → true` 记为补货，`true → false` 记为售罄；`price_usd` 变化记为涨价/降价。
- 首次采集只建基线，不产生事件。
- 若解析结果为空，或套餐数较上次下降超过 50%，本次更新会被拒绝并写入错误日志。
- 源站 `history`（最近 30 条）会单独保存并在「补货动态」页展示，与本地的长期事件分开。
- 价格展示保留源站币种，`price_usd` 用于统一比较和规则里的价格区间。

## HTTP API

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/api/summary` | 总数、有货/售罄、关注数、最近采集状态 |
| GET | `/api/products` | 库存列表，支持 `status/vendor/region/q/min_price/max_price/watch/sort/limit/offset` |
| GET | `/api/vendors` | 商家及有货统计 |
| GET | `/api/regions` | 地区及有货统计 |
| GET | `/api/events` | 本地事件，支持 `limit/type` |
| GET | `/api/source-history` | 源站补货动态 |
| GET | `/api/alerts` | 告警记录 |
| GET | `/api/polls` | 采集日志 |
| GET/POST | `/api/rules` | 规则列表 / 新建 |
| PUT/DELETE | `/api/rules/{id}` | 修改 / 删除 |
| POST | `/api/watch/{product_id}` | 切换关注 |
| POST | `/api/collect?force=1` | 立即采集（可强制比对） |
| GET | `/api/config` | 脱敏后的配置状态 |

## 目录结构

```
vps-loading/
├── vps_monitor/
│   ├── __main__.py      # CLI 入口
│   ├── config.py        # 配置加载与端口写回
│   ├── source.py        # data.js 拉取、解析、归一化
│   ├── collector.py     # 采集循环、快照 diff、事件生成
│   ├── db.py            # SQLite schema 与数据访问
│   ├── alerts.py        # 规则匹配与告警通道
│   ├── web.py           # FastAPI 路由与后台线程
│   ├── tray.py          # Windows 托盘模式（图标/菜单/设置窗口/服务托管）
│   └── autostart.py     # 开机自启动（注册表 Run 键）
├── web/                 # 无构建步骤的前端（HTML/CSS/JS）
├── scripts/             # 打包脚本（entry.py / vps-monitor.spec / build.py）
├── .github/workflows/   # 打 tag 自动构建 exe 并发布 Release
├── data/                # 运行数据（已忽略，含 vps_monitor.log）
├── config.example.json
├── config.json          # 首次运行生成（已忽略）
├── requirements.txt
└── run.bat
```

## 数据来源与注意事项

- 数据来自 `https://kyoa.de/data.js`，是对方公开发布在静态页面上的聚合数据；本项目不绕过任何登录或访问控制。
- 请用于个人用途，保持 2–5 分钟级的低频轮询，并保留可识别的 User-Agent。不要并发轰炸、不要转售数据或商家 logo。
- `buy_url` 中是源站维护者的推广参数。保留与否由你决定；保留时对方可能获得推广佣金。
- 告警速度受源站更新频率限制（约 20 分钟），无法做到秒级补货提醒。
- 如果源站改版、加防护或停止维护，采集会报错并在看板显示。解析失败/数量异常时会拒绝更新，不会清空本地数据。
- 本项目没有登录鉴权，默认只监听 127.0.0.1。若暴露到公网，请自行加反向代理与认证。

## 已验证

- 首次采集：416 款、23 家商家，有货 203 / 售罄 213，0 误报事件。
- 重复采集：返回 `304 Not Modified`，不产生重复事件。
- 强制比对：能识别售罄→有货并生成事件，本地告警落库。
- 冷却：同规则同套餐 30 分钟内不重复告警。
- Telegram 未配置：告警记录为 `failed`，错误信息为 `Telegram 未配置 bot_token / chat_id`，不影响采集。
- 前端：库存筛选（有货 203 条）、只看关注（1 条）、补货动态、告警、规则、采集日志均正常渲染。
- 托盘模式（2026-10-20）：双实例互斥、改端口自动重启、开机自启动注册表开关、设置窗口均通过；单文件 exe 独立目录实测：首次运行生成配置（种子端口来自打包内示例）、监听与看板 API 正常、基线采集完成、Web 起不来时进程存活等待改端口。
