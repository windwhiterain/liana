# Liana: Cache Aware Agent Memory Management Demo
most previous memory management didn't care about LLM cache hit, which is gradually more and more cheap because of KV cache。

this demo use a tree to orgnize memory, each memory block as a tree node. when a new task lunched, a set of memory blocks is selected that constrainted be loaded to context, this demo find the branch in the memory tree that satisfy the the memory block constraint, with minimul LLM API cost with cache hit awareness.

an example dialogue can be found [here](dialogue.txt).