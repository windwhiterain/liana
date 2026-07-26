# AGENTS.md

## Build & Run

```bash
cargo build
cargo run
```

Standard Rust tooling — no special build system quirks.

## Architecture

Single-crate Rust project (edition 2024). The app is an eframe/egui GUI agent harness, **not** a CLI.

- **`src/lib.rs`** — real entrypoint: `App` implements `eframe::App`; wires config, LLM client, and current UI state.
- **`src/main.rs`** — stub placeholder (`println!("Hello, world!")`). The eframe entrypoint (`eframe::run_native`) is not wired yet.
- **`src/config.rs`** — loads config from `{config_dir}/liana/config.json` (via `dirs` crate). Fields: `base_url`, `api_key`, `model`. Panics if missing or unparseable.
- **`src/llm.rs`** — builds a `rig` OpenAI-compatible client using the Responses API (`GenericResponsesCompletionModel`). The LLM type is `rig::agent::AgentBuilder`.
- **`src/state.rs`** — `State` trait with a single `ui()` method. Screens implement this trait; currently only `Chat`.
- **`src/state/chat.rs`** — `Chat` state using `egui_probe` for UI introspection.
- **`src/memory.rs`** — tree-based memory manager with cache-hit-aware node placement (`find()`), memory selection prompts, and iterator utilities.

## Conventions

- `use` imports are explicit module paths (`use crate::config::Config`), not wildcards.
- Snake_case for struct fields — matches Drizzle-style conventions from the CLAUDE.md guidelines.
- `indoc` crate used for multi-line string constants (prompts).
- The `State` trait uses dynamic dispatch (`Box<dyn State>`) — new screens must implement `State`.

## Config

The app expects `liana/config.json` in the platform config directory:

```json
{
    "base_url": "https://api.openai.com/v1",
    "api_key": "sk-...",
    "model": "gpt-4o"
}
```

## Key Dependencies

- **`rig`** (`0.40.0`) — LLM framework, OpenAI Responses API provider
- **`eframe`** (`0.35.0`) — egui framework for native GUI
- **`egui-probe`** (`0.12.0`) — derive-macro UI introspection
- **`serde`** / **`serde_json`** — config parsing
- **`dirs`** — platform config directory resolution
- **`indoc`** — indented string literals for prompts
