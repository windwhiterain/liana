use std::sync::Arc;

use iced::{Element, Task, Theme};

use crate::{
    config::{Config, load_config},
    llm::{LLM, llm_from_config},
    state::chat::{Chat, ChatMessage},
};

pub mod config;
pub mod llm;
pub mod memory;
pub mod probe;
pub mod state;

// ── State trait (non-object-safe: each state works with its own message type) ──

pub trait State {
    type Message: Send + 'static;

    fn update(
        &mut self,
        message: Self::Message,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<Self::Message>, Option<Box<dyn ErasedState>>);

    fn view(&self) -> Element<'_, Self::Message>;
}

// ── Object-safe trait for type-erased storage ──

pub trait ErasedState {
    fn update_erased(
        &mut self,
        message: Message,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<Message>, Option<Box<dyn ErasedState>>);

    fn view_erased(&self) -> Element<'_, Message>;
}

// ── Adapter: wraps a typed State, bridges Self::Message ↔ Message ──

pub struct Erased<S: State> {
    state: S,
}

impl<S: State> Erased<S> {
    pub fn new(state: S) -> Self {
        Self { state }
    }
}

impl<S: State> ErasedState for Erased<S>
where
    S::Message: TryFrom<Message, Error = Message> + Into<Message>,
{
    fn update_erased(
        &mut self,
        message: Message,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<Message>, Option<Box<dyn ErasedState>>) {
        match S::Message::try_from(message) {
            Ok(typed) => {
                let (task, transition) = self.state.update(typed, llm, memory_manager);
                (task.map(Into::into), transition)
            }
            Err(_) => (Task::none(), None),
        }
    }

    fn view_erased(&self) -> Element<'_, Message> {
        self.state.view().map(Into::into)
    }
}

// ── App ──

pub struct App {
    pub config: Config,
    pub llm: Arc<LLM>,
    pub memory_manager: memory::Manager,
    state: Box<dyn ErasedState>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Chat(ChatMessage),
}

// ── ChatMessage ↔ Message conversions ──

impl From<ChatMessage> for Message {
    fn from(msg: ChatMessage) -> Self {
        Message::Chat(msg)
    }
}

impl TryFrom<Message> for ChatMessage {
    type Error = Message;

    fn try_from(msg: Message) -> Result<Self, Message> {
        let Message::Chat(m) = msg;
        Ok(m)
    }
}

// ── Entry points ──

pub fn init() -> App {
    let config = load_config();
    let llm = Arc::new(llm_from_config(&config));
    App {
        config,
        llm,
        memory_manager: memory::Manager::default(),
        state: Box::new(Erased::new(Chat::default())),
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    let (task, transition) = app
        .state
        .update_erased(message, &app.llm, &mut app.memory_manager);
    if let Some(new_state) = transition {
        app.state = new_state;
    }
    task
}

pub fn view(app: &App) -> Element<'_, Message> {
    app.state.view_erased()
}

pub fn theme(_app: &App) -> Theme {
    Theme::Dark
}
