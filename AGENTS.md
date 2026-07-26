# AGENTS.md

## Quick check

```bash
cargo check        # compiles main + liana-probe proc-macro
```

No special build system — standard Cargo workspace. Edition 2024.

## Architecture

Two crates: `liana` (app) + `liana-probe` (proc-macro). The app is an **iced 0.14** desktop GUI, not a CLI.

### `liana` crate

| File | Role |
|---|---|
| `src/main.rs` | Bootstraps `iced::application(init, update, view)` — no trait impl |
| `src/lib.rs` | `App` struct, `Message` enum, `init()`/`update()`/`view()` free functions |
| `src/config.rs` | `Config` deserialized from `{config_dir}/liana/config.json` (panics on missing) |
| `src/llm.rs` | `LLM` type alias for `rig::agent::Agent<GenericCompletionModel>`. Factory: `llm_from_config(&Config)` |
| `src/memory.rs` | Tree-based memory manager. UI-agnostic — no iced imports |
| `src/state/chat.rs` | `Chat` struct + `ChatMessage` enum. `Probe` derives on `Config` and `MessageConfig` |
| `src/probe.rs` | `Probe` trait, `ProbeMsg<Child>`, generic `probe_view()`/`probe_update()`. Type aliases: `TextEditor`, `TextEditorAction` |

### `liana-probe` crate (proc-macro)

`#[derive(Probe)]` generates `describe()`, `field_value()`, `apply()`, `variants()`, `current_variant_index()` on a type. Supports:

- Structs with named fields: infers `FieldKind` from type (`String`→String, `bool`→Bool, `probe::TextEditor`→Multiline)
- Enums with tuple variants: delegates `field_value`/`apply` to inner probe, generates `SelectVariant` for switching
- Per-field attrs: `#[probe(hide_label)]`, `#[probe(label = "...")]`, `#[probe(kind = "...")]`
- Enum attrs: `#[probe(tags = "inlined")]` (renders variant tabs)

References paths via `crate::probe::*` within the `liana` crate (proc-macro assumes it's used from liana).

## Key conventions

- Iced 0.14 uses **free functions** (`init`, `update`, `view`), not the old `Application` trait
- `Command` is `Task` in iced 0.14. Async: `Task::perform(future, Msg::Response)`
- LLM is wrapped in `Arc<LLM>` for cloneability across async tasks
- Markdown rendering uses `frostmark` (`MarkState` + `MarkWidget`), stored as `Vec<MarkState>` parallel to `Vec<Message>`
- `probe::TextEditor = iced::widget::text_editor::Content` — a type alias. Use `.text()` to read, `.perform(action)` for edits, `.new()` to create empty
- The `Probe` trait uses lifetime-annotated `FieldValue<'a>` for borrow-safe rendering of `text_editor` widgets
- Module structure: `pub mod` declarations in `lib.rs`. No wildcard imports

## Config

`{config_dir}/liana/config.json` (platform config dir via `dirs` crate):

```json
{
    "base_url": "https://api.openai.com/v1",
    "api_key": "sk-...",
    "model": "gpt-4o"
}
```

## Dependencies

| Crate | Version | Purpose |
|---|---|---|
| `iced` | 0.14 | GUI framework |
| `frostmark` | 0.3 | Markdown widget for iced (comrak-based) |
| `rig` | 0.40.0 | LLM client (OpenAI Responses API) |
| `rig-memory` | 0.40.0 | Token counting |
| `liana-probe` | path dep | Derive macro for auto-widget generation |
| `indoc` | 2 | Multi-line string literals for prompts |
| `serde`/`serde_json` | 1 | Config parsing |
| `tokio` | 1.53 | Async runtime (multi-thread) |
