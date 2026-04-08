# AgentDesk - macOS AI Agent 管理应用设计文档

## 概述

AgentDesk 是一款 macOS 桌面应用，用于管理本机上运行的 AI Agent（Claude Code、Codex 等），提供项目长记忆管理、Agent 生命周期管理、终端集成等功能。

## 技术栈

- **语言**: Rust
- **GUI 框架**: Dioxus (桌面端)
- **异步运行时**: Tokio
- **不依赖 Xcode**

## 核心需求

### 1. 项目长记忆系统

**数据流**:
```
~/.claude/projects/{project}/*.jsonl  (Claude Code 会话数据源)
         │
         ▼
   session_reader  (解析 JSONL, 提取 user/assistant 消息对)
         │
         ▼
   memory_indexer  (按主题分类, 生成摘要, 提取关键决策/代码变更)
         │
         ▼
   {project_dir}/.agentdesk/
   ├── sessions/          # 会话摘要 (按日期归档)
   │   └── 2026-04-08.md
   ├── index.json         # 记忆索引 (关键词 → 文件位置映射)
   └── memory.md          # 结构化长记忆 (Agent 可直接读取)
         │
         ▼
   claudemd_writer  (在项目 CLAUDE.md 中插入记忆读取指引段落)
```

**索引策略**:
- 应用启动时全量扫描一次
- 增量扫描基于**游标机制**：为每个 JSONL 文件记录已处理的字节偏移量、inode 号、文件大小（存储在 `index.json` 的 `cursors` 字段）
- 每次扫描先校验 inode 和文件大小：若 inode 变化或文件大小小于记录值，判定文件被替换/截断，触发该文件全量重扫
- 校验通过后从上次偏移量继续读取，避免重复处理
- 使用 `notify` crate 监听文件变化触发增量扫描
- 每条记忆条目使用会话消息的 `uuid` 作为去重键，保证幂等性
- **单写者模型**：每个项目的索引更新由 Tokio mutex 保护的 actor 串行执行，`notify` 事件通过 debounce（500ms）合并后投递给 actor，避免并发写冲突
- **跨进程保护**：每个项目的所有写入操作（`index.json`、`sessions/*.md`、`memory.md`、`CLAUDE.md`）作为一个事务，写入前获取项目级 OS 文件锁（`flock` on `.agentdesk/.lock`），确保所有文件的一致性。锁获取失败时：标记项目为 dirty，启动独立的定时重试（指数退避，最大间隔 60s），重试成功后清除 dirty 标记。dirty 状态在 UI 中可见
- **崩溃安全写入顺序**：先写派生文件（`sessions/*.md`、`memory.md`），再写 `index.json`（含游标状态）。游标只在所有派生文件落盘后才推进。崩溃后重启时，未推进的游标会导致重新处理，但因 uuid 去重不会产生重复条目
- 所有文件写入采用写临时文件 + 原子重命名（`rename`），防止 torn write
- actor 在写入前重新读取磁盘上最新的 index.json，合并增量变更后再写出

**记忆存储格式** (`index.json`):
```json
{
  "version": 1,
  "last_scan": "2026-04-08T12:00:00Z",
  "cursors": {
    "b174651a-8138-4b60-8cfd-3e08c6e15bb9.jsonl": {
      "byte_offset": 245678,
      "inode": 12345678,
      "file_size": 300000,
      "last_uuid": "msg_abc123"
    }
  },
  "entries": [
    {
      "id": "mem_001",
      "uuid": "msg_abc123",
      "timestamp": "2026-04-08T12:00:00Z",
      "session_id": "b174651a-...",
      "topics": ["auth", "database"],
      "summary": "实现了用户认证模块，使用 JWT...",
      "keywords": ["jwt", "auth", "middleware"],
      "file_ref": "sessions/2026-04-08.md#auth-module"
    }
  ]
}
```

**CLAUDE.md 集成**:
仅在项目本地存储模式（`.agentdesk/` 位于项目目录内）时写入 CLAUDE.md。回退到用户级存储时不修改 CLAUDE.md（因为 Agent 无法可靠访问 `~/.agentdesk/` 下的路径）。

项目本地模式插入内容：
```markdown
## Project Memory (AgentDesk)
- 项目记忆索引: .agentdesk/index.json
- 结构化记忆: .agentdesk/memory.md
- 会话摘要目录: .agentdesk/sessions/
- 使用方式: 需要历史上下文时，先读取 .agentdesk/memory.md 获取概览，再通过 index.json 定位详细记录
```

用户级存储回退模式：不修改 CLAUDE.md，记忆仅通过 AgentDesk UI 浏览。后续可考虑在 CLAUDE.md 中写入绝对路径指向用户级存储。

### 2. 项目管理面板

**项目发现**:
- 扫描 `~/.claude/projects/` 目录下的 JSONL 会话文件
- **从会话 JSONL 的 `cwd` 字段提取真实项目路径**（不依赖目录名反推，避免连字符歧义）
- **路径规范化**：对提取的 `cwd` 执行 `git rev-parse --show-toplevel` 获取仓库根目录作为项目标识（非 git 项目则使用 `cwd` 本身）。解析 symlink 为真实路径，确保同一仓库的子目录会话归属同一项目
- **Worktree 处理**：git worktree 的会话归属主仓库，但记忆条目中标注 `branch` 字段（从 JSONL 的 `gitBranch` 提取），Agent 读取时可按分支过滤相关记忆
- 建立并持久化 `claude_dir_name → canonical_project_root` **绑定**映射表（存储在 `~/.agentdesk/project_map.json`）。检测到同一 claude_dir_name 的 cwd 发生变化时：**阻止该来源的索引写入**，在 UI 中高亮提示路径漂移，用户确认后重新绑定或忽略
- **项目写入授权**：首次发现新项目路径时，仅在 UI 中展示（只读）。用户需在项目面板中手动"启用记忆"才将该路径加入允许写入的白名单（`~/.agentdesk/approved_projects.json`）。未经批准的项目不进行任何文件系统写入
- 遇到无法确定路径的目录时跳过并记录警告，不猜测
- 验证项目路径是否存在
- 读取项目基本信息（git info、语言、框架等）

**展示内容**:
- 项目名称 + 路径
- 运行中 Agent 数量（实时检测）
- 最近活跃时间
- 记忆条目数量

### 3. Agent 检测与管理

**进程检测**:
```
ps aux → 过滤 claude/codex 进程
       → 解析命令行参数获取工作目录
       → 匹配到项目
```

具体匹配规则:
- Claude Code: 进程名含 `node` + 参数含 `claude`，通过 `lsof -p <pid>` 获取 cwd
- Codex: 进程名含 `codex`，关联 Codex.app

**Agent 管理功能**:
- 查看运行中 Agent 列表
- 编辑 Agent 备注名（持久化到 `.agentdesk/agents.json`）
- 点击 Agent 卡片 → 拉起对应终端窗口（通过 AppleScript 激活窗口）

### 4. 新建 Agent

**流程**:
1. 用户选择项目
2. 选择 Agent 类型（Claude Code / Codex）
3. 可选配置（权限模式、模型等）
4. 确认 → 通过 osascript 在 iTerm2/Terminal.app 中打开新窗口
5. 自动执行 `cd <project_dir> && claude` 或 `codex`

**终端启动命令**:

安全要求：**禁止**将项目路径直接拼接进 shell 字符串。使用 AppleScript 的 `quoted form of` 对路径进行转义，或通过 argv 传递参数。

```applescript
-- iTerm2 (安全版本: 使用 quoted form 防止路径注入)
tell application "iTerm2"
    set newWindow to (create window with default profile)
    tell current session of newWindow
        write text "cd " & quoted form of "/path/to/project" & " && claude"
    end tell
end tell

-- Terminal.app (安全版本)
tell application "Terminal"
    do script "cd " & quoted form of "/path/to/project" & " && claude"
end tell
```

Rust 侧使用 `std::process::Command` 调用 `osascript`，路径作为变量注入 AppleScript 模板，而非拼接 shell 字符串。对路径进行校验：必须是已存在的绝对路径目录。

**参数安全模型**：Agent 类型和启动参数使用 Rust 枚举严格约束，不接受自由文本：
```rust
enum AgentType { ClaudeCode, Codex }
enum PermissionMode { Default, DangerouslySkipPermissions, Plan }
// 每个参数单独 quoted form 转义，不拼接为 shell 字符串
// 不支持的参数值直接拒绝，不传递给 shell
```

## 架构设计

```
agentdesk/
├── src/
│   ├── main.rs                  # 入口，初始化 Dioxus + Tokio
│   ├── app.rs                   # Dioxus 根组件，路由
│   ├── models/
│   │   ├── mod.rs
│   │   ├── project.rs           # Project 结构体
│   │   ├── agent.rs             # Agent 结构体（pid, type, name, project）
│   │   └── memory.rs            # MemoryEntry, MemoryIndex
│   ├── services/
│   │   ├── mod.rs
│   │   ├── project_scanner.rs   # 扫描 ~/.claude/projects/ 发现项目
│   │   ├── agent_detector.rs    # ps + lsof 检测运行中 agent
│   │   ├── session_reader.rs    # 解析 JSONL 会话文件
│   │   ├── memory_indexer.rs    # 会话 → 摘要 → 索引
│   │   ├── terminal_launcher.rs # osascript 启动终端
│   │   └── claudemd_writer.rs   # 更新 CLAUDE.md
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── sidebar.rs           # 项目列表侧边栏
│   │   ├── dashboard.rs         # 项目详情主面板
│   │   ├── agent_card.rs        # Agent 信息卡片
│   │   ├── memory_view.rs       # 记忆浏览/搜索界面
│   │   └── new_agent_dialog.rs  # 新建 Agent 对话框
│   └── utils/
│       ├── mod.rs
│       ├── process.rs           # 进程相关工具函数
│       └── config.rs            # 应用配置（终端偏好等）
├── assets/                      # 图标、字体等
├── Cargo.toml
└── README.md
```

## 关键技术依赖

| crate | 版本 | 用途 |
|---|---|---|
| `dioxus` | 0.6.x | GUI 框架 |
| `dioxus-desktop` | 0.6.x | 桌面端渲染 |
| `serde` + `serde_json` | 1.x | JSON/JSONL 序列化 |
| `tokio` | 1.x | 异步运行时 |
| `notify` | 7.x | 文件系统事件监听 |
| `chrono` | 0.4.x | 时间处理 |

**暂不引入**:
- `tantivy`（全文搜索）— MVP 阶段用简单关键词匹配即可
- `reqwest`（网络请求）— 暂无网络需求

## 数据持久化

| 数据 | 存储位置 | 格式 |
|---|---|---|
| Agent 备注名 | `{project}/.agentdesk/agents.json` | JSON |
| 记忆索引 | `{project}/.agentdesk/index.json` | JSON |
| 会话摘要 | `{project}/.agentdesk/sessions/*.md` | Markdown |
| 结构化记忆 | `{project}/.agentdesk/memory.md` | Markdown |
| 应用配置 | `~/.agentdesk/config.json` | JSON |

**安全措施 — 防止记忆数据泄露到 git**:
- **写入前提条件（fail-closed）**：在创建 `.agentdesk/` 目录或写入任何记忆文件之前，必须通过以下全部检查：
  1. 项目路径在已批准的白名单中
  2. 定位仓库根目录（`git rev-parse --show-toplevel`）
  3. 检查 `.agentdesk/` 是否已被 git 跟踪或暂存（`git ls-files .agentdesk/`），若已跟踪则拒绝写入并回退到用户级存储
  4. 检查/更新 `.gitignore` 添加 `.agentdesk/` 条目
  5. **步骤 1（白名单）失败** → 完全不写入任何记忆，不回退，该项目仅可查看
  6. **步骤 2-4（git 安全检查）失败**（项目已批准但 git 保护无法就位） → 回退到 `~/.agentdesk/projects/{path_hash}/`（`path_hash` = 项目规范路径的 SHA256 前 16 位 + 人类可读名称，如 `a1b2c3d4-petpal/`），并在 UI 中提示用户
- 如果项目没有 `.gitignore`，则创建一个包含 `.agentdesk/` 的文件
- 非 git 仓库项目默认使用用户级存储（`~/.agentdesk/projects/{path_hash}/`），不在项目目录写入
- 敏感信息防护（多层策略）：
  - **摘要层面**：记忆摘要只保存决策/架构/变更概述，不保存原始对话全文
  - **正则过滤**：在写入前对摘要文本扫描常见敏感模式（`sk-*`、`ghp_*`、`Bearer *`、`password=*`、base64 编码的长字符串等），命中则替换为 `[REDACTED]`
  - **用户确认**：首次在项目中启用记忆存储时，提示用户确认（应用内设置，非每次弹窗）
  - **注意**：存储在项目目录是用户明确需求，.gitignore 自动写入是核心防线

## MVP 范围

**P0（首版必须）**:
- 项目列表展示（从 ~/.claude/projects/ 发现）
- Agent 进程检测与展示
- 新建 Agent（iTerm2 终端启动）
- 会话 JSONL 读取与基础摘要

**P1（第二版）**:
- 记忆索引与搜索
- CLAUDE.md 自动更新
- Agent 备注名编辑
- 终端窗口聚焦

**P2（后续）**:
- 记忆智能分类（可选接入 LLM）
- 多 Agent 协作视图
- 资源消耗监控（token 用量等）

## 前置条件

- 需要安装 Rust 工具链（rustup）
- 用户已安装 iTerm2（已确认）
- macOS 环境（AppleScript 依赖）

