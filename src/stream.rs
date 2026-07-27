use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::stream::Stream;
use futures::StreamExt;
use rig::agent::MultiTurnStreamItem;
use rig::streaming::{StreamedAssistantContent, StreamingChat};
use tokio::sync::mpsc;

use crate::llm::LLM;

// ── mpsc UnboundedReceiver → futures::Stream adapter ──

pub struct ReceiverStream<T> {
    rx: mpsc::UnboundedReceiver<T>,
}

impl<T> Stream for ReceiverStream<T> {
    type Item = T;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.rx.poll_recv(cx)
    }
}

// ── Common streaming message type ──

#[derive(Debug, Clone)]
pub enum StreamMsg {
    Started,
    Text(String),
    Reasoning(String),
    Done,
    Error(String),
}

// ── Shared streaming function ──

pub fn stream_prompt(
    llm: Arc<LLM>,
    prompt: String,
    history: Vec<rig::completion::Message>,
) -> impl Stream<Item = StreamMsg> + Send {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut stream = llm.stream_chat(prompt, history).await;
        let mut final_text = String::new();

        while let Some(item) = stream.next().await {
            match item {
                Ok(MultiTurnStreamItem::StreamAssistantItem(content)) => match content {
                    StreamedAssistantContent::Text(t) => {
                        final_text.push_str(&t.text);
                        let _ = tx.send(StreamMsg::Text(t.text));
                    }
                    StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                        let _ = tx.send(StreamMsg::Reasoning(reasoning));
                    }
                    _ => {}
                },
                Ok(MultiTurnStreamItem::FinalResponse(resp)) => {
                    let output = resp.output;
                    if !output.is_empty() && final_text.is_empty() {
                        let _ = tx.send(StreamMsg::Text(output));
                    }
                }
                _ => {}
            }
        }

        let _ = tx.send(StreamMsg::Done);
    });

    ReceiverStream { rx }
}
