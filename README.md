# Liana
An opinioned agent harness, exporing new things.

## Explorations

### Cache-aware memory management
This is why this project is born.

Long-running LLM agent conversations face a tension between **composability** (arbitrarily selecting subsets of past interactions as context) and **stability** (presenting the LLM with a deterministic, linearly-ordered prompt regardless of which memories are selected). Existing approaches like MemGPT (Packer et al., 2023) and MemTree (Rezazadeh et al., 2024) address memory hierarchy and tree-structured summarization, but none directly solves the combined problem. Liana resolves this by organizing conversation history as a tree of *Memory* segments (raw messages + compressed summary) and modeling context construction as a **cache-cost minimization problem**: selected memories treat their descendant subtrees as cached (zero cost), while non-descendant nodes incur a penalty proportional to their token footprint. The `find()` algorithm identifies the minimum-cost anchor node, then walks the parent chain upward to assemble a deterministic, chronologically-grounded context — the selection of memories only determines *where* uncached segments attach, never the order of the backbone.

## Philosophy
- No LLM contribution without human polishment.
- Less prompt is more.

## User Guide

1. Create `{config_dir}/liana/config.json` with your LLM credentials (`base_url`, `api_key`, `model`).
2. Launch the app. The sidebar switches between **Chat** (conversation) and **Recall** (memory browser).
3. In Chat, switch to **Summary** mode and press run to persist the session into memory.
4. In Recall, use **LLM** mode to get suggested memories, or **Confirm** mode to pick manually — either way, you resume Chat with full context.