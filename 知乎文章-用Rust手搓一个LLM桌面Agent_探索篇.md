# 上下文管理：可组合性和缓存命中可以兼得

> 由 D 指导创作

长对话 Agent 面临一个被反复讨论但始终悬而未决的矛盾。

一方面，你希望能灵活选择任意一段历史对话作为上下文——这要求记忆系统具备**可组合性**（composability）。另一方面，LLM 对 prompt 的线性顺序极其敏感，一旦打乱，就会出现 Liu et al.（2024）所描述的"lost in the middle"退化——这要求上下文**稳定**（stable），每次喂给模型的 prompt 必须在内容和排序上都是确定性的。

MemGPT（Packer et al., 2023）通过分页式虚拟内存机制来扩展上下文容量，MemTree（Rezazadeh et al., 2024）用树结构组织层级摘要。它们各自解决了记忆管理的一部分问题，但都没有正面回答一个更根本的问题：**如何从任意选择的记忆子集中，生成一段确定性的、线性排序的上下文。** MemGPT 把"该回顾什么"的决策外包给 LLM 自己的 function calling，MemTree 把遍历路径留给交互式导航——两者都在回避这个结构性问题。

[Liana](https://github.com/windwhiterain/liana) 的解法不同。它把这个问题建模为**缓存代价最小化**。

先看数据结构。会话历史被组织成一棵记忆树，每个 *Memory* 节点存储两类信息：原始对话消息和 LLM 生成的压缩摘要。关键之处在于，摘要和原始消息以穿插的方式存入 prompt：

每步遍历之间会插回 U→A→U→A 这样的实际对话轮次，让 LLM 就像亲历过那段对话一样重建上下文。来看 `messages()` 的核心逻辑：

```rust
pub fn messages(&self, node: Option<NodeId>) -> impl Iterator<Item = Message> {
    let memories = MemoryIterator { manager: self, node }
        .collect::<Vec<_>>();
    memories.into_iter().rev().flat_map(|x| {
        x.messages.iter().cloned().chain([
            Message::User {
                content: OneOrMany::one(UserContent::Text(Text::new(
                    "summary our chat since the previous summary",
                ))),
            },
            Message::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::Text(Text::new(
                    x.summary.clone(),
                ))),
            },
        ])
    })
}
```

父链自底向上遍历，每个 Memory 节点先展开原始消息再附上摘要。不管从哪个锚点出发，**线性上下文的主干顺序始终不变**。

然后是 `find()` 算法。当用户勾选了一组记忆（比如选了 D 和 C），系统需要决定把这些记忆挂在树的哪个位置来生成最终上下文。`find()` 的做法：

1. 对于每个被选中的记忆，它的**后代子树被视为"已缓存"**——零代价
2. 不在选中记忆子树中的节点承担**"缓存未命中"惩罚**——代价 = 节点 token 数 × 惩罚系数（默认 64）
3. 遍历整棵树，寻找**总代价最小的节点**作为锚点

```rust
for memory in memories.iter().copied() {
    let memory = &self.memories[memory];
    let mut mask = vec![true; self.nodes.len()];
    for node in memory.nodes.iter().copied() {
        let mut nodes = vec![node];
        while let Some(node) = nodes.pop() {
            mask[node.0] = false;
            nodes.extend(self.nodes[node.0].children.iter());
        }
    }
    for node in 0..self.nodes.len() {
        if !mask[node] { continue; }
        costs[node] += memory.size * 64;
    }
}
```

选定锚点后，所有未被缓存的记忆作为子节点挂载到锚点之下，然后从锚点出发沿父链组装上下文。整个流程还顺手计算了**理论缓存命中率**和**记忆稀疏度**两个指标：

```rust
let (theory_cache, memory_sparsity) = if let Some(node_id) = node_id {
    let node = &self.nodes[node_id.0];
    (
        Some((cached_size as f64 / node.size as f64) * 100.0),
        Some((node.size as f64 / self.size as f64) * 100.0),
    )
} else {
    (None, None)
};
```

- 理论缓存命中率 = 已缓存 token 数 / 锚点总 token 数，反映**记忆选择效率**
- 记忆稀疏度 = 锚点总 token 数 / 全树总 token 数，反映**上下文密度**

最终的效果是：**可组合性和稳定性同时成立。** 记忆可以被附加到任意分支，形成对对话主题结构的有向无环表示；而 LLM 始终收到确定性的、按时间线排列的上下文序列，不会有"lost in the middle"。树作为共享表征，将"发生了什么的结构"和"回忆什么的选择"完全解耦了。

这个设计在工程上的代价很低——不需要额外的向量数据库，不需要 embedding，不需要函数调用，纯树 + 代价函数就完成了。笔者认为这比 MemGPT 那种"让 LLM 自己管理自己的记忆"的方案更可控，也适合作为 Agent 记忆管理的一个可复现的 baseline。
