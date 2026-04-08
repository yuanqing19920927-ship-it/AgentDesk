# AgentDesk - macOS AI Agent 管理应用设计文档

## 概述

AgentDesk 是一款 macOS 桌面应用，用于管理本机上运行的 AI Agent（Claude Code、Codex 等），提供项目长记忆管理、Agent 生命周期管理、协作编排、成本监控、审计追踪、终端集成等功能。

## 技术栈

- **语言**: Rust
- **GUI 框架**: Dioxus (桌面端)
- **异步运行时**: Tokio
- **不依赖 Xcode**

---

## 核心功能模块

### 模块 1: 项目长记忆系统

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
      "branch": "main",
      "topics": ["auth", "database"],
      "summary": "实现了用户认证模块，使用 JWT...",
      "keywords": ["jwt", "auth", "middleware"],
      "file_ref": "sessions/2026-04-08.md#auth-module"
    }
  ]
}
```

**CLAUDE.md 集成**:
仅在项目本地存储模式（`.agentdesk/` 位于项目目录内）时写入 CLAUDE.md。回退到用户级存储时不修改 CLAUDE.md。

项目本地模式插入内容：
```markdown
## Project Memory (AgentDesk)
- 项目记忆索引: .agentdesk/index.json
- 结构化记忆: .agentdesk/memory.md
- 会话摘要目录: .agentdesk/sessions/
- 使用方式: 需要历史上下文时，先读取 .agentdesk/memory.md 获取概览，再通过 index.json 定位详细记录
```

用户级存储回退模式：不修改 CLAUDE.md，记忆仅通过 AgentDesk UI 浏览。

### 模块 2: 项目管理面板

**项目发现**:
- 扫描 `~/.claude/projects/` 目录下的 JSONL 会话文件
- **从会话 JSONL 的 `cwd` 字段提取真实项目路径**（不依赖目录名反推，避免连字符歧义）
- **路径规范化**：对提取的 `cwd` 执行 `git rev-parse --show-toplevel` 获取仓库根目录作为项目标识（非 git 项目则使用 `cwd` 本身）。解析 symlink 为真实路径
- **Worktree 处理**：git worktree 的会话归属主仓库，但记忆条目中标注 `branch` 字段（从 JSONL 的 `gitBranch` 提取），Agent 读取时可按分支过滤相关记忆
- 建立并持久化 `claude_dir_name → canonical_project_root` **绑定**映射表（存储在 `~/.agentdesk/project_map.json`）。检测到同一 claude_dir_name 的 cwd 发生变化时：**阻止该来源的索引写入**，在 UI 中高亮提示路径漂移，用户确认后重新绑定或忽略
- **项目写入授权**：首次发现新项目路径时，仅在 UI 中展示。用户需在项目面板中手动"启用记忆"才将该路径加入白名单（`~/.agentdesk/approved_projects.json`）。未经批准的项目：**不在项目目录内写入任何文件**（`.agentdesk/` 不会被创建）。用户级元数据（如 `~/.agentdesk/agent_names.json` 中的备注名）允许存储，因其不触及项目仓库
- 遇到无法确定路径的目录时跳过并记录警告，不猜测
- 验证项目路径是否存在
- 读取项目基本信息（git info、语言、框架等）

**展示内容**:
- 项目名称 + 路径
- 运行中 Agent 数量（实时检测）
- 最近活跃时间
- 记忆条目数量
- 项目健康指标（见模块 8）

### 模块 3: Agent 检测与管理

**进程检测**:
```
ps aux → 过滤 claude/codex 进程
       → 解析命令行参数获取工作目录
       → 匹配到项目
```

具体匹配规则:
- Claude Code: 进程名含 `node` + 参数含 `claude`，通过 `lsof -p <pid>` 获取 cwd
- Codex: 进程名含 `codex`，关联 Codex.app
- 检测频率：每 3 秒轮询一次，配合进程退出信号

**Agent 管理功能**:
- 查看运行中 Agent 列表
- 编辑 Agent 备注名（持久化到用户级存储 `~/.agentdesk/agent_names.json`，以项目路径为 key，不受项目写入白名单限制）
- 点击 Agent 卡片 → 拉起对应终端窗口（通过 AppleScript 激活窗口）
- Agent 状态指示：运行中 / 等待输入 / 已完成 / 错误

### 模块 4: 新建 Agent

**流程**:
1. 用户选择项目（或从当前面板上下文自动填充）
2. 选择 Agent 类型（Claude Code / Codex）
3. 可选配置（权限模式、模型、初始 prompt）
4. 可选择从模板创建（见模块 7）
5. 确认 → 通过 osascript 在 iTerm2/Terminal.app 中打开新窗口
6. 自动执行启动命令（仅含枚举约束的参数，不含 initial prompt）
7. **Initial prompt 延迟发送**：Agent 进程启动后，使用与"快速指令发送"（模块 12）相同的安全通道：原子化 PID/cwd/REPL 验证 + `write text`。来自模板的 prompt 文本需用户在首次使用时确认。prompt 中的换行符替换为空格。**已知限制**：基于 AppleScript 终端控制的方案无法 100% 防止 REPL 误判，P3 阶段探索 CLI stdin/pipe 直连方案

**终端启动安全**:

**禁止**将项目路径或参数直接拼接进 shell 字符串。使用 AppleScript 的 `quoted form of` 对每个 token 转义。

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

**参数安全模型**：Agent 类型和启动参数使用 Rust 枚举严格约束，不接受自由文本：
```rust
enum AgentType { ClaudeCode, Codex }
enum PermissionMode { Default, DangerouslySkipPermissions, Plan }
// 每个参数单独 quoted form 转义，不拼接为 shell 字符串
// 不支持的参数值直接拒绝，不传递给 shell
```

### 模块 5: Agent 协作与编排

**任务编排引擎**:
- 定义 Agent 间的依赖关系：DAG（有向无环图）模型
- Agent A 完成后触发 Agent B，完成信号**仅基于进程退出码**（0 = 成功，非 0 = 失败）。不使用文件变更检测作为触发条件（共享仓库中不可靠）
- **幂等执行与崩溃恢复**：每个节点执行前写入持久化执行日志（`workflows/{workflow_id}/runs/{run_id}.json`）。节点状态机：
  - `pending` → `trigger_requested`（记录触发条件满足）→ `launching`（开始启动进程）→ `running`（确认进程 PID 存在）→ `completed`/`failed`
  - 仅在 `running` 状态确认（检测到进程 PID）后才视为已启动
  - 进入 `launching` 时记录 launch token（UUID）+ 目标终端标识。launch token 通过环境变量 `AGENTDESK_LAUNCH_TOKEN` 传递给子进程，重启后可通过进程表匹配。进入 `running` 时记录实际 PID
  - `trigger_requested` 和 `launching` 状态设置超时（默认 30s），超时后自动重试（最多 3 次）
  - 应用崩溃重启后：扫描进程表，通过 `AGENTDESK_LAUNCH_TOKEN` 环境变量匹配存活进程与执行日志。`launching` 状态的节点仅在确认无对应存活进程后才重试；匹配不确定时保持 `launching` 状态等待用户手动确认；`running` 状态的节点检查进程是否存活
- 编排配置持久化到 `{project}/.agentdesk/workflows/`（受统一写入安全策略保护）

**可视化编排画布**:
- 拖拽式 DAG 编辑器，节点 = Agent 任务，边 = 依赖关系
- 每个节点配置：Agent 类型、初始 prompt、完成条件、超时设置
- 实时展示执行状态（待运行 / 运行中 / 完成 / 失败）
- 支持暂停、重试单个节点

**上下文传递（结构化手递）**:
- Agent 完成后，生成**结构化交接数据**（非原始文本摘要）：
  ```json
  {
    "handoff_type": "workflow_step",
    "source_agent": "agent_001",
    "files_changed": ["src/auth.rs", "tests/auth_test.rs"],
    "exit_code": 0,
    "key_decisions": ["使用 JWT 而非 session", "添加了 refresh token 机制"],
    "warnings": []
  }
  ```
- 下游 Agent 接收的是 AgentDesk 从结构化数据生成的上下文描述，**不直接传递上游 Agent 的原始输出**
- 支持配置传递策略：仅文件变更列表 / 结构化决策摘要 / 完整交接报告
- 自动触发下游前需用户审批交接内容（可配置为自动/手动）

**工作流模板**:
- 预设常用编排模式：
  - "前端+后端+测试" 并行开发
  - "实现 → 代码审查 → 修复" 串行流水线
  - "调研 → 设计 → 实现" 瀑布流
- 用户可保存自定义工作流为模板

### 模块 6: 成本与用量监控

**数据采集**:
- **当前支持**：Claude Code（解析 `~/.claude/cost-tracker.log` 或会话 JSONL 中的 usage 字段）
- **Codex**：暂无公开的成本日志接口，UI 中标注为"未计量"，不纳入预算统计。后续 Codex 开放日志后扩展
- 按 session → agent → project → 全局 四级聚合
- 记录每次 API 调用的 input/output token 数、模型、费用

**仪表盘展示**:
- **项目维度**：每个项目的累计费用、本周/本月趋势图
- **Agent 维度**：单个 Agent 会话的 token 消耗明细
- **时间维度**：日/周/月费用柱状图、趋势线
- **模型维度**：各模型的使用占比饼图

**预算告警**:
- 可设置项目级/全局月度预算上限
- 达到 80% / 100% 时推送 macOS 通知
- 超预算后可配置行为：仅告警 / 阻止创建新 **Claude Code** Agent
- **未计量 Agent 处理**：在启用预算控制的项目中，启动 Codex 等未计量 Agent 时弹出警告"此 Agent 类型费用未知，不纳入预算统计"，用户确认后方可启动。预算仪表盘中 Codex 会话标注为"费用未知"

**数据存储**:
- 成本数据存储在 `~/.agentdesk/costs/` 下
- 按月归档：`costs/2026-04.json`
- 保留原始明细 + 预聚合的统计摘要

### 模块 7: 模板与预设系统

**Agent 模板**:
```json
{
  "id": "tmpl_001",
  "name": "代码审查专家",
  "agent_type": "ClaudeCode",
  "permission_mode": "Plan",
  "model": "opus",
  "initial_prompt": "请审查当前分支相对于 main 的所有变更...",
  "tags": ["review", "quality"]
}
```

**功能**:
- 创建/编辑/删除模板
- 从已有 Agent 会话"另存为模板"
- 模板分类与搜索（按标签）
- 一键从模板启动 Agent

**组合预设**:
- 定义多 Agent 启动组合（如"全栈开发套件"：前端 Agent + 后端 Agent + 测试 Agent）
- 一键同时启动组合中所有 Agent
- 组合可引用编排工作流（模块 5）

**导入导出**:
- 模板导出为 JSON 文件
- 从文件导入模板
- 存储位置：`~/.agentdesk/templates/`

### 模块 8: 通知系统

**事件源**:
- Agent 进程退出（正常完成 / 错误退出）
- Agent 等待用户输入（检测终端状态）
- 预算告警触发
- 编排工作流节点状态变更
- 记忆索引异常（dirty 状态、drift 检测）

**通知渠道**:
- macOS 原生通知（`NSUserNotification` / `UNUserNotificationCenter`，通过 Rust 的 `objc2` crate 调用）
- 应用内通知中心（铃铛图标 + badge 计数）

**通知规则配置**:
- 全局级别：全部通知 / 仅错误 / 静音
- 项目级别覆盖：可为特定项目单独配置
- 事件类型过滤：可逐个开关每种事件类型
- 勿扰时段设置

**通知历史**:
- 保留最近 500 条通知记录
- 可按类型/项目/时间过滤
- 点击通知跳转到相关 Agent/项目面板

### 模块 9: Agent 输出审计

**变更追踪**:
- 每个 Agent 会话开始时记录完整快照：`git status --porcelain`（含 untracked 文件列表）+ 所有 dirty 文件（已修改 + untracked）的内容 SHA256 哈希 + `git diff HEAD`（含已修改 tracked 文件的完整 preimage diff）。如果工作区不干净，审计报告标注为"脏工作区启动"，回滚功能降级为 diff 导出
- 会话结束时生成 diff 报告：新增/修改/删除的文件列表 + 统计（对比开始时的快照）
- diff 报告存储在 `.agentdesk/audits/{session_id}.json`（受统一写入安全策略保护）

**操作时间线**:
- 按时间轴展示所有 Agent 的文件操作
- 每条记录：时间、Agent 名称、操作类型（创建/修改/删除）、文件路径、diff 行数
- 支持按 Agent / 文件路径 / 时间范围过滤
- **并发检测**：当同一项目在同一时段有多个 Agent 或用户在编辑时，审计报告标注为"共享编辑期间"，变更归属降级为"会话期间的仓库变更"而非精确到单个 Agent

**变更回滚**:
- **前提条件**：仅在 Agent 会话有明确的 commit 边界时才提供回滚功能（即会话的变更已独立 commit，未与其他 Agent/用户的 commit 混合）
- 如果 commit 范围包含非本 Agent 的变更，**不提供一键回滚**，改为导出 diff 供用户手动审查和 reverse-apply
- 回滚前展示影响范围预览 + 冲突检测，需用户二次确认
- 通过 `git revert` 实现（非 destructive）
- 未来增强：Agent 在隔离 worktree/分支中运行时，回滚等价于丢弃分支，更安全

**审计报告**:
- 按项目/时间范围生成审计摘要
- 统计：总变更文件数、代码行增删、各 Agent 贡献占比

### 模块 10: Agent 日志实时查看

**实时日志流**:
- 在 AgentDesk 内嵌终端输出查看器（只读）
- 数据源：读取 Agent 进程的 pty 输出，或轮询 Claude Code 的会话 JSONL 增量
- 推荐方案：监听 JSONL 文件增量（与记忆系统共享 notify watcher），解析 assistant 消息并格式化展示

**日志功能**:
- 实时滚动输出
- 搜索/过滤（按关键词、消息类型：user/assistant/tool_use/tool_result）
- 折叠长输出（代码块、工具调用结果）
- 时间戳标注
- 导出会话日志为 Markdown

### 模块 11: 项目健康度仪表盘

**指标采集**（从 Agent 会话和 git 历史中提取）:
- **构建状态**：最近一次构建是否成功（从会话中检测构建命令的退出码）
- **测试通过率**：从会话中提取测试运行结果
- **Agent 活跃度**：各项目的 Agent 使用频率热力图（按日/周）
- **代码变更速率**：git commit 频率、代码行变化趋势
- **记忆丰富度**：记忆条目数、覆盖主题数

**展示**:
- 项目卡片上的健康指示灯（绿/黄/红）
- 点击展开详细指标面板
- 全局仪表盘：所有项目的健康度概览排列

### 模块 12: 快捷操作

**全局快捷键**:
- `Cmd+Shift+A`：唤出 AgentDesk 主窗口（可自定义）
- `Cmd+Shift+N`：快速新建 Agent（弹出精简版创建对话框）
- 注册方式：通过 macOS `CGEventTap` 或 `NSEvent.addGlobalMonitorForEvents`（Rust `objc2` 调用）

**命令面板（Cmd+K）**:
- Spotlight 风格的模糊搜索输入框
- 可搜索：项目名、Agent 名、模板名、操作命令
- 操作示例：
  - "petpal" → 跳转到 petpal 项目
  - "new claude" → 新建 Claude Code Agent
  - "kill agent-3" → 终止指定 Agent
  - "cost this month" → 跳转到本月费用面板

**快速指令发送**:
- 选中一个 Agent → `Cmd+Enter` 打开指令输入框
- **安全发送机制**：发送前必须验证目标终端状态：
  1. 通过 PID 确认 Agent 进程仍在运行
  2. 通过 `lsof -p <pid>` 确认 cwd 未变化（防止 tab 被复用）
  3. 仅在 Agent REPL 就绪状态（非 shell prompt）时发送
  4. **原子化检查+发送**：将 PID/cwd 验证和 `write text` 操作包含在同一个 AppleScript 脚本块中执行，减少 check-to-send 的竞态窗口
  5. 仅允许白名单内的 slash 命令（如 "/commit"、"/review-pr"）直接发送；自由文本需用户在发送前二次确认
- 常用指令收藏（预置 slash 命令列表）

---

## 架构设计

```
agentdesk/
├── src/
│   ├── main.rs                     # 入口，初始化 Dioxus + Tokio
│   ├── app.rs                      # Dioxus 根组件，路由，全局状态
│   ├── models/
│   │   ├── mod.rs
│   │   ├── project.rs              # Project 结构体
│   │   ├── agent.rs                # Agent 结构体（pid, type, name, project, status）
│   │   ├── memory.rs               # MemoryEntry, MemoryIndex
│   │   ├── workflow.rs             # WorkflowDef, WorkflowNode, WorkflowEdge
│   │   ├── template.rs             # AgentTemplate, ComboPreset
│   │   ├── cost.rs                 # CostRecord, CostSummary, Budget
│   │   ├── audit.rs                # AuditEntry, DiffReport
│   │   └── notification.rs         # NotificationEvent, NotificationRule
│   ├── services/
│   │   ├── mod.rs
│   │   ├── project_scanner.rs      # 扫描 ~/.claude/projects/ 发现项目
│   │   ├── agent_detector.rs       # ps + lsof 检测运行中 agent
│   │   ├── session_reader.rs       # 解析 JSONL 会话文件
│   │   ├── memory_indexer.rs       # 会话 → 摘要 → 索引
│   │   ├── terminal_launcher.rs    # osascript 启动终端
│   │   ├── claudemd_writer.rs      # 更新 CLAUDE.md
│   │   ├── workflow_engine.rs      # DAG 编排引擎，任务调度
│   │   ├── cost_tracker.rs         # 成本日志解析与聚合
│   │   ├── audit_recorder.rs       # 变更追踪与 diff 快照
│   │   ├── notification_service.rs # 通知调度与 macOS 推送
│   │   ├── template_manager.rs     # 模板 CRUD 与启动
│   │   ├── log_streamer.rs         # JSONL 实时流解析与格式化
│   │   └── health_monitor.rs       # 项目健康指标采集
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── sidebar.rs              # 项目列表侧边栏
│   │   ├── dashboard.rs            # 项目详情主面板
│   │   ├── agent_card.rs           # Agent 信息卡片
│   │   ├── memory_view.rs          # 记忆浏览/搜索界面
│   │   ├── new_agent_dialog.rs     # 新建 Agent 对话框
│   │   ├── workflow_canvas.rs      # 可视化编排画布
│   │   ├── cost_dashboard.rs       # 费用统计仪表盘
│   │   ├── audit_timeline.rs       # 操作审计时间线
│   │   ├── notification_center.rs  # 通知中心面板
│   │   ├── template_browser.rs     # 模板浏览与管理
│   │   ├── log_viewer.rs           # 实时日志查看器
│   │   ├── health_dashboard.rs     # 健康度仪表盘
│   │   └── command_palette.rs      # Cmd+K 命令面板
│   └── utils/
│       ├── mod.rs
│       ├── process.rs              # 进程工具函数
│       ├── config.rs               # 应用配置
│       ├── hotkey.rs               # 全局快捷键注册
│       └── macos_notify.rs         # macOS 通知桥接
├── assets/                         # 图标、字体等
├── Cargo.toml
└── README.md
```

## 关键技术依赖

| crate | 用途 |
|---|---|
| `dioxus` + `dioxus-desktop` | GUI 框架 + 桌面渲染 |
| `serde` + `serde_json` | JSON/JSONL 序列化 |
| `tokio` | 异步运行时（定时任务、actor、文件监听） |
| `notify` | 文件系统事件监听 |
| `chrono` | 时间处理 |
| `objc2` + `objc2-foundation` | macOS 原生 API 调用（通知、全局快捷键） |
| `petgraph` | DAG 图数据结构（编排引擎） |
| `sha2` | SHA256 哈希（path_hash、内容校验） |
| `fs2` | 跨平台文件锁（flock） |

## 数据持久化

| 数据 | 存储位置 | 格式 |
|---|---|---|
| Agent 备注名 | `~/.agentdesk/agent_names.json` | JSON |
| 记忆索引 | `{project}/.agentdesk/index.json` | JSON |
| 会话摘要 | `{project}/.agentdesk/sessions/*.md` | Markdown |
| 结构化记忆 | `{project}/.agentdesk/memory.md` | Markdown |
| 编排工作流 | `{project}/.agentdesk/workflows/*.json` | JSON |
| 审计记录 | `{project}/.agentdesk/audits/*.json` | JSON |
| 应用配置 | `~/.agentdesk/config.json` | JSON |
| 项目映射 | `~/.agentdesk/project_map.json` | JSON |
| 项目白名单 | `~/.agentdesk/approved_projects.json` | JSON |
| 成本数据 | `~/.agentdesk/costs/{yyyy-mm}.json` | JSON |
| Agent 模板 | `~/.agentdesk/templates/*.json` | JSON |
| 组合预设 | `~/.agentdesk/presets/*.json` | JSON |
| 通知历史 | `~/.agentdesk/notifications.json` | JSON |
| 通知规则 | `~/.agentdesk/notification_rules.json` | JSON |

## 安全措施

### 终端启动安全
- **禁止** shell 字符串拼接，所有路径和参数使用 `quoted form of` 单独转义
- Agent 类型和参数使用 Rust 枚举约束，不接受自由文本
- 路径校验：必须是已存在的绝对路径目录

### 项目目录写入安全（统一策略）
所有向 `{project}/.agentdesk/` 写入的模块（记忆、工作流、审计）共享同一安全策略：
- **写入前提条件（fail-closed）**：
  1. 项目路径在已批准的白名单中（步骤 1 失败 → 完全不写入）
  2. 定位仓库根目录
  3. 检查 `.agentdesk/` 是否已被 git 跟踪或暂存，若是则拒绝并回退
  4. 检查/更新 `.gitignore`
  5. 步骤 2-4 失败 → 回退到 `~/.agentdesk/projects/{path_hash}/`
- 敏感信息过滤：摘要仅存概述，正则扫描并替换 API key/token/密码为 `[REDACTED]`
- **审计记录特别注意**：diff 内容可能包含敏感代码/密钥，审计 JSON 中的 diff 片段同样执行正则过滤

### 审计安全
- 回滚操作使用 `git revert`（非 destructive），回滚前需用户二次确认
- 审计记录只读，不提供从 UI 删除审计记录的功能

## MVP 分期

**P0（首版 — 核心可用）**:
- 模块 2: 项目列表展示
- 模块 3: Agent 进程检测与展示
- 模块 4: 新建 Agent（iTerm2 终端启动）
- 模块 1: 会话 JSONL 读取与基础摘要（不含 CLAUDE.md 集成）

**P1（第二版 — 记忆与管理完善）**:
- 模块 1: 完整记忆索引、搜索、CLAUDE.md 自动更新
- 模块 3: Agent 备注名编辑、终端窗口聚焦
- 模块 7: 模板与预设系统
- 模块 8: 通知系统（基础版：进程退出通知）

**P2（第三版 — 监控与审计）**:
- 模块 6: 成本与用量监控
- 模块 9: Agent 输出审计与变更追踪
- 模块 10: Agent 日志实时查看
- 模块 11: 项目健康度仪表盘

**P3（第四版 — 协作与效率）**:
- 模块 5: Agent 协作与编排（DAG 引擎 + 可视化画布）
- 模块 12: 快捷操作（全局热键、命令面板、快速指令）

## 前置条件

- 需要安装 Rust 工具链（rustup）
- 用户已安装 iTerm2（已确认）
- macOS 环境（AppleScript、NSUserNotification 等依赖）
