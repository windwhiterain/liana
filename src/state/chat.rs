use std::sync::Arc;

use frostmark::{MarkState, MarkWidget};
use iced::{
    Alignment, Element, Length, Task,
    widget::{button, column, container, row, scrollable, text},
};
use liana_probe::Probe;
use rig::{
    OneOrMany,
    agent::Text,
    completion::{Prompt, PromptError},
    message::{AssistantContent, Message, UserContent},
};

use crate::{
    ErasedState, State,
    llm::LLM,
    memory::{self, SUMMARY_PROMPT},
    probe,
};

pub struct Chat {
    pub messages: Vec<Message>,
    markdown_states: Vec<MarkState>,
    pub config: Config,
    pub parent_memory: Option<memory::NodeId>,
    pub busy: bool,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            markdown_states: Vec::new(),
            config: Config::default(),
            parent_memory: None,
            busy: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatMessage {
    Run,
    ConfigProbe(probe::ProbeMsg<ConfigProbeMsg>),
    Response(Result<String, String>),
}

#[derive(Debug, Clone, Probe)]
#[probe(tags = "inlined")]
pub enum Config {
    Message(MessageConfig),
    Summary,
}

impl Default for Config {
    fn default() -> Self {
        Self::Message(MessageConfig::default())
    }
}

#[derive(Debug, Clone, Probe)]
pub struct MessageConfig {
    #[probe(hide_label)]
    pub message: probe::TextEditor,
}

impl Default for MessageConfig {
    fn default() -> Self {
        Self {
            message: probe::TextEditor::new(),
        }
    }
}

impl Chat {
    pub fn with_parent_memory(parent_memory: Option<memory::NodeId>) -> Self {
        Self {
            parent_memory,
            ..Self::default()
        }
    }

    pub fn context<'a>(
        parent_memory: Option<memory::NodeId>,
        messages: &'a Vec<Message>,
        memory_manager: &'a memory::Manager,
    ) -> impl Iterator<Item = Message> {
        memory_manager
            .messages(parent_memory)
            .chain(messages.iter().cloned())
    }
}

impl State for Chat {
    type Message = ChatMessage;

    fn update(
        &mut self,
        message: ChatMessage,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<ChatMessage>, Option<Box<dyn ErasedState>>) {
        match message {
            ChatMessage::Run => {
                if self.busy {
                    return (Task::none(), None);
                }
                self.busy = true;

                match &mut self.config {
                    Config::Message(config) => {
                        let message_text = config.message.text().to_string();
                        config.message = probe::TextEditor::new();
                        let llm = Arc::clone(llm);
                        let history =
                            Self::context(self.parent_memory, &self.messages, memory_manager)
                                .collect::<Vec<_>>();

                        self.messages.push(Message::User {
                            content: OneOrMany::one(UserContent::Text(Text {
                                text: message_text.clone(),
                                ..Default::default()
                            })),
                        });
                        self.markdown_states.push(MarkState::default());

                        (
                            Task::perform(
                                async move {
                                    llm.prompt(message_text)
                                        .history(history)
                                        .await
                                        .map_err(|e: PromptError| e.to_string())
                                },
                                ChatMessage::Response,
                            ),
                            None,
                        )
                    }
                    Config::Summary => {
                        let llm = Arc::clone(llm);
                        let history =
                            Self::context(self.parent_memory, &self.messages, memory_manager)
                                .collect::<Vec<_>>();

                        self.messages.push(Message::User {
                            content: OneOrMany::one(UserContent::Text(Text {
                                text: SUMMARY_PROMPT.to_string(),
                                ..Default::default()
                            })),
                        });
                        self.markdown_states.push(MarkState::default());

                        (
                            Task::perform(
                                async move {
                                    llm.prompt(SUMMARY_PROMPT)
                                        .history(history)
                                        .await
                                        .map_err(|e: PromptError| e.to_string())
                                },
                                ChatMessage::Response,
                            ),
                            None,
                        )
                    }
                }
            }
            ChatMessage::Response(result) => {
                self.busy = false;
                match result {
                    Ok(response) => {
                        self.markdown_states.push(
                            MarkState::with_html_and_markdown(&response),
                        );
                        self.messages.push(Message::Assistant {
                            id: None,
                            content: OneOrMany::one(AssistantContent::Text(Text {
                                text: response,
                                ..Default::default()
                            })),
                        });
                    }
                    Err(err) => {
                        eprintln!("LLM error: {}", err);
                    }
                }
                (Task::none(), None)
            }
            ChatMessage::ConfigProbe(msg) => {
                probe::probe_update(&mut self.config, msg);
                (Task::none(), None)
            }
        }
    }

    fn view(&self) -> Element<'_, ChatMessage> {
        column![
            scrollable(column(
                self.messages
                    .iter()
                    .enumerate()
                    .map(|(i, msg)| render_message(i, msg, &self.markdown_states))
                    .collect::<Vec<_>>()
            )
            .spacing(12))
            .height(Length::Fill),
            container(
                row![
                    probe::probe_view(&self.config).map(ChatMessage::ConfigProbe),
                    button(text("run"))
                        .on_press(ChatMessage::Run)
                        .width(Length::Shrink),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(8),
        ]
        .into()
    }
}

fn render_message<'a>(
    index: usize,
    message: &Message,
    states: &'a [MarkState],
) -> Element<'a, ChatMessage> {
    match message {
        Message::User { content } => {
            let texts: Vec<String> = content
                .iter()
                .filter_map(|item| {
                    if let UserContent::Text(text) = item {
                        Some(text.text.clone())
                    } else {
                        None
                    }
                })
                .collect();
            column![text("You").size(14), text(texts.join("\n"))]
                .spacing(4)
                .into()
        }
        Message::System { content } => {
            column![text("System").size(14), text(content.clone())]
                .spacing(4)
                .into()
        }
        Message::Assistant { .. } => {
            let md_widget: Element<'a, ChatMessage> = if index < states.len() {
                MarkWidget::new(&states[index]).into()
            } else {
                text("").into()
            };
            column![text("Liana").size(14), md_widget]
                .spacing(4)
                .into()
        }
    }
}


