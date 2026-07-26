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

pub struct App {
    pub config: Config,
    pub llm: Arc<LLM>,
    pub memory_manager: memory::Manager,
    screen: Chat,
}

#[derive(Debug, Clone)]
pub enum Message {
    Chat(ChatMessage),
}

pub fn init() -> App {
    let config = load_config();
    let llm = Arc::new(llm_from_config(&config));
    App {
        config,
        llm,
        memory_manager: memory::Manager::default(),
        screen: Chat::default(),
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Chat(chat_msg) => app
            .screen
            .update(chat_msg, &app.llm, &mut app.memory_manager)
            .map(Message::Chat),
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    app.screen.view().map(Message::Chat)
}

pub fn theme(_app: &App) -> Theme {
    Theme::Dark
}
