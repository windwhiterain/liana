# Liana
An opinioned agent harness, exporing new things.

## Explorations

### Cache-aware memory management
This is why this project is born.

Long-running LLM agent conversations face a structural tension between two competing requirements: **composability** — the ability to construct context by selecting and combining arbitrary subsets of past interactions — and **stability** — ensuring that the linearized prompt presented to the LLM is deterministic in both content and ordering, regardless of which memories are selected. This tension is well-documented: approaches like MemGPT (Packer et al., 2023) address the memory problem through hierarchical paging between tiers, while MemTree (Rezazadeh et al., 2024) and MemWalker (Chen et al., 2023) organize memory into tree-structured representations for hierarchical summarization and interactive navigation. However, none directly solves the problem of serving a deterministic, linearly-ordered context from an arbitrarily composable selection of memory nodes.

Our system resolves this by maintaining conversation history as a tree of *Memory* segments, each storing raw messages alongside a compressed summary. When a subset of memories is selected for context construction, a `find()` algorithm operates on the tree as a cache-cost minimization problem: for each selected memory, its descendant subtree is treated as already cached (incurring zero marginal cost), while all non-descendant nodes are penalized with a cache-miss cost proportional to their token footprint. The algorithm identifies the node that minimizes total cost and attaches any uncached memories as children beneath it. Context is then assembled by walking the parent chain upward from the anchor node, interleaving summary-assisted exchanges at each step. Critically, this parent-chain traversal yields the same linearized prompt regardless of which memories were selected — the selection only determines *where* in the tree uncached memories are attached, not the ordering of the backbone. This design echoes the KV-cache reuse insights from systems like AttentionStore (Gao et al., 2024), where cache-miss penalties dominate inference cost, and draws on the hierarchical schema concept from MemTree to maintain a single consistent tree structure that grows with the conversation.

The result is a memory subsystem that is simultaneously **composable** — memories can be attached to any branch, forming a directed acyclic representation of the conversation's topical structure — and **stable** — the LLM always receives a deterministic, chronologically-grounded context sequence, eliminating the "lost in the middle" degradation (Liu et al., 2024) that arises from ad-hoc context assembly. The tree serves as a shared representation that decouples the *structure* of what happened (the conversation tree) from the *selection* of what to recall (the subset of memories), resolving the fundamental tension between flexible retrieval and consistent prompting.

## Philosophy
- No LLM contribution without human polishment.
- Less prompt is more.

## User Guide

1. Create `{config_dir}/liana/config.json` with your LLM credentials (`base_url`, `api_key`, `model`).
2. Launch the app. The sidebar switches between **Chat** (conversation) and **Recall** (memory browser).
3. In Chat, switch to **Summary** mode and press run to persist the session into memory.
4. In Recall, use **LLM** mode to get suggested memories, or **Confirm** mode to pick manually — either way, you resume Chat with full context.