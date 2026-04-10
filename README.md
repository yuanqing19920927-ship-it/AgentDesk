# AgentDesk

macOS 桌面应用，实时监控和管理 AI 编程代理（Claude Code、Codex 等），并提供项目记忆、费用追踪、变更审计与健康度仪表盘。

菜单栏灵动岛（Dynamic Island）一览所有运行中的 Agent 状态；工作台按项目隔离展示费用、git 快照、会话日志、记忆条目。

## 功能

### 项目管理

- 自动扫描 `~/.claude/projects/` 发现项目
- 支持手动添加自定义项目路径
- 项目分组、重命名、右键菜单操作
- 项目概览：README 摘要、Agent 数量、会话统计

### Agent 监控

- 实时检测运行中的 Claude Code / Codex 进程
- 显示 PID、CPU 占用、工作目录、状态（工作中/空闲）
- 识别子 Agent（subagent）及其父子关系
- 每 Agent 自定义备注名，一键终止 / 新建（支持权限模式与模板）

### 会话历史与日志实时查看

- 读取 Claude Code 的 JSONL 会话记录，按时间倒序展示
- 展开查看典型对话，含 Git 分支、消息数量
- **日志实时查看（Module 10）**: 消息 / 思考 / 工具调用 / 工具结果四类过滤，一键导出 Markdown 到桌面

### 项目记忆 (Module 2)

- 扫描 Claude Code JSONL 会话生成结构化索引与 `memory.md`
- 自动过滤敏感信息（API keys、tokens 等）
- 存储位置：已授权项目写入 `{project}/.agentdesk/`（附带 `.gitignore`），否则回退 `~/.agentdesk/projects/<hash>/`
- 生成的 `memory.md` 可被 Agent 读取作为历史上下文

### 费用与用量追踪 (Module 6)

- 解析每条 JSONL 的 `usage` 字段，按模型聚合 Opus / Sonnet / Haiku 花销
- 每个项目独立展示累计 USD / input / output / 缓存写入 / 缓存读取 tokens
- 按模型列出调用次数与累计成本
- 价格表内嵌，无需外部 API

### 变更审计 (Module 9)

- 手动一键记录 git 快照（HEAD SHA、branch、porcelain 状态）
- 时间线展示每次快照的修改 / 新增 / 删除 / 未跟踪文件数
- 快照存储于 `~/.agentdesk/audits/<project_hash>/`，可随时删除

### 项目健康度仪表盘 (Module 11)

- 聚合 4 类信号：近 7/30 天 git 提交、近 7 天会话、记忆状态、当前活跃 Agent
- 启发式打分得出绿/黄/红三态，附提示说明原因
- 项目切换时立即重新计算（通过 Dioxus `key` 强制组件重挂载）

### 主目录总览

- 点击侧栏「主目录」查看跨项目的全局仪表盘
- 项目总数、运行中 Agent、会话与助手调用次数
- 累计费用 + 按模型分解
- 项目花销排行 Top 5 + 最近活跃项目列表

### 灵动岛 (Dynamic Island)

- 常驻菜单栏，实时显示活跃 Agent 数量与工作状态
- 鼠标悬停自动展开详情面板
- 两阶段平滑收缩动画（先缩高度，再缩宽度）
- 点击 Agent 条目直接跳转到对应终端（支持 iTerm2 / Terminal）
- 任务完成时桌面通知（10 秒防抖，避免子任务间隙误报）

## 安装

### DMG 安装（推荐）

从 [Releases](https://github.com/yuanqing19920927-ship-it/AgentDesk/releases) 下载最新 DMG，打开后将 AgentDesk 拖入 Applications 文件夹。

支持 macOS 12.0+（Apple Silicon 原生）。首次运行请右键 → 打开（未进行 Apple Notarization）。

### 从源码构建

```bash
# 前置条件：Rust 工具链、Xcode Command Line Tools
git clone https://github.com/yuanqing19920927-ship-it/AgentDesk.git
cd AgentDesk
cargo run --release
```

构建过程会自动通过 `build.rs` 编译 Swift 灵动岛组件。

### 打包 DMG

```bash
./scripts/build-dmg.sh
```

脚本会读取 `Cargo.toml` 中的版本号，产物为 `AgentDesk-<version>.dmg`（包含 ad-hoc 签名的 `.app` + `/Applications` 拖拽链接）。

## 项目结构

```
src/
├── main.rs                      # 应用入口
├── models/                      # 数据模型
│   ├── agent.rs                 #   Agent / AgentType / AgentStatus / PermissionMode
│   ├── audit.rs                 #   AuditSnapshot / AuditDiff
│   ├── cost.rs                  #   ModelPricing / SessionCost / ProjectCost
│   ├── health.rs                #   HealthStatus / ProjectHealth
│   ├── memory.rs                #   MemoryEntry / MemoryIndex / Cursor
│   ├── notification.rs          #   NotificationRules / QuietHours
│   ├── project.rs               #   Project
│   ├── session.rs               #   SessionRecord / SessionSummary
│   └── template.rs              #   AgentTemplate
├── services/                    # 后端服务
│   ├── agent_detector.rs        #   进程检测（ps aux + lsof）
│   ├── agent_names.rs           #   Agent 备注名持久化
│   ├── approved_projects.rs     #   项目写入白名单
│   ├── audit_recorder.rs        #   git 快照记录与时间线
│   ├── claudemd_writer.rs       #   CLAUDE.md 项目记忆段落管理
│   ├── config.rs                #   全局配置
│   ├── cost_tracker.rs          #   JSONL usage → USD 聚合
│   ├── health_monitor.rs        #   健康度评分
│   ├── island.rs                #   灵动岛进程管理
│   ├── log_streamer.rs          #   日志流式读取（4 类 StreamItem）
│   ├── memory_indexer.rs        #   项目记忆索引 / 存储模式
│   ├── notifier.rs              #   系统通知 + 规则过滤
│   ├── project_manager.rs       #   自定义项目与昵称
│   ├── project_scanner.rs       #   项目自动发现
│   ├── session_reader.rs        #   JSONL 会话元数据
│   ├── template_manager.rs      #   Agent 模板
│   └── terminal_launcher.rs     #   iTerm2 / Terminal 启动
└── ui/
    ├── app_shell/               # 主界面
    │   ├── dashboard.rs         #   项目仪表盘（健康度 + 记忆 + 费用 + 审计 + 日志）
    │   ├── home_dashboard.rs    #   主目录全局总览
    │   ├── memory_view.rs       #   项目记忆面板
    │   ├── new_agent_dialog.rs  #   新建 Agent 对话框
    │   ├── settings.rs          #   设置面板
    │   ├── sidebar.rs           #   侧边栏（项目列表、分组）
    │   └── templates.rs         #   Agent 模板管理
    ├── icons.rs                 #   内联 SVG 图标库
    └── styles.rs                #   全局样式

helpers/
├── island.swift                 # 灵动岛原生 Swift 实现
└── island-overlay-universal     # 预编译 universal binary（DMG 打包用）

scripts/
└── build-dmg.sh                 # 一键打包 release .app + DMG
```

## 技术栈

- **Rust** + **Dioxus 0.6** — 桌面 UI 框架（wry / WKWebView）
- **Swift** / **AppKit** — 菜单栏灵动岛（NSPanel + CALayer mask 动画）
- **Tokio** — 异步运行时
- **Serde** — JSON 序列化
- **Chrono** — 时间处理

## 支持的 Agent 类型

| Agent | 进程关键词 | 检测方式 |
|-------|-----------|---------|
| Claude Code | `node` + `claude` | ps aux 进程匹配 |
| Codex | `codex` | ps aux 进程匹配 |

## 配置与数据

用户级数据存储在 `~/.agentdesk/`：

| 文件 / 目录 | 用途 |
|---|---|
| `config.json` | 扫描目录、项目分组 |
| `project_map.json` | Claude 项目目录 → 真实路径映射 |
| `project_nicknames.json` | 项目自定义显示名 |
| `custom_projects.json` | 手动添加的项目 |
| `approved_projects.json` | 允许写入项目内 `.agentdesk/` 的白名单 |
| `agent_names.json` | 每 Agent 的备注名 |
| `notifications.json` | 通知规则与静音时段 |
| `templates/` | 自定义 Agent 启动模板 |
| `projects/<hash>/` | 回退模式下的项目记忆索引 |
| `audits/<hash>/` | git 变更快照时间线 |
| `island_state.json` | 灵动岛状态（运行时） |

项目级数据（仅对已授权项目）写入 `{project}/.agentdesk/`，自动追加到 `.gitignore`。

## 许可证

[MIT](LICENSE)
