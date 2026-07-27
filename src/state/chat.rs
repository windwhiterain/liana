use iced::widget::markdown;
use iced::{
    Alignment, Element, Length, Task, Theme,
    widget::{button, column, container, row, scrollable, text},
};
use liana_probe::Probe;
use rig::{
    OneOrMany,
    agent::Text,
    message::{AssistantContent, Message, Reasoning, UserContent},
};

use super::{State, StreamIntent};
use crate::{
    memory::{self, SUMMARY_PROMPT},
    probe,
    stream::StreamMsg,
};

pub struct Chat {
    pub config: Config,
    busy: Busy,
    streaming_text: String,
    streaming_reasoning: String,
    streaming_markdown: Vec<markdown::Item>,
    streaming_reasoning_md: Vec<markdown::Item>,
    reasoning_expanded: bool,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            config: Config::default(),
            busy: Busy::Idle,
            streaming_text: String::new(),
            streaming_reasoning: String::new(),
            streaming_markdown: Vec::new(),
            streaming_reasoning_md: Vec::new(),
            reasoning_expanded: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatMessage {
    Run,
    ConfigProbe(probe::ProbeMsg<ConfigProbeMsg>),
    StreamIntent(StreamIntent),
    ToggleReasoning,
    LinkClicked(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Busy { Idle, Message, Summary }

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
    pub fn context<'a>(data: &'a crate::Data) -> impl Iterator<Item = Message> {
        data.memory_manager
            .messages(data.parent_memory)
            .chain(data.messages.iter().cloned())
    }
}

impl State for Chat {
    type Message = ChatMessage;

    fn update(
        &mut self,
        message: ChatMessage,
        data: &mut crate::Data,
    ) -> (Task<ChatMessage>, Option<std::any::TypeId>) {
        match message {
            ChatMessage::Run => {
                if self.busy != Busy::Idle {
                    return (Task::none(), None);
                }

                match &mut self.config {
                    Config::Message(config) => {
                        let message_text = config.message.text().to_string();
                        config.message = probe::TextEditor::new();
                        let history = Self::context(data).collect::<Vec<_>>();

                        data.messages.push(Message::User {
                            content: OneOrMany::one(UserContent::Text(Text {
                                text: message_text.clone(),
                                ..Default::default()
                            })),
                        });
                        data.markdown_states.push(Vec::new());
                        data.reasoning_markdown_states.push(None);
                        self.streaming_text.clear();
                        self.streaming_reasoning.clear();
                        self.streaming_markdown = Vec::new();
                        self.streaming_reasoning_md = Vec::new();
                        self.busy = Busy::Message;

                        (
                            Task::done(ChatMessage::StreamIntent(StreamIntent {
                                owner: Self::type_id(),
                                task_id: 0,
                                prompt: message_text,
                                history,
                                json: false,
                            })),
                            None,
                        )
                    }
                    Config::Summary => {
                        let history = Self::context(data).collect::<Vec<_>>();

                        data.messages.push(Message::User {
                            content: OneOrMany::one(UserContent::Text(Text {
                                text: SUMMARY_PROMPT.to_string(),
                                ..Default::default()
                            })),
                        });
                        data.markdown_states.push(Vec::new());
                        data.reasoning_markdown_states.push(None);
                        self.streaming_text.clear();
                        self.streaming_reasoning.clear();
                        self.streaming_markdown = Vec::new();
                        self.streaming_reasoning_md = Vec::new();
                        self.busy = Busy::Summary;

                        (
                            Task::done(ChatMessage::StreamIntent(StreamIntent {
                                owner: Self::type_id(),
                                task_id: 0,
                                prompt: SUMMARY_PROMPT.to_string(),
                                history,
                                json: false,
                            })),
                            None,
                        )
                    }
                }
            }
            ChatMessage::ConfigProbe(msg) => {
                probe::probe_update(&mut self.config, msg);
                (Task::none(), None)
            }
            ChatMessage::StreamIntent(_) => {
                (Task::none(), None)
            }
            ChatMessage::ToggleReasoning => {
                self.reasoning_expanded = !self.reasoning_expanded;
                (Task::none(), None)
            }
            ChatMessage::LinkClicked(_) => (Task::none(), None),
        }
    }

    fn handle_stream_response(
        &mut self,
        content: StreamMsg,
        data: &mut crate::Data,
    ) -> (Task<ChatMessage>, Option<std::any::TypeId>) {
        match content {
            StreamMsg::Started => (Task::none(), None),
            StreamMsg::Text(text) => {
                self.streaming_text.push_str(&text);
                self.streaming_markdown = markdown::parse(&self.streaming_text).collect();
                (Task::none(), None)
            }
            StreamMsg::Reasoning(reasoning) => {
                self.streaming_reasoning.push_str(&reasoning);
                self.streaming_reasoning_md = markdown::parse(&self.streaming_reasoning).collect();
                (Task::none(), None)
            }
            StreamMsg::Done => {
                let reasoning = std::mem::take(&mut self.streaming_reasoning);
                let text = std::mem::take(&mut self.streaming_text);

                match self.busy {
                    Busy::Message => {
                        data.reasoning_markdown_states.push(if !reasoning.is_empty() {
                            Some(markdown::parse(&reasoning).collect())
                        } else {
                            None
                        });
                        data.markdown_states
                            .push(markdown::parse(&text).collect());
                        let mut content: Vec<AssistantContent> = vec![];
                        if !reasoning.is_empty() {
                            content.push(AssistantContent::Reasoning(
                                Reasoning::new(&reasoning),
                            ));
                        }
                        content.push(AssistantContent::Text(Text {
                            text,
                            ..Default::default()
                        }));
                        data.messages.push(Message::Assistant {
                            id: None,
                            content: OneOrMany::many(content)
                                .expect("assistant content is never empty"),
                        });
                    }
                    Busy::Summary => {
                        data.reasoning_markdown_states.push(None);
                        data.messages.push(Message::Assistant {
                            id: None,
                            content: OneOrMany::one(AssistantContent::Text(Text {
                                text: text.clone(),
                                ..Default::default()
                            })),
                        });
                        let messages = std::mem::take(&mut data.messages);
                        data.markdown_states.clear();
                        data.reasoning_markdown_states.clear();
                        let memory = memory::Memory::new(messages, text);
                        data.memory_manager.add_memory(memory, data.parent_memory);
                        data.parent_memory = data.memory_manager
                            .memories
                            .last()
                            .and_then(|m| {
                                data.memory_markdowns
                                    .push(markdown::parse(&m.summary).collect());
                                m.nodes.last().copied()
                            });
                    }
                    Busy::Idle => {}
                }
                self.busy = Busy::Idle;
                (Task::none(), None)
            }
            StreamMsg::Error(err) => {
                self.busy = Busy::Idle;
                self.streaming_text.clear();
                self.streaming_reasoning.clear();
                self.streaming_markdown = Vec::new();
                self.streaming_reasoning_md = Vec::new();
                eprintln!("Chat stream error: {}", err);
                (Task::none(), None)
            }
        }
    }

    fn view<'a>(&'a self, data: &'a crate::Data) -> Element<'a, ChatMessage> {
        let mut children: Vec<Element<'a, ChatMessage>> = vec![];

        let mut message_items: Vec<Element<'a, ChatMessage>> = data
            .messages
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                render_message(
                    i,
                    msg,
                    &data.markdown_states,
                    &data.reasoning_markdown_states,
                    self.reasoning_expanded,
                )
            })
            .collect();

        if self.busy != Busy::Idle && (!self.streaming_text.is_empty() || !self.streaming_reasoning.is_empty())
        {
            message_items.push(render_streaming(
                &self.streaming_reasoning,
                &self.streaming_reasoning_md,
                &self.streaming_text,
                &self.streaming_markdown,
                self.reasoning_expanded,
            ));
        }

        children.push(
            scrollable(column(message_items).spacing(12))
                .height(Length::Fill)
                .into(),
        );

        children.push(
            container(
                row![
                    probe::probe_view(&self.config).map(ChatMessage::ConfigProbe),
                    button(text("run"))
                        .on_press_maybe(if self.busy != Busy::Idle { None } else { Some(ChatMessage::Run) })
                        .width(Length::Shrink),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(8)
            .into(),
        );

        column(children).into()
    }
}

fn render_message<'a>(
    index: usize,
    message: &Message,
    states: &'a [Vec<markdown::Item>],
    reasoning_states: &'a [Option<Vec<markdown::Item>>],
    reasoning_expanded: bool,
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
        Message::Assistant { content: assistant_content, .. } => {
            if index >= states.len() {
                return text("").into();
            }
            let mut items: Vec<Element<'a, ChatMessage>> = vec![];

            for c in assistant_content.iter() {
                match c {
                    AssistantContent::Reasoning(_) => {
                        if let Some(Some(reasoning_md)) = reasoning_states.get(index) {
                            items.push(render_reasoning_section(reasoning_md, reasoning_expanded));
                        }
                    }
                    _ => {}
                }
            }

            let md_widget: Element<'a, ChatMessage> = markdown::view(&states[index], Theme::Dark)
                .map(|_| ChatMessage::LinkClicked(String::new()))
                .into();
            items.push(column![text("Liana").size(14), md_widget].spacing(4).into());
            column(items).spacing(8).into()
        }
    }
}

fn render_reasoning_section<'a>(
    reasoning_md: &'a [markdown::Item],
    expanded: bool,
) -> Element<'a, ChatMessage> {
    if expanded {
        let md: Element<'a, ChatMessage> = markdown::view(reasoning_md, Theme::Dark)
            .map(|_| ChatMessage::LinkClicked(String::new()))
            .into();
        let boxed: Element<'a, ChatMessage> = container(md)
            .padding(8)
            .style(|theme: &Theme| iced::widget::container::Style {
                border: iced::Border {
                    color: {
                        let mut c = theme.extended_palette().primary.base.color;
                        c.a = 0.25;
                        c
                    },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .into();
        column![
            button(text("Hide thinking"))
                .on_press(ChatMessage::ToggleReasoning),
            boxed,
        ]
        .spacing(4)
        .into()
    } else {
        button(text("Show thinking..."))
            .on_press(ChatMessage::ToggleReasoning)
            .into()
    }
}

fn render_streaming<'a>(
    reasoning: &'a str,
    reasoning_md: &'a [markdown::Item],
    response: &'a str,
    response_md: &'a [markdown::Item],
    expanded: bool,
) -> Element<'a, ChatMessage> {
    let mut children: Vec<Element<'a, ChatMessage>> = vec![];
    if !reasoning.is_empty() {
        if expanded {
            let md: Element<'a, ChatMessage> = markdown::view(reasoning_md, Theme::Dark)
                .map(|_| ChatMessage::LinkClicked(String::new()))
                .into();
            let boxed: Element<'a, ChatMessage> = container(md)
                .padding(8)
                .style(|theme: &Theme| iced::widget::container::Style {
                    border: iced::Border {
                        color: {
                            let mut c = theme.palette().text;
                            c.a = 0.15;
                            c
                        },
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .into();
            children.push(
                column![
                    button(text("Hide thinking"))
                        .on_press(ChatMessage::ToggleReasoning),
                    boxed,
                ]
                .spacing(4)
                .into(),
            );
        } else {
            children.push(
                button(text("Show thinking..."))
                    .on_press(ChatMessage::ToggleReasoning)
                    .into(),
            );
        }
    }
    if !response.is_empty() {
        let md: Element<'a, ChatMessage> = markdown::view(response_md, Theme::Dark)
            .map(|_| ChatMessage::LinkClicked(String::new()))
            .into();
        children.push(
            column![text("Streaming...").size(12), md]
                .spacing(4)
                .into(),
        );
    }
    column(children).spacing(8).into()
}
