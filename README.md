# Liana
An opinioned agent harness, exporing new things.

## Explorations

### Cache-aware Memory Composition
This is why this project is born.

Agent memory management has two extremes:

||composabe|stable|
|-|-|-|
|llm cache|low|high|
|context sparsity|high|low|

Liana use a unique method to balance them:

||liana|
|-|-|
|llm cache|memory tree, each node is a memory|
|related memories|constraints on tree node|
|stable memory order|ancestor chain of tree node|

Memory composition pipline:
- select relative memories.
- find optimal tree node.
  - for each node:
    - find relative memories that not inside ancestor chain.
    - compute cost for each tree node:
    
      `cost(node) = cache_hit_price * ancestor_chain(node).size + cache_miss_price * missing_memories(node).size`
- use the ancestor chain of the tree node as context.

## Philosophy
- No LLM contribution without human polishment.
- Less prompt is more.

## User Guide

1. Create `{config_dir}/liana/config.json` with your LLM credentials (`base_url`, `api_key`, `model`).
2. Launch the app. The sidebar switches between **Chat** (conversation) and **Recall** (memory browser).
3. In Chat, switch to **Summary** mode and press run to add the session into memory management.
4. In Recall, you can select memories needed,  use **LLM** mode to get suggested memories, use **Confirm** mode to lauch a session with this memories to **Chat**.