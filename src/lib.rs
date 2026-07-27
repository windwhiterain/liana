use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use iced::widget::markdown;
use iced::{
    Element, Length, Task, Theme,
    widget::{button, column, container, row, text},
};
use rig::message::Message as RigMessage;

use crate::{
    config::{Config, load_config},
    llm::{LLM, llm_from_config, llm_json_from_config},
    state::{
        Erased, ErasedState, Message, StreamIntent, StreamResponse,
        chat::Chat,
        recall::Recall,
    },
    stream::StreamMsg,
};

pub mod config;
pub mod llm;
pub mod memory;
pub mod probe;
pub mod state;
pub mod stream;

// ── Shared mutable context for states ──

pub struct Data {
    pub llm: Arc<LLM>,
    pub llm_json: Arc<LLM>,
    pub memory_manager: memory::Manager,
    // Transition params
    pub parent_memory: Option<memory::NodeId>,
    // Conversation state (owned by Data so Recall can clear it)
    pub messages: Vec<RigMessage>,
    pub markdown_states: Vec<Vec<markdown::Item>>,
    pub reasoning_markdown_states: Vec<Option<Vec<markdown::Item>>>,
    // Memory markdowns for Recall view (mirrors memory_manager.memories)
    pub memory_markdowns: Vec<Vec<markdown::Item>>,
}

// ── Sidebar ──

pub struct SidebarEntry {
    pub label: &'static str,
    pub type_id: TypeId,
}

#[derive(Debug, Clone)]
pub struct SidebarNav(pub TypeId);

// ── App ──

pub struct App {
    pub config: Config,
    pub data: Data,
    pub sidebar: Vec<SidebarEntry>,
    states: HashMap<TypeId, Box<dyn ErasedState>>,
    active: TypeId,
}

// ── SidebarNav → Message ──

impl From<SidebarNav> for Message {
    fn from(nav: SidebarNav) -> Self {
        Box::new(nav)
    }
}

// ── Entry points ──

pub fn init() -> App {
    let config = load_config();
    let memory_manager = memory::Manager::default();
    let memory_markdowns = memory_manager
        .memories
        .iter()
        .map(|m| markdown::parse(&m.summary).collect())
        .collect();
    let data = Data {
        llm: Arc::new(llm_from_config(&config)),
        llm_json: Arc::new(llm_json_from_config(&config)),
        memory_manager,
        parent_memory: None,
        messages: Vec::new(),
        markdown_states: Vec::new(),
        reasoning_markdown_states: Vec::new(),
        memory_markdowns,
    };

    let mut states = HashMap::new();
    let chat_id = TypeId::of::<Chat>();
    let recall_id = TypeId::of::<Recall>();

    // Recall needs memory_manager reference, created first
    states.insert(recall_id, Box::new(Erased::new(Recall::new(&data.memory_manager))) as Box<dyn ErasedState>);
    states.insert(chat_id, Box::new(Erased::new(Chat::default())) as Box<dyn ErasedState>);

    App {
        config,
        data,
        sidebar: vec![
            SidebarEntry { label: "Chat", type_id: chat_id },
            SidebarEntry { label: "Recall", type_id: recall_id },
        ],
        states,
        active: chat_id,
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    // ── Stream service: intercept StreamIntent → spawn streaming ──
    match message.downcast::<StreamIntent>() {
        Ok(intent) => {
            let owner = intent.owner;
            let task_id = intent.task_id;
            let llm = if intent.json {
                Arc::clone(&app.data.llm_json)
            } else {
                Arc::clone(&app.data.llm)
            };
            let s = stream::stream_prompt(llm, intent.prompt, intent.history);
            let stream_task = Task::stream(s.map(move |msg| {
                Box::new(StreamResponse { owner, task_id, content: msg }) as Message
            }));
            let start_msg = Box::new(StreamResponse {
                owner,
                task_id,
                content: StreamMsg::Started,
            }) as Message;
            return Task::batch([stream_task, Task::done(start_msg)]);
        }
        Err(message) => {
            // ── Stream service: route StreamResponse to active state ──
            match message.downcast::<StreamResponse>() {
                Ok(resp) => {
                    let owner_id = resp.owner;
                    let (task, transition) = app
                        .states
                        .get_mut(&owner_id)
                        .expect("stream owner state")
                        .handle_stream_response(resp.content, &mut app.data);
                    if let Some(type_id) = transition {
                        app.active = type_id;
                    }
                    return task;
                }
                Err(message) => {
                    // ── Sidebar nav ──
                    match message.downcast::<SidebarNav>() {
                        Ok(nav) => {
                            if app.states.contains_key(&nav.0) {
                                app.active = nav.0;
                            }
                            Task::none()
                        }
                        Err(message) => {
                            let (task, transition) = app
                                .states
                                .get_mut(&app.active)
                                .expect("active state")
                                .update_erased(message, &mut app.data);
                            if let Some(type_id) = transition {
                                app.active = type_id;
                            }
                            task
                        }
                    }
                }
            }
        }
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let buttons: Vec<Element<'_, Message>> = app
        .sidebar
        .iter()
        .map(|entry| {
            let btn: Element<'_, SidebarNav> = button(text(entry.label))
                .on_press(SidebarNav(entry.type_id))
                .width(Length::Fill)
                .into();
            btn.map(|nav| Box::new(nav) as Message)
        })
        .collect();

    let sidebar = container(column(buttons).spacing(4))
        .width(Length::Fixed(120.0))
        .height(Length::Fill)
        .padding(8)
        .style(|theme: &Theme| {
            container::Style {
                background: Some(iced::Background::Color(
                    theme.extended_palette().background.weak.color,
                )),
                ..Default::default()
            }
        });

    let active_view = app
        .states
        .get(&app.active)
        .expect("active state")
        .view_erased(&app.data);

    row![sidebar, active_view].into()
}

pub fn theme(_app: &App) -> Theme {
    Theme::Dark
}
