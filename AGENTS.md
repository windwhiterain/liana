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

### State co-existence (the core pattern)

```
App.states: HashMap<TypeId, Box<dyn ErasedState>>      ← all states live simultaneously
App.active: TypeId                                      ← only one rendered
              ↓
Erased<S>::update_erased(Message, data)                 ← downcasts Message→S::Message, delegates to S::update
              ↓
S::update(Self::Message, data) → (Task, Option<TypeId>) ← transition returns target TypeId
```

Three traits in `src/state.rs`:

- **`State`** (non-object-safe): `type Message: Any + Send + Into<Message>`. Methods: `update()`, `view()`, `handle_stream_response()` — all take `data: &mut Data`/`&Data`. Has `fn type_id() -> TypeId` with default impl `TypeId::of::<Self>()`.
- **`ErasedState`** (object-safe): `update_erased()` / `view_erased()` / `handle_stream_response()`. All take `data: &mut Data`/`&Data`, return `Option<TypeId>` for transitions.
- **`Erased<S>`** adapter: wraps a typed `State`, bridges `Self::Message` ↔ `Message` via `downcast()` / `Into`.

### `Data` struct (`src/lib.rs`)

Shared mutable context for all states. Replaces the old individual `&Arc<LLM>` + `&mut memory::Manager` params:

```rust
pub struct Data {
    pub llm: Arc<LLM>,
    pub llm_json: Arc<LLM>,
    pub memory_manager: memory::Manager,
    pub parent_memory: Option<memory::NodeId>,    // transition param (Recall → Chat)
    pub messages: Vec<RigMessage>,                // conversation history
    pub markdown_states: Vec<Vec<markdown::Item>>,
    pub reasoning_markdown_states: Vec<Option<Vec<markdown::Item>>>,
    pub memory_markdowns: Vec<Vec<markdown::Item>>, // mirrors memory_manager.memories
}
```

`messages`/`markdown_states`/`reasoning_markdown_states` are in `Data` so Recall can clear them on transition. `memory_markdowns` is in `Data` because Chat's Summary mode adds memories while Recall displays them — both need the same markdown cache.

### Message type

```rust
pub type Message = Box<dyn Any + Send>;   // NOT an enum — type-erased, no central variant list
```

iced 0.14 requires `Message: Send + 'static` — no `Debug` or `Clone`.

### Sidebar

```rust
pub struct SidebarEntry {
    pub label: &'static str,
    pub type_id: TypeId,           // maps to HashMap key
}

pub struct SidebarNav(pub TypeId); // produced by sidebar button clicks
```

No `factory` — states are created upfront in `init()` and persist.

### Message dispatch order in `App::update`

```
StreamIntent → StreamResponse → SidebarNav → active state (update_erased)
```

### LLM streaming service (App-level)

States emit `StreamIntent { owner: TypeId, task_id: u64, ... }` with `owner` set via `Self::type_id()`. App intercepts and spawns streaming. `StreamResponse` mirrors the `owner` back.

**Routing**: `StreamResponse` is routed to `app.states[resp.owner]` — NOT `app.active`. This ensures responses always reach the spawner state even if the user switched sidebar mid-stream.

### `From` impls (`src/state.rs`)

`From<ChatMessage>` and `From<RecallMessage>` extract `StreamIntent` variants so App can intercept:

```rust
chat::ChatMessage::StreamIntent(intent) => Box::new(intent),  // interceptible
other => Box::new(other),                                      // routed to state
```

### State files

| File | Role |
|---|---|
| `src/state/chat.rs` | `Chat` state: UI state only (busy, streaming, config). Conversation data lives in `Data`. Message + Summary modes; reasoning as collapsible sections. |
| `src/state/recall.rs` | `Recall` state: checkbox memory list. `Config::LLM` → streams for memory selection, parses JSON on Done. `Config::Confirm` → sets `data.parent_memory`, clears `data.messages`, returns `Some(TypeId::of::<Chat>())`. |

### Shared modules

| File | Role |
|---|---|
| `src/stream.rs` | `stream_prompt()` — tokio::spawn + mpsc. `ReceiverStream<T>` — `UnboundedReceiver` → `futures::Stream`. `StreamMsg` enum. |

## Key conventions

- **Iced 0.14**: free functions (`init`, `update`, `view`). `Command` → `Task`.
- **`view()` signature**: `fn view<'a>(&'a self, data: &'a Data) -> Element<'a, Self::Message>` — named `'a` lifetime required because `Element` borrows from both `self` and `data`.
- **Markdown**: `iced::widget::markdown` (enabled via `features = ["markdown"]`). `markdown::parse(text).collect()` → `Vec<markdown::Item>`, rendered via `markdown::view(&items, Theme::Dark)`. Link clicks produce a `Uri` message — add a `LinkClicked(String)` variant to state messages.
- **Streaming markdown**: re-parse accumulated text on each `Text` chunk: `self.streaming_md = markdown::parse(&self.streaming_text).collect()`. Final response also uses `markdown::view`.
- **LLM**: `rig::agent::Agent<GenericCompletionModel>` in `Arc`. Streaming via `StreamingChat` trait, **not** `Prompt` (batch). Also handle `FinalResponse` fallback for short responses.
- **`probe::TextEditor`** = `iced::widget::text_editor::Content`. Use `.text()` to read, `.perform(action)` for edits.
- **`#[derive(Probe)]`** from `liana-probe` auto-generates a `{TypeName}ProbeMsg` type.
- **Config**: `{config_dir}/liana/config.json` (platform dir via `dirs`), deserialized at startup, panics if missing.
- **Chat's `Busy`**: enum `{ Idle, Message, Summary }`. `Recall` uses `busy: bool`.
- **Reasoning**: embedded as `AssistantContent::Reasoning` in the same `Message::Assistant` alongside text, combined via `OneOrMany::many`. View renders as collapsible sections.

## Memory system (`src/memory.rs`)

UI-agnostic (no iced imports). Key API:

- `Manager::messages(parent)` → `impl Iterator<Item = Message>` — walks tree upward
- `Manager::add_memory(Memory, parent)` → `MemoryId`
- `Manager::find(&[usize])` → `(Option<NodeId>, Option<f64>, Option<f64>)`
- `Manager::memories: Vec<Memory>` (pub) — direct access
- Prompts: `SELECT_MEMORY_PROMPT`, `SUMMARY_PROMPT`, `LIST_MEMORY_PROMPT`

## Dependencies

| Crate | Purpose |
|---|---|
| `iced` 0.14 (`features = ["markdown"]`) | GUI + native markdown rendering |
| `rig` 0.40.0 | LLM client (OpenAI streaming) |
| `rig-memory` 0.40.0 | Token counting |
| `liana-probe` (path) | Proc-macro widget bindings |
| `futures` 0.3 | Stream combinators |
| `tokio` 1.53 | Async runtime (multi-thread) |
| `serde`/`serde_json` | Config parsing |
