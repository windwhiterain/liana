# liana

A bare-bones coding assistant. Sends your prompt to an LLM and prints the reply.

**You stay in control.** liana doesn't touch files, doesn't run commands, doesn't
have a plugin system. It's a REPL for your API provider — nothing more.

## Quick start

```bash
export LIANA_API_KEY="sk-..."
export LIANA_MODEL="gpt-4o"       # optional (default: gpt-4o)
export LIANA_BASE_URL="https://api.openai.com/v1"  # optional

cargo run
```

You'll see a `>>` prompt. Type your question.

## Single-shot mode

```bash
echo "write a fibonacci function in rust" | cargo run
```

## Environment

| Variable         | Required | Default                        |
|------------------|----------|--------------------------------|
| `LIANA_API_KEY`  | yes      | —                              |
| `LIANA_MODEL`    | no       | `gpt-4o`                       |
| `LIANA_BASE_URL` | no       | `https://api.openai.com/v1`    |

Works with any OpenAI-compatible API — OpenAI, DeepSeek, Ollama, LiteLLM, etc.

## Files

```
src/
  main.rs   — REPL loop, env config, entry point
  llm.rs    — OpenAI-compatible JSON API client (POST /chat/completions)
```

Each file is under 100 lines. Open them in any editor and read top-to-bottom.

## Design principles

- **One conversation turn = one HTTP call.** No tool loops, no state machine.
- **Environment variables, not config files.** A `Config` struct read from env at
  startup — nothing to parse.
- **Standard wire format.** If you can `curl` it, liana can talk to it.
- **No build-time codegen, no macros beyond derive.**
