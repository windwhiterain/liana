use std::{collections::HashSet, fmt::Display};

use crate::api::Message;
use indoc::indoc;

pub const SELECT_MEMORY_PROMPT: &'static str = indoc!(
    "
    given the last question I ask or task I assign to you, do not answer, please filter and select strongly relative memories listed bellow, from high-relativity to low, only select nessesary memory.
    if you found that the paragraph is talking something you are confused or seems lack some background, its because the related memories were wipped out from you, you must aggressively guess and pick related memories from the list.
    if the user change topic, just return empty array [].
    only output an json array of indices of your selected memories, example: [3,0,8,4].
    "
);

pub const SYSTEM_PROMPT: &'static str = indoc!(
    "
    you are AI assistant \"Liana\".
    when user ask question or assign task: give comprehensive, in-depth thought and answer.
    when user ask to summarize: be brief and at the point.
    when user shows a question or task then let you select relative memories listed: think quick and output json.
    "
);

pub const MEMORY_DESCRIBE_PROMPT: &'static str = indoc! {
    "
    use a brief sentence to summarize the topic of my last question or task I given you and the your last answer or respond you given me. 
    the summarization must use the same language as the dialogue to be summarized.
    "
};

#[derive(Debug)]
pub struct Memory {
    pub messages: Vec<Message>,
    pub summary: String,
    pub size: usize,
    pub nodes: Vec<NodeId>,
}

impl Memory {
    pub fn new(messages: Vec<Message>, summary: String, size: usize) -> Self {
        Self {
            messages,
            summary,
            size,
            nodes: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct Manager {
    pub memories: Vec<Memory>,
    pub nodes: Vec<Node>,
    pub last_memory: Option<MemoryId>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            memories: Default::default(),
            nodes: Default::default(),
            last_memory: None,
        }
    }
    pub fn last_node(&self) -> Option<NodeId> {
        let Some(memory) = self.last_memory else {
            return None;
        };
        let memory = &self.memories[memory.0];
        memory
            .nodes
            .iter()
            .copied()
            .max_by_key(|x| self.nodes[x.0].size)
    }
    pub fn messages(&self, node: Option<NodeId>) -> impl IntoIterator<Item = &Message> {
        let memories = MemoryIterator {
            manager: self,
            node,
        }
        .collect::<Vec<_>>();
        memories.into_iter().rev().flat_map(|x| x.messages.iter())
    }
    pub fn display_memories(&self) -> impl Display {
        DisplayMemories(self.memories.iter().enumerate())
    }
    pub fn add_memory(&mut self, mut memory: Memory, parent: Option<NodeId>) -> MemoryId {
        let memory_id = MemoryId(self.memories.len());
        let node_id = NodeId(self.nodes.len());
        memory.nodes.push(node_id);
        self.memories.push(memory);
        self.add_node(memory_id, parent);
        memory_id
    }
    pub fn find(&mut self, memories: &[usize]) -> Option<NodeId> {
        if memories.is_empty() {
            return None;
        }
        let cache_miss_cost_scale = 64usize;
        let mut min_cost = memories
            .iter()
            .copied()
            .map(|x| self.memories[x].size)
            .sum::<usize>()
            * cache_miss_cost_scale;
        let mut min_node: Option<NodeId> = None;
        let mut costs: Vec<usize> = self.nodes.iter().map(|x| x.size).collect();
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
                if !mask[node] {
                    continue;
                }
                costs[node] += memory.size * cache_miss_cost_scale;
            }
        }
        for (node, cost) in costs.iter().copied().enumerate() {
            let node = NodeId(node);
            if cost < min_cost {
                min_cost = cost;
                min_node = Some(node);
            }
        }
        // println!("min_node: {:#?}", min_node);

        let mut cached_memories = HashSet::new();
        if let Some(mut node_id) = min_node {
            loop {
                let node = &self.nodes[node_id.0];
                cached_memories.insert(node.memory);
                let Some(parent) = node.parent else {
                    break;
                };
                node_id = parent;
            }
        }
        // println!("cached_memories: {:#?}", cached_memories);
        let mut node_id = min_node;
        for memory in memories.iter().copied() {
            let memory = MemoryId(memory);
            if !cached_memories.contains(&memory) {
                node_id = Some(self.add_node(memory, node_id));
            }
        }
        node_id
    }
    fn add_node(&mut self, memory_id: MemoryId, parent: Option<NodeId>) -> NodeId {
        let memory = &self.memories[memory_id.0];
        let id = NodeId(self.nodes.len());
        let size = if let Some(parent) = parent {
            let parent = &mut self.nodes[parent.0];
            parent.children.push(id);
            parent.size
        } else {
            0
        } + memory.size;
        self.nodes.push(Node {
            memory: memory_id,
            parent,
            children: Default::default(),
            size,
        });
        id
    }
}

#[derive(Debug)]
pub struct Node {
    pub memory: MemoryId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub size: usize,
}

pub struct MemoryIterator<'a> {
    pub manager: &'a Manager,
    pub node: Option<NodeId>,
}

impl<'a> Iterator for MemoryIterator<'a> {
    type Item = &'a Memory;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(node) = self.node else { return None };
        let node = &self.manager.nodes[node.0];
        let ret = &self.manager.memories[node.memory.0];
        self.node = node.parent;
        Some(ret)
    }
}

pub struct DisplayMemories<'a, T: IntoIterator<Item = (usize, &'a Memory)> + Clone>(pub T);

impl<'a, T: IntoIterator<Item = (usize, &'a Memory)> + Clone> Display for DisplayMemories<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (id, memory) in self.0.clone().into_iter() {
            writeln!(f, "{id}. {}", memory.summary)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryId(usize);
