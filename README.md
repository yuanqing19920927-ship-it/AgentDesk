# AgentDesk

macOS 桌面应用，实时监控和管理 AI 编程代理（Claude Code、Codex 等）。

通过菜单栏灵动岛（Dynamic Island）一览所有运行中的 Agent 状态，无需切换窗口。

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
- 一键终止 Agent，一键新建 Agent（支持权限模式选择）

### 会话历史

- 读取 Claude Code 的 JSONL 会话记录
- 按时间倒序展示，支持展开查看完整对话
- 显示 Git 分支、消息数量、会话时长

### 灵动岛 (Dynamic Island)

- 常驻菜单栏，实时显示活跃 Agent 数量与工作状态
- 鼠标悬停自动展开详情面板
- 两阶段平滑收缩动画（先缩高度，再缩宽度）
- 点击 Agent 条目直接跳转到对应终端（支持 iTerm2 / Terminal）
- 任务完成时桌面通知（10 秒防抖，避免子任务间隙误报）

## 安装

### DMG 安装（推荐）

从 [Releases](https://github.com/yuanqing19920927-ship-it/AgentDesk/releases) 下载最新 DMG，打开后将 AgentDesk 拖入 Applications 文件夹。

支持 macOS 12.0+，Universal Binary（Apple Silicon + Intel）。

### 从源码构建

```bash
# 前置条件：Rust 工具链、Xcode Command Line Tools
git clone https://github.com/yuanqing19920927-ship-it/AgentDesk.git
cd AgentDesk
cargo run --release
```

构建过程会自动通过 `build.rs` 编译 Swift 灵动岛组件。

## 项目结构

```
src/
├── main.rs                 # 应用入口
├── models/                 # 数据模型
│   ├── agent.rs            #   Agent、AgentType、AgentStatus
│   ├── project.rs          #   Project
│   └── session.rs          #   SessionRecord、SessionSummary
├── services/               # 后端服务
│   ├── agent_detector.rs   #   进程检测（ps aux + lsof）
│   ├── project_scanner.rs  #   项目自动发现
│   ├── session_reader.rs   #   JSONL 会话解析
│   ├── island.rs           #   灵动岛进程管理
│   ├── notifier.rs         #   系统通知
│   └── config.rs           #   配置持久化
└── ui/
    ├── app_shell/          # UI 组件
    │   ├── sidebar.rs      #   侧边栏（项目列表、分组）
    │   ├── dashboard.rs    #   仪表盘（概览、Agent、会话）
    │   ├── settings.rs     #   设置面板
    │   └── new_agent_dialog.rs  # 新建 Agent 对话框
    └── styles.rs           # 全局样式

helpers/
└── island.swift            # 灵动岛原生 Swift 实现
```

## 技术栈

- **Rust** + **Dioxus** — 桌面 UI 框架
- **Swift** / **AppKit** — 菜单栏灵动岛（NSPanel + CALayer mask 动画）
- **Tokio** — 异步运行时
- **Serde** — JSON 序列化

## 支持的 Agent 类型

| Agent | 进程关键词 | 检测方式 |
|-------|-----------|---------|
| Claude Code | `node` + `claude` | ps aux 进程匹配 |
| Codex | `codex` | ps aux 进程匹配 |

## 配置

配置文件存储在 `~/.agentdesk/` 目录：

- `config.json` — 扫描目录、项目分组
- `project_map.json` — 项目路径映射
- `nicknames.json` — 项目自定义名称
- `island_state.json` — 灵动岛状态（运行时）

## 许可证

[MIT](LICENSE)
