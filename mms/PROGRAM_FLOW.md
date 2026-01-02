# Program Flow: MMS vs Legacy codex-rs

## Executive Summary

Both systems follow a similar architecture but with different complexity levels:

| Aspect | MMS | Legacy codex-rs |
|--------|-----|-----------------|
| **Entry Points** | 1 (CLI) | 3 (CLI, TUI, exec-server) |
| **Processor** | Single loop | Multi-task with handlers |
| **Tool Routing** | Direct dispatch | Registry + router pattern |
| **Streaming** | Basic events | Rich event protocol |
| **Config** | Single layer | Multi-layer cascade |

---

## Phase-by-Phase Comparison

### PHASE 1: CLI Entry Point

#### MMS Flow
```
cli/src/main.rs:main()
    │
    ├── init_logging()                    # Setup tracing
    ├── Cli::parse()                      # Parse clap args
    │
    └── match cli.command {
        Commands::Run { mode: Cli }
            └── commands::run_cli()       # cli_mode.rs
        Commands::Shell { ... }
            └── commands::run_shell()
        Commands::Mcp { ... }
            └── commands::run_mcp()
        ...
    }
```

#### Legacy Flow
```
cli/src/main.rs:main()
    │
    ├── arg0_dispatch_or_else()           # Linux sandbox dispatch
    ├── MultitoolCli::parse()             # Parse args
    │
    └── match cli.command {
        None
            └── run_interactive_tui()     # TUI mode (ratatui)
        Some(Exec)
            └── codex_exec::run_main()    # Non-interactive
        Some(Review)
            └── codex_exec::run_main()    # Review task
        Some(MCP)
            └── mcp_server::run()         # MCP server mode
        Some(Skills)
            └── skills::cmd::run()        # Skills management
        ...
    }
```

**Key Differences:**
- Legacy has TUI as default mode, MMS has CLI mode
- Legacy supports MCP server mode (act as MCP server)
- Legacy has review/compact task modes
- Legacy dispatches to sandbox process via arg0

---

### PHASE 2: Configuration Loading

#### MMS Flow
```
config/src/loader.rs:
    │
    ConfigLoader::from_default_home()
        │
        └── Load ~/.agent/config.toml
            └── Parse into Config struct
                ├── model: String
                ├── provider_id: String
                ├── providers: HashMap
                ├── approval_mode: ApprovalMode
                ├── sandbox: SandboxConfig
                └── tools: ToolsConfig
```

#### Legacy Flow
```
core/src/config_loader/mod.rs:
    │
    load_config_layers_state()
        │
        ├── Load /etc/codex/requirements.toml    # Admin restrictions
        ├── Load /etc/codex/config.toml          # System config
        ├── Load $CODEX_HOME/config.toml         # User config (~/.codex)
        ├── Load .codex/config.toml              # Project config (git root)
        ├── Load ./codex/config.toml             # Tree config (parent dirs)
        ├── Load ./config.toml                   # Current dir config
        └── Apply CLI overrides (-c key=value)
            │
            └── Merge into ConfigLayerStack
                ├── Priority: CLI > Project > User > System > Admin
                ├── Requirements: Enforced restrictions
                └── Final Config object
```

**Key Differences:**
- Legacy has 6-layer config cascade with priority
- Legacy supports admin/MDM managed restrictions
- Legacy has requirements.toml for enforced policies
- MMS has single config file approach

---

### PHASE 3: Agent & Session Initialization

#### MMS Flow
```
core/src/agent.rs:
    │
    Agent::new()
        └── Arc<RwLock<Option<Session>>>     # Empty session
    │
    agent.start(config)
        │
        ├── ToolRegistry::new()
        │   └── register_default_handlers()
        │       ├── ReadFileHandler
        │       ├── WriteFileHandler
        │       ├── EditFileHandler
        │       ├── ListDirectoryHandler
        │       ├── GlobHandler
        │       ├── GrepHandler
        │       └── ShellHandler
        │
        ├── Session::new(config, registry)
        │   ├── session_id: Uuid
        │   ├── conversation_id
        │   └── state: Idle
        │
        └── session.start()
            ├── Create channels (submission, event)
            ├── Create ToolRouter
            ├── Create Executor
            └── spawn_processor()    # Async task
```

#### Legacy Flow
```
core/src/codex.rs:
    │
    Codex::spawn(config, auth, models, skills, history, source)
        │
        ├── Create channels
        │   ├── tx_sub, rx_sub     # Submission queue
        │   └── tx_event, rx_event # Event stream
        │
        ├── Load skills (if enabled)
        │   └── SkillsManager::skills_for_cwd()
        │       ├── Scan .codex/skills/
        │       ├── Scan $CODEX_HOME/skills/
        │       └── Parse SKILL.md files
        │
        ├── Create Session
        │   ├── SessionState
        │   ├── RolloutRecorder          # Persist conversation
        │   ├── McpConnectionManager     # MCP servers
        │   └── OtelManager              # Telemetry
        │
        ├── Emit SessionConfiguredEvent
        │
        └── spawn submission_loop()      # Main agent task
```

**Key Differences:**
- Legacy loads skills at startup
- Legacy has RolloutRecorder for persistence
- Legacy integrates OtelManager for telemetry
- Legacy has MCP connection management built-in
- MMS has simpler initialization path

---

### PHASE 4: Main Processing Loop

#### MMS Processor Loop
```
core/src/processor.rs:Processor::run()
    │
    loop {
        submission = rx_sub.recv()      # Block for submission
        │
        match submission.operation {
            │
            UserMessage(msg)
                └── handle_user_message()
                    │
                    ├── context.record(user_message)
                    ├── turn = Turn::new()
                    │
                    └── loop (max 50 iterations) {
                        │
                        ├── call_llm_with_retry()
                        │   ├── Build messages from context
                        │   ├── Stream LLM response
                        │   ├── Emit AgentThinking/AgentMessage events
                        │   └── Collect tool_calls
                        │
                        ├── if tool_calls.is_empty() {
                        │       context.record(assistant_message)
                        │       break
                        │   }
                        │
                        └── execute_tool_calls_parallel()
                            ├── Phase 1: Gather approvals
                            ├── Phase 2: Filter approved
                            ├── Phase 3: Execute parallel (max 5)
                            └── Record results to context
                    }
            │
            Approve(id, decision)
                └── approval_manager.set_decision()
            │
            Reject(id, reason)
                └── approval_manager.set_rejection()
            │
            Interrupt
                └── cancel_token.cancel()
            │
            Shutdown
                └── break loop
        }
    }
```

#### Legacy Submission Loop
```
core/src/codex.rs:submission_loop()
    │
    loop {
        submission = rx_sub.recv()
        │
        match submission.op {
            │
            Op::UserTurn { items, cwd, approval_policy, sandbox_policy, ... }
                └── handlers::user_input_or_turn()
                    │
                    ├── Create TurnContext
                    │   ├── ModelClient
                    │   ├── ToolsConfig
                    │   ├── TruncationPolicy
                    │   └── Overrides
                    │
                    └── sess.spawn_task(turn_context, items, RegularTask)
                        └── run_turn()  # Spawned as separate task
            │
            Op::ExecApproval { id, decision }
                └── handlers::exec_approval()
            │
            Op::PatchApproval { id, decision }
                └── handlers::patch_approval()
            │
            Op::Interrupt
                └── handlers::interrupt()
            │
            Op::Shutdown
                └── break
            │
            Op::Review { ... }
                └── handlers::review_task()
            │
            Op::Compact { ... }
                └── handlers::compact_task()
            │
            Op::GhostSnapshot { ... }
                └── handlers::ghost_snapshot()
        }
    }
```

**Key Differences:**
- Legacy spawns turn as separate task
- Legacy has dedicated handlers for each operation type
- Legacy supports Review, Compact, GhostSnapshot operations
- MMS handles everything in single processor loop
- Legacy has more granular approval types (Exec vs Patch)

---

### PHASE 5: Turn Execution & LLM Streaming

#### MMS Turn Flow
```
core/src/processor.rs:call_llm()
    │
    ├── Build tool definitions from specs
    │
    ├── build_messages_for_request()
    │   └── Convert context to API messages
    │
    ├── Create CompletionRequest
    │   ├── model
    │   ├── messages
    │   ├── tools
    │   └── stream: true
    │
    ├── client.stream(request)
    │   └── Returns ResponseStream
    │
    └── for event in stream {
            match event {
                StreamEvent::Start
                    → Set response_id, model

                StreamEvent::Delta { content, reasoning }
                    → Emit AgentThinking or AgentMessageDelta

                StreamEvent::ToolCallStart { id, name }
                    → Create tool call accumulator

                StreamEvent::ToolCallDelta { arguments }
                    → Accumulate JSON arguments

                StreamEvent::ToolCallEnd
                    → Mark tool call complete

                StreamEvent::End { usage }
                    → Update token counts
            }
        }
```

#### Legacy Turn Flow
```
core/src/codex.rs:try_run_turn()
    │
    ├── Create ToolCallRuntime
    │
    ├── Load MCP tools from McpConnectionManager
    │
    ├── Create ToolRouter
    │   └── ToolRouter::from_config(tools_config, mcp_tools)
    │
    ├── Build Prompt
    │   ├── input items
    │   ├── tool specs (native + MCP)
    │   ├── parallel_tool_calls flag
    │   └── output schema
    │
    ├── turn_context.client.stream(prompt)
    │
    └── for event in stream {
            match event {
                ResponseEvent::Created
                    → emit_turn_started()

                ResponseEvent::OutputItemAdded(item)
                    → emit_turn_item_started()

                ResponseEvent::OutputTextDelta(delta)
                    → emit AgentMessageContentDeltaEvent

                ResponseEvent::OutputItemDone(item)
                    → handle_output_item_done()
                    │
                    └── if tool_call:
                            ToolRouter::build_tool_call()
                            Queue execution future

                ResponseEvent::Completed { usage }
                    → Update token counts, break
            }
        }
```

**Key Differences:**
- Legacy has MCP tool integration in turn flow
- Legacy has richer event types (OutputItemAdded, OutputItemDone)
- Legacy builds tool calls via ToolRouter pattern
- Legacy queues tool execution as futures
- MMS accumulates tool calls during stream

---

### PHASE 6: Tool Execution

#### MMS Tool Execution
```
core/src/processor.rs:execute_tool_calls_parallel()
    │
    ├── PHASE 1: Gather Approvals
    │   │
    │   for each tool_call {
    │       if needs_approval(call) {
    │           emit ApprovalRequiredEvent
    │           spawn request_command_approval()
    │               → Wait for Approve/Reject operation
    │               → Timeout after 30s
    │       }
    │   }
    │   await all approval futures
    │
    ├── PHASE 2: Filter Approved
    │   │
    │   for each tool_call {
    │       match approval_result {
    │           Approved → add to execution list
    │           Rejected → create error ToolOutput
    │           TimedOut → create error ToolOutput
    │       }
    │   }
    │
    └── PHASE 3: Execute Parallel
        │
        parallel_executor.execute_with_cancellation(calls)
            │
            └── for each call (max 5 concurrent) {
                    ctx = ToolContext::new(config)
                    router.execute(&ctx, call)
                        │
                        └── handler = registry.get(call.name)
                            handler.execute(&ctx, call)
                }
```

#### Legacy Tool Execution
```
core/src/tools/parallel.rs:ToolCallRuntime::handle_tool_call()
    │
    ├── Check parallel support
    │   └── Acquire read lock (parallel) or write lock (sequential)
    │
    ├── Spawn execution task
    │   └── AbortOnDropHandle for cancellation
    │
    └── dispatch_tool_call()
            │
            ToolRouter::dispatch_tool_call(session, turn, tracker, call)
                │
                ├── Look up handler in ToolRegistry
                │
                ├── Create ToolInvocation
                │   ├── session reference
                │   ├── turn context
                │   ├── diff tracker
                │   └── call details
                │
                ├── Wait for tool call gate (if mutating)
                │   └── Ensures dangerous commands run sequentially
                │
                ├── handler.handle(invocation)
                │   └── Tool-specific execution
                │
                ├── Log via OtelManager
                │
                └── Convert ToolOutput to ResponseInputItem
```

**Key Differences:**
- Legacy has read/write lock pattern for parallel vs sequential
- Legacy tracks diffs via TurnDiffTracker
- Legacy has OtelManager integration
- Legacy has "tool call gate" for dangerous operations
- MMS has simpler phase-based execution

---

### PHASE 7: Tool Handlers

#### MMS Tool Handlers
```
tools/src/handlers/

ReadFileHandler (read_file.rs)
    └── Read file, return content or error

WriteFileHandler (write_file.rs)
    └── Write content to file

EditFileHandler (edit_file.rs)
    └── Apply edits (search/replace, line edits)

ListDirectoryHandler (list_dir.rs)
    └── List directory with filtering

GlobHandler (glob.rs)
    └── Glob pattern matching

GrepHandler (grep.rs)
    └── Regex search in files

ShellHandler (shell.rs)
    └── Execute shell commands
        ├── Check exec policy
        ├── Apply sandbox if configured
        ├── Execute with timeout
        └── Capture output

BackgroundHandler (background.rs)
    └── Run commands in background

McpHandler (mcp_handler.rs)
    └── Forward to MCP server
```

#### Legacy Tool Handlers
```
core/src/tools/handlers/

shell.rs
    └── Shell execution with:
        ├── is_dangerous_command() check
        ├── is_safe_command() whitelist
        ├── Approval flow integration
        └── Sandbox execution

read_file.rs
    └── File reading

list_dir.rs
    └── Directory listing

apply_patch.rs
    └── Apply Git-style patches
        ├── Parse unified diff
        ├── Apply hunks
        └── Handle conflicts

mcp.rs
    └── MCP tool forwarding

grep_files.rs
    └── File content search

container.rs
    └── Container tool execution
```

**Key Differences:**
- Legacy has is_dangerous_command() / is_safe_command() analysis
- Legacy has apply_patch handler (unified diff support)
- Legacy has container handler
- MMS has EditFile (inline edits vs patches)
- Both have similar core handlers

---

### PHASE 8: Event Emission & Output

#### MMS Events
```
protocol/src/event.rs:

EventMessage enum:
    ├── AgentThinking { content }        # Reasoning
    ├── AgentMessageDelta { delta }      # Streaming text
    ├── AgentMessage { content }         # Final message
    ├── ToolCallStarted { id, name }     # Tool begins
    ├── ToolCallCompleted { id, output } # Tool ends
    ├── ApprovalRequired { id, command } # Needs approval
    ├── TurnCompleted { tokens, duration }
    ├── SessionEnded
    └── ShutdownComplete
```

#### Legacy Events
```
core/src/event_types.rs:

Event types include:
    ├── SessionConfiguredEvent
    ├── TaskStartedEvent
    ├── ItemStartedEvent
    ├── ItemCompletedEvent
    ├── AgentMessageContentDeltaEvent
    ├── ReasoningContentDeltaEvent
    ├── ExecApprovalRequestEvent
    ├── PatchApprovalRequestEvent
    ├── ToolOutputEvent
    ├── TokenCountEvent
    ├── TurnDiffEvent              # File changes this turn
    ├── McpServerStatusEvent       # MCP server state
    ├── ErrorEvent
    ├── ShutdownCompleteEvent
    └── ... many more
```

**Key Differences:**
- Legacy has TurnDiffEvent for tracking file changes
- Legacy separates ExecApproval and PatchApproval
- Legacy has McpServerStatusEvent
- Legacy has more granular event types
- MMS has simpler event model

---

## Module Responsibility Matrix

| Function | MMS Module | Legacy Module |
|----------|------------|---------------|
| **CLI Parsing** | cli/main.rs | cli/main.rs |
| **TUI Interface** | ❌ | tui/ crate |
| **Config Loading** | config/loader.rs | core/config_loader/ |
| **Agent Orchestration** | core/agent.rs | core/codex.rs |
| **Session Management** | core/session/ | core/session.rs |
| **Turn Execution** | core/processor.rs | core/codex.rs (run_turn) |
| **LLM Client** | providers/clients/ | core/client.rs |
| **Tool Registry** | tools/registry.rs | core/tools/registry.rs |
| **Tool Router** | tools/router.rs | core/tools/router.rs |
| **Tool Handlers** | tools/handlers/ | core/tools/handlers/ |
| **Approval Flow** | core/approval.rs | core/approval.rs |
| **Parallel Execution** | core/parallel.rs | core/tools/parallel.rs |
| **Retry Logic** | core/retry.rs | core/retry.rs |
| **Context Management** | core/context/ | core/context.rs |
| **History/Rollout** | core/history/ | core/rollout.rs |
| **Skills** | skills/ (stub) | core/skills/ |
| **MCP Client** | mcp-client/ | core/mcp/ |
| **MCP Server** | ❌ | codex-mcp-server/ |
| **Sandbox (Linux)** | linux-sandbox/ | linux-sandbox/ |
| **Sandbox (macOS)** | ❌ | seatbelt/ |
| **Exec Policy** | execpolicy/ | execpolicy/ |
| **Git Integration** | git/ | git/ |
| **Error Types** | common/error.rs | core/error.rs |
| **Protocol Types** | protocol/ | protocol/ |

---

## Data Flow Diagrams

### MMS Request Flow
```
┌─────────────────────────────────────────────────────────────────────┐
│                           CLI (cli_mode.rs)                          │
├─────────────────────────────────────────────────────────────────────┤
│  User Input ──► agent.send_message() ──► Submission Channel         │
│                                                                      │
│  agent.next_event() ◄── Event Channel ◄── Processor Events          │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      PROCESSOR (processor.rs)                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐           │
│  │   Context    │◄──►│     LLM      │◄──►│    Tools     │           │
│  │   Manager    │    │    Client    │    │   Executor   │           │
│  └──────────────┘    └──────────────┘    └──────────────┘           │
│         │                   │                   │                    │
│         ▼                   ▼                   ▼                    │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐           │
│  │   History    │    │  Anthropic/  │    │    Tool      │           │
│  │   Storage    │    │   OpenAI/    │    │   Router     │           │
│  │              │    │    Groq      │    │              │           │
│  └──────────────┘    └──────────────┘    └──────────────┘           │
│                                                 │                    │
│                                                 ▼                    │
│                                          ┌──────────────┐           │
│                                          │   Handlers   │           │
│                                          │  ┌────────┐  │           │
│                                          │  │ Shell  │  │           │
│                                          │  │ Read   │  │           │
│                                          │  │ Write  │  │           │
│                                          │  │ Edit   │  │           │
│                                          │  │ Glob   │  │           │
│                                          │  │ Grep   │  │           │
│                                          │  │ MCP    │  │           │
│                                          │  └────────┘  │           │
│                                          └──────────────┘           │
└─────────────────────────────────────────────────────────────────────┘
```

### Legacy Request Flow
```
┌─────────────────────────────────────────────────────────────────────┐
│                    CLI / TUI / Exec-Server                           │
├─────────────────────────────────────────────────────────────────────┤
│  User Input ──► conversation.submit(Op) ──► Submission Channel      │
│                                                                      │
│  conversation.next_event() ◄── Event Channel ◄── Session Events     │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    SUBMISSION LOOP (codex.rs)                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Op::UserTurn ──► handlers::user_input_or_turn()                    │
│  Op::Review ──► handlers::review_task()                             │
│  Op::Compact ──► handlers::compact_task()                           │
│  Op::GhostSnapshot ──► handlers::ghost_snapshot()                   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      TURN EXECUTION                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐    │
│  │  Context   │  │   Skills   │  │    MCP     │  │  Rollout   │    │
│  │  Manager   │  │  Manager   │  │  Manager   │  │  Recorder  │    │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘    │
│        │               │               │               │            │
│        ▼               ▼               ▼               ▼            │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                     TurnContext                              │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │   │
│  │  │  Model   │  │  Tools   │  │Truncation│  │   Diff   │    │   │
│  │  │  Client  │  │  Config  │  │  Policy  │  │ Tracker  │    │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    ToolRouter                                │   │
│  │  ┌──────────────────────────────────────────────────────┐   │   │
│  │  │                   ToolRegistry                        │   │   │
│  │  │  ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐  │   │   │
│  │  │  │ Shell │ │ Read  │ │ List  │ │ Patch │ │  MCP  │  │   │   │
│  │  │  │       │ │ File  │ │  Dir  │ │ Apply │ │       │  │   │   │
│  │  │  └───────┘ └───────┘ └───────┘ └───────┘ └───────┘  │   │   │
│  │  └──────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│                     ┌────────────────┐                              │
│                     │  OtelManager   │                              │
│                     │  (Telemetry)   │                              │
│                     └────────────────┘                              │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Critical Implementation Gaps

Based on the flow comparison, these are the modules MMS needs to match legacy:

### 1. Skills System
```
Legacy: core/src/skills/
    ├── manager.rs      # SkillsManager - loads/caches skills
    ├── loader.rs       # SkillLoader - parses SKILL.md files
    └── injection.rs    # Injects skill info into prompts

MMS: skills/ (stub only)
    ├── skill.rs        # Basic Skill trait
    └── registry.rs     # Empty registry

NEEDED:
    ├── SkillLoader - Parse .md files with YAML frontmatter
    ├── SkillsManager - Multi-root loading (repo/user/system)
    ├── Skill injection - Format skills for system prompt
    └── Slash command execution
```

### 2. Command Safety Analysis
```
Legacy: core/src/tools/handlers/shell.rs
    ├── is_dangerous_command()   # Detects rm -rf, sudo, etc.
    ├── is_safe_command()        # Whitelists ls, cat, etc.
    └── Command classification for approval

MMS: tools/src/handlers/shell.rs
    └── No safety analysis

NEEDED:
    ├── DangerousCommandDetector
    ├── SafeCommandWhitelist
    └── Integration with approval flow
```

### 3. Task System
```
Legacy: core/src/codex.rs handlers
    ├── Op::Review       # Code review task
    ├── Op::Compact      # Context compaction
    ├── Op::GhostSnapshot # State snapshot
    └── Op::Undo         # Undo changes

MMS: None

NEEDED:
    ├── ReviewTask - Run code review
    ├── CompactTask - Summarize/compact context
    ├── UndoTask - Revert file changes
    └── Task abstraction layer
```

### 4. Turn Diff Tracker
```
Legacy: core/src/turn_diff_tracker.rs
    ├── Track file changes per turn
    ├── Enable undo functionality
    └── Emit TurnDiffEvent

MMS: None

NEEDED:
    ├── TurnDiffTracker struct
    ├── Integration with tool handlers
    └── TurnDiffEvent emission
```

### 5. macOS Sandbox (Seatbelt)
```
Legacy: seatbelt/ crate
    ├── sandbox-exec integration
    ├── Entitlements management
    └── macOS-specific restrictions

MMS: linux-sandbox/ only

NEEDED:
    ├── macos-sandbox/ crate
    ├── Seatbelt profile generation
    └── Cross-platform sandbox abstraction
```

### 6. MCP Server Mode
```
Legacy: codex-mcp-server/ crate
    ├── Act as MCP server
    ├── Expose tools via MCP protocol
    └── Handle MCP requests

MMS: mcp-client/ only (client mode)

NEEDED:
    ├── mcp-server/ crate
    ├── Tool exposure via MCP
    └── Server lifecycle management
```

---

## Implementation Priority Order

Based on flow analysis, recommended order:

1. **Command Safety** - Required for proper approval flow
2. **Skills System** - User-facing feature, high impact
3. **Turn Diff Tracker** - Enables undo, improves UX
4. **Task System** - Review/compact operations
5. **macOS Sandbox** - Platform parity
6. **MCP Server** - IDE integration support

Each builds on the previous, following the data flow patterns established in both systems.
