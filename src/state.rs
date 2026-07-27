use std::any::{Any, TypeId};

use iced::{Element, Task};

use crate::{memory, stream::StreamMsg};

pub mod chat;
pub mod recall;

// ── ChatMessage → Message ──

impl From<chat::ChatMessage> for Message {
    fn from(msg: chat::ChatMessage) -> Self {
        match msg {
            chat::ChatMessage::StreamIntent(intent) => Box::new(intent),
            other => Box::new(other),
        }
    }
}

// ── RecallMessage → Message ──

impl From<recall::RecallMessage> for Message {
    fn from(msg: recall::RecallMessage) -> Self {
        match msg {
            recall::RecallMessage::StreamIntent(intent) => Box::new(intent),
            other => Box::new(other),
        }
    }
}

// ── Message type (type-erased, supports dynamic state injection) ──

pub type Message = Box<dyn Any + Send>;

// ── State trait (non-object-safe: each state works with its own message type) ──

pub trait State {
    type Message: Any + Send + Into<Message>;

    fn type_id() -> TypeId
    where
        Self: Sized + 'static,
    {
        TypeId::of::<Self>()
    }

    fn update(
        &mut self,
        message: Self::Message,
        data: &mut crate::Data,
    ) -> (Task<Self::Message>, Option<TypeId>);

    fn view<'a>(&'a self, data: &'a crate::Data) -> Element<'a, Self::Message>;

    fn handle_stream_response(
        &mut self,
        _content: StreamMsg,
        _data: &mut crate::Data,
    ) -> (Task<Self::Message>, Option<TypeId>) {
        (Task::none(), None)
    }
}

// ── Object-safe trait for type-erased storage ──

pub trait ErasedState {
    fn update_erased(
        &mut self,
        message: Message,
        data: &mut crate::Data,
    ) -> (Task<Message>, Option<TypeId>);

    fn view_erased<'a>(&'a self, data: &'a crate::Data) -> Element<'a, Message>;

    fn handle_stream_response(
        &mut self,
        content: StreamMsg,
        data: &mut crate::Data,
    ) -> (Task<Message>, Option<TypeId>);
}

// ── Adapter: wraps a typed State, bridges Self::Message ↔ Message via downcast ──

pub struct Erased<S: State> {
    state: S,
}

impl<S: State> Erased<S> {
    pub fn new(state: S) -> Self {
        Self { state }
    }
}

impl<S: State + 'static> ErasedState for Erased<S> {
    fn update_erased(
        &mut self,
        message: Message,
        data: &mut crate::Data,
    ) -> (Task<Message>, Option<TypeId>) {
        match message.downcast::<S::Message>() {
            Ok(typed) => {
                let (task, transition) = self.state.update(*typed, data);
                (task.map(Into::into), transition)
            }
            Err(_) => (Task::none(), None),
        }
    }

    fn view_erased<'a>(&'a self, data: &'a crate::Data) -> Element<'a, Message> {
        self.state.view(data).map(Into::into)
    }

    fn handle_stream_response(
        &mut self,
        content: StreamMsg,
        data: &mut crate::Data,
    ) -> (Task<Message>, Option<TypeId>) {
        let (task, transition) =
            self.state
                .handle_stream_response(content, data);
        (task.map(Into::into), transition)
    }
}

// ── Stream service types ──

/// State emits this to request an LLM stream. App intercepts and spawns it.
#[derive(Debug, Clone)]
pub struct StreamIntent {
    pub owner: TypeId,
    pub task_id: u64,
    pub prompt: String,
    pub history: Vec<memory::Message>,
    pub json: bool,
}

/// App-sent response chunk routed to the active state's handle_stream_response().
#[derive(Debug, Clone)]
pub struct StreamResponse {
    pub owner: TypeId,
    pub task_id: u64,
    pub content: StreamMsg,
}
