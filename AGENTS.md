# AGENTS.md

## Quick check

```bash
cargo check        # compiles liana + liana-probe proc-macro (NOT a workspace — single crate + path dep)
```

Edition 2024. Standard Cargo project.

## Architecture

### Crate structure

| Crate | Role |
|---|---|
| `liana` (root) | iced 0.14 desktop GUI. Entrypoint: `src/main.rs` |
| `liana-probe` (path dep) | Proc-macro crate. `#[derive(Probe)]` generates widget bindings |

### State machine (the core pattern)

```
App.state: Box<dyn ErasedState>      ← type-erased, swappable at runtime
         ↓
Erased<S>::update_erased(Message)    ← downcasts Message→S::Message, delegates to S::update
         ↓
S::update(Self::Message) → (Task<Self::Message>, Option<Box<dyn ErasedState>>)
                                                                   ↑
                                          Some(state) triggers state swap in App::update
```

Two traits:

- **`State`** (non-object-safe, `src/lib.rs`): `type Message: Any + Send + Into<Message>` + `update()` / `view()`. Each state works with its own message type.
- **`ErasedState`** (object-safe): `update_erased(Message)` / `view_erased()`. What `App` actually holds as `Box<dyn ErasedState>`.
- **`Erased<S>`** adapter: wraps a typed `State`, bridges `Self::Message` ↔ `Message` via `downcast()` / `Into`.

### Message type

```rust
pub type Message = Box<dyn Any + Send>;   // NOT an enum — type-erased, no central variant list
```

iced 0.14 requires `Message: Send + 'static` — no `Debug` or `Clone`. This lets any crate add states without modifying `lib.rs`.

### Sidebar

`App.sidebar: Vec<SidebarEntry>` — each entry has a `label` and a `factory: Box<dyn Fn(&App) -> Box<dyn ErasedState>>`.

`update()` first checks `message.downcast::<SidebarNav>()`; if matched, calls the factory with `&App` (fresh data) and swaps `app.state`. Otherwise, routes the message to the active state.

### State files

| File | Role |
|---|---|
| `src/state/chat.rs` | `Chat` state: main chat UI with message/summary modes. Splits responses into `ChatResponse` (append) and `SummaryResponse` (creates `Memory` via `manager.add_memory()`, takes all messages, clears state). `parent_memory` tracks the tree node for context reconstruction. |
| `src/state/recall.rs` | `Recall` state: checkbox memory list from `memory_manager.memories`. `Config::LLM` mode: LLM selects indices → sets checkboxes (stays in Recall for review). `Config::Confirm` mode: collects checked → `find()` → switches to `Chat`. |

### Memory system (`src/memory.rs`)

UI-agnostic (no iced imports). Key API:

- `Manager::display_memories()` → `impl Display` (lists `<index>. <summary>`)
- `Manager::messages(parent: Option<NodeId>)` → `impl Iterator<Item = Message>` — reconstructs conversation context by walking the tree upward
- `Manager::add_memory(Memory, parent)` → `MemoryId` — persists messages+summary into the tree
- `Manager::find(&[usize])` → `(Option<NodeId>, Option<f64>, Option<f64>)` — finds optimal tree node for selected memory indices
- `Manager::memories: Vec<Memory>` (pub) — direct access for display
- Prompts: `SELECT_MEMORY_PROMPT`, `SUMMARY_PROMPT`

## Key conventions

- **Iced 0.14**: free functions (`init`, `update`, `view`), not the old `Application` trait. `Command` → `Task`.
- **`view()` signature**: `fn view<'a>(&'a self, memory_manager: &'a memory::Manager) -> Element<'a, Self::Message>` — the named `'a` lifetime is **required** because the returned `Element` may borrow from both `self` and `memory_manager`.
- **LLM** is `Arc<LLM>` (cloneable across async tasks). `Prompt` trait must be in scope to call `.prompt()` on it.
- **Markdown**: `frostmark` — `Vec<MarkState>` parallel to `Vec<Message>`, rendered via `MarkWidget`.
- **`probe::TextEditor`** = `iced::widget::text_editor::Content`. Use `.text()` to read, `.perform(action)` for edits.
- **`#[derive(Probe)]`** from `liana-probe` auto-generates a `{TypeName}ProbeMsg` type (empty enum as "never" type). References `crate::probe::*` — assumes use from within the `liana` crate.
- **Config**: `{config_dir}/liana/config.json` (platform dir via `dirs`), deserialized at startup, panics if missing.

## Dependencies

| Crate | Purpose |
|---|---|
| `iced` 0.14 | GUI framework |
| `frostmark` 0.3 | Markdown rendering for iced |
| `rig` 0.40.0 | LLM client |
| `rig-memory` 0.40.0 | Token counting |
| `liana-probe` (path) | Proc-macro for auto-widget generation |
| `serde`/`serde_json` | Config parsing |
| `tokio` 1.53 | Async runtime (multi-thread) |
