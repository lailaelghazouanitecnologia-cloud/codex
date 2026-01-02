# Gap Analysis: MMS vs Legacy codex-rs

## Overview

| Metric | MMS | Legacy codex-rs |
|--------|-----|-----------------|
| **Crates/Modules** | 20 | 53 |
| **Lines of Code** | ~24,280 | ~80,000+ |
| **Status** | In Development | Production |

---

## Feature Comparison Matrix

### Core Agent Loop
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| Turn processing | ✅ processor.rs | ✅ core/turn.rs | None |
| Context management | ✅ context crate | ✅ core/context | None |
| Session management | ✅ session module | ✅ core/session | None |
| Approval system | ✅ approval.rs | ✅ core/approval | None |
| Retry logic | ✅ retry.rs | ✅ core/retry | None |
| Parallel execution | ✅ parallel.rs | ✅ core/parallel | None |
| Cancellation | ✅ cancel module | ✅ core/cancel | None |
| History/Rollout | ✅ history module | ✅ core/rollout | None |

### Tool System
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| Shell execution | ✅ shell crate | ✅ exec crate | None |
| File read | ✅ read_file.rs | ✅ tool handlers | None |
| File write | ✅ write_file.rs | ✅ tool handlers | None |
| File edit | ✅ edit_file.rs | ✅ tool handlers | None |
| Glob search | ✅ glob.rs | ✅ tool handlers | None |
| Grep search | ✅ grep.rs | ✅ tool handlers | None |
| Directory list | ✅ list_dir.rs | ✅ tool handlers | None |
| Background tasks | ✅ background.rs | ✅ exec crate | None |
| MCP handler | ✅ mcp_handler.rs | ✅ mcp integration | None |
| **Command safety analysis** | ❌ | ✅ is_dangerous_command | **MISSING** |
| **Parse command (lossy)** | ❌ | ✅ parse.rs | **MISSING** |

### Providers & LLM
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| Anthropic client | ✅ anthropic.rs | ✅ | None |
| OpenAI client | ✅ openai.rs | ✅ | None |
| Groq client | ✅ groq.rs | ❌ | MMS ahead |
| Ollama support | ⚠️ Config only | ✅ | Partial |
| Model registry | ✅ model_info.rs | ✅ | None |
| Streaming | ⚠️ Basic | ✅ Full | Partial |
| **Compact remote** | ❌ | ✅ OpenAI compaction | **MISSING** |

### Authentication
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| API key auth | ✅ | ✅ | None |
| OAuth flow | ✅ | ✅ | None |
| Token storage | ✅ file/keyring | ✅ | None |
| Multi-provider | ✅ | ✅ | None |

### Sandboxing
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| Landlock (Linux) | ✅ landlock.rs | ✅ | None |
| Seccomp (Linux) | ✅ seccomp.rs | ✅ | None |
| Policy engine | ✅ execpolicy | ✅ | None |
| Container support | ✅ container.rs | ✅ | None |
| **Seatbelt (macOS)** | ❌ | ✅ seatbelt crate | **MISSING** |

### Configuration
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| Config loading | ✅ loader.rs | ✅ config-loader | None |
| Feature flags | ✅ features.rs (50+) | ✅ | None |
| Provider config | ✅ | ✅ | None |
| **Multi-layer config** | ⚠️ Basic | ✅ LayeredConfig | Partial |
| **Project docs loading** | ❌ | ✅ ProjectDocLoader | **MISSING** |

### MCP (Model Context Protocol)
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| MCP types | ✅ mcp-types | ✅ | None |
| MCP client | ✅ mcp-client | ✅ | None |
| Tool discovery | ✅ | ✅ | None |
| Server lifecycle | ✅ manager.rs | ✅ | None |
| **MCP server mode** | ❌ | ✅ codex-mcp-server | **MISSING** |

### Git Integration
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| Repo detection | ✅ | ✅ | None |
| Branch/commit info | ✅ | ✅ | None |
| Diff tracking | ✅ | ✅ | None |
| Remote management | ✅ | ✅ | None |
| **Turn diff tracker** | ❌ | ✅ TurnDiffTracker | **MISSING** |

### User Interface
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| CLI commands | ✅ 8 commands | ✅ | None |
| **TUI (ratatui)** | ❌ | ✅ tui crate | **MISSING** |
| **Rendering utils** | ❌ | ✅ tui/render | **MISSING** |
| **App server (IDE)** | ❌ | ✅ codex-exec-server | **MISSING** |

### Skills System
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| **Skill trait/registry** | ⚠️ Stub only | ✅ skills crate | Partial |
| **Slash commands** | ❌ | ✅ SkillLoader | **MISSING** |
| **Skill injection** | ❌ | ✅ skill_injection | **MISSING** |
| **Built-in skills** | ❌ | ✅ /help, /review, etc. | **MISSING** |

### Task System
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| **Review task** | ❌ | ✅ ReviewTask | **MISSING** |
| **Compact task** | ❌ | ✅ CompactTask | **MISSING** |
| **Ghost snapshot** | ❌ | ✅ GhostSnapshot | **MISSING** |
| **Undo task** | ❌ | ✅ UndoTask | **MISSING** |
| **Cloud tasks** | ❌ | ✅ CloudTask | **MISSING** |

### Prompts & Context
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| System prompts | ✅ | ✅ | None |
| Context window | ✅ truncation.rs | ✅ | None |
| Environment context | ✅ environment.rs | ✅ | None |
| **Custom prompts** | ❌ | ✅ CustomPrompts | **MISSING** |
| **Review prompts** | ❌ | ✅ ReviewPrompts | **MISSING** |

### Embedding & Search
| Feature | MMS | Legacy | Gap |
|---------|-----|--------|-----|
| File search | ✅ file-search | ✅ | None |
| **Embedding client** | ⚠️ Stub only | ✅ | Partial |
| **Vector store** | ⚠️ Stub only | ✅ | Partial |

---

## Priority Implementation List

### P0 - Critical (Core Functionality Gaps)

1. **Skills System** - `/home/user/codex/mms/skills/`
   - Implement SkillLoader for slash command files (.md)
   - Add skill injection for context
   - Built-in skills: /help, /review, /compact
   - ~1,500 lines estimated

2. **Command Safety Analysis** - `/home/user/codex/mms/shell/`
   - `is_dangerous_command()` detection
   - `is_safe_command()` whitelist
   - Command classification for approval
   - ~500 lines estimated

3. **TUI Interface** - New `tui` crate
   - ratatui-based interactive mode
   - Streaming response display
   - Input handling and history
   - ~3,000 lines estimated

### P1 - High Priority (Important Features)

4. **Task System** - New `tasks` crate
   - ReviewTask for code review
   - CompactTask for context compression
   - UndoTask for reverting changes
   - GhostSnapshot for state capture
   - ~2,000 lines estimated

5. **Turn Diff Tracker** - `/home/user/codex/mms/git/`
   - Track file changes per turn
   - Enable undo functionality
   - Diff visualization
   - ~600 lines estimated

6. **Seatbelt Sandbox (macOS)** - New `macos-sandbox` crate
   - macOS sandbox-exec integration
   - Entitlements management
   - ~800 lines estimated

### P2 - Medium Priority (Enhanced Functionality)

7. **MCP Server Mode** - New `mcp-server` crate
   - Expose agent as MCP server
   - Tool registration
   - Resource serving
   - ~1,000 lines estimated

8. **App Server (IDE)** - New `app-server` crate
   - HTTP/WebSocket server
   - VS Code extension support
   - JetBrains extension support
   - ~2,500 lines estimated

9. **Compact Remote** - `/home/user/codex/mms/providers/`
   - OpenAI-based context compaction
   - Summarization for long contexts
   - ~400 lines estimated

10. **Custom/Review Prompts** - `/home/user/codex/mms/core/`
    - User-defined prompt templates
    - Review-specific prompts
    - ~500 lines estimated

### P3 - Low Priority (Nice to Have)

11. **Multi-layer Config** - `/home/user/codex/mms/config/`
    - Global → Project → Local layering
    - Override precedence
    - ~400 lines estimated

12. **Project Doc Loading** - `/home/user/codex/mms/context/`
    - README, CONTRIBUTING detection
    - Auto-context loading
    - ~300 lines estimated

13. **Embedding System** - `/home/user/codex/mms/embedding/`
    - Complete EmbeddingClient
    - VectorStore implementation
    - ~800 lines estimated

14. **Parse Command (Lossy)** - `/home/user/codex/mms/shell/`
    - Detailed command parsing
    - Safe lossy summarization
    - ~400 lines estimated

---

## Estimated Effort Summary

| Priority | Items | Estimated Lines | Complexity |
|----------|-------|-----------------|------------|
| P0 | 3 | ~5,000 | High |
| P1 | 3 | ~3,400 | Medium-High |
| P2 | 4 | ~4,400 | Medium |
| P3 | 4 | ~1,900 | Low-Medium |
| **Total** | **14** | **~14,700** | |

---

## What MMS Has That Legacy Doesn't

1. **Groq Provider** - Full client implementation
2. **Cleaner Error System** - 40+ categorized error types with classification
3. **More Feature Flags** - 50+ flags vs ~20 in legacy
4. **Modern Model Registry** - Comprehensive model capabilities/pricing
5. **Better Structured Auth** - 4 provider types with storage backends

---

## Recommended Implementation Order

1. **Week 1-2**: Skills system (P0) - Critical for user experience
2. **Week 3**: Command safety + Turn diff tracker (P0/P1)
3. **Week 4-5**: TUI interface (P0) - Essential for interactive use
4. **Week 6**: Task system (P1) - Review, compact, undo
5. **Week 7**: Seatbelt + MCP server (P1/P2)
6. **Week 8+**: App server, remaining P2/P3 items

---

## Conclusion

MMS is approximately **65-70% feature complete** compared to legacy codex-rs. The core agent loop, tool system, providers, auth, and sandboxing are solid. The main gaps are:

1. **Skills/slash commands** - User-facing commands
2. **TUI** - Interactive terminal interface
3. **Task system** - Review, compact, undo operations
4. **macOS sandbox** - Seatbelt support
5. **IDE integration** - App server for extensions

The foundation is strong and well-architected. Remaining work is primarily in user-facing features and platform-specific enhancements.
