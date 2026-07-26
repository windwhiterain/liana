use std::any::Any;
use std::sync::Arc;

use iced::{
    Color, Element, Length, Task, Theme,
    widget::{button, column, container, row, text},
};

use crate::{
    config::{Config, load_config},
    llm::{LLM, llm_from_config},
    state::chat::{Chat, ChatMessage},
    state::recall::{Recall, RecallMessage},
};

pub mod config;
pub mod llm;
pub mod memory;
pub mod probe;
pub mod state;

// ── Message type (type-erased, supports dynamic state injection) ──

pub type Message = Box<dyn Any + Send>;

// ── State trait (non-object-safe: each state works with its own message type) ──

pub trait State {
    type Message: Any + Send + Into<Message>;

    fn update(
        &mut self,
        message: Self::Message,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<Self::Message>, Option<Box<dyn ErasedState>>);

    fn view<'a>(&'a self, memory_manager: &'a memory::Manager) -> Element<'a, Self::Message>;
}

// ── Object-safe trait for type-erased storage ──

pub trait ErasedState {
    fn update_erased(
        &mut self,
        message: Message,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<Message>, Option<Box<dyn ErasedState>>);

    fn view_erased<'a>(&'a self, memory_manager: &'a memory::Manager) -> Element<'a, Message>;
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

impl<S: State> ErasedState for Erased<S> {
    fn update_erased(
        &mut self,
        message: Message,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<Message>, Option<Box<dyn ErasedState>>) {
        match message.downcast::<S::Message>() {
            Ok(typed) => {
                let (task, transition) = self.state.update(*typed, llm, memory_manager);
                (task.map(Into::into), transition)
            }
            Err(_) => (Task::none(), None),
        }
    }

    fn view_erased<'a>(&'a self, memory_manager: &'a memory::Manager) -> Element<'a, Message> {
        self.state.view(memory_manager).map(Into::into)
    }
}

// ── Sidebar ──

pub struct SidebarEntry {
    pub label: &'static str,
    pub factory: Box<dyn Fn(&App) -> Box<dyn ErasedState>>,
}

#[derive(Debug, Clone)]
pub struct SidebarNav(pub usize);

// ── App ──

pub struct App {
    pub config: Config,
    pub llm: Arc<LLM>,
    pub memory_manager: memory::Manager,
    pub sidebar: Vec<SidebarEntry>,
    state: Box<dyn ErasedState>,
}

// ── SidebarNav → Message ──

impl From<SidebarNav> for Message {
    fn from(nav: SidebarNav) -> Self {
        Box::new(nav)
    }
}

// ── ChatMessage → Message ──

impl From<ChatMessage> for Message {
    fn from(msg: ChatMessage) -> Self {
        Box::new(msg)
    }
}

// ── RecallMessage → Message ──

impl From<RecallMessage> for Message {
    fn from(msg: RecallMessage) -> Self {
        Box::new(msg)
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
        sidebar: vec![
            SidebarEntry {
                label: "Chat",
                factory: Box::new(|_| Box::new(Erased::new(Chat::default()))),
            },
            SidebarEntry {
                label: "Recall",
                factory: Box::new(|app| Box::new(Erased::new(Recall::new(&app.memory_manager)))),
            },
        ],
        state: Box::new(Erased::new(Chat::default())),
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message.downcast::<SidebarNav>() {
        Ok(nav) => {
            if nav.0 < app.sidebar.len() {
                app.state = (app.sidebar[nav.0].factory)(app);
            }
            Task::none()
        }
        Err(message) => {
            let (task, transition) = app
                .state
                .update_erased(message, &app.llm, &mut app.memory_manager);
            if let Some(new_state) = transition {
                app.state = new_state;
            }
            task
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let buttons: Vec<Element<'_, Message>> = app
        .sidebar
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let btn: Element<'_, SidebarNav> = button(text(entry.label))
                .on_press(SidebarNav(i))
                .width(Length::Fill)
                .into();
            btn.map(|nav| Box::new(nav) as Message)
        })
        .collect();

    let sidebar = container(column(buttons).spacing(4))
        .width(Length::Fixed(120.0))
        .height(Length::Fill)
        .padding(8)
        .style(|_: &Theme| {
            container::Style {
                background: Some(iced::Background::Color(Color::from_rgb(
                    0.1, 0.11, 0.16,
                ))),
                ..Default::default()
            }
        });

    row![sidebar, app.state.view_erased(&app.memory_manager)].into()
}

pub fn theme(_app: &App) -> Theme {
    Theme::Dark
}
