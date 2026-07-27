use iced::widget::markdown;
use iced::{
    Element, Length, Task, Theme,
    widget::{button, column, container, row, scrollable, text, toggler},
};
use liana_probe::Probe;
use rig::message::Message;

use super::{State, StreamIntent};
use crate::{
    memory::{self, LIST_MEMORY_PROMPT, SELECT_MEMORY_PROMPT},
    probe,
    stream::StreamMsg,
};

pub struct Recall {
    pub config: Config,
    checked: Vec<bool>,
    busy: bool,
    streaming_text: String,
    streaming_reasoning: String,
    streaming_reasoning_md: Vec<markdown::Item>,
    reasoning_expanded: bool,
}

#[derive(Debug, Clone)]
pub enum RecallMessage {
    Run,
    Toggle(usize, bool),
    ConfigProbe(probe::ProbeMsg<ConfigProbeMsg>),
    StreamIntent(StreamIntent),
    ToggleReasoning,
    LinkClicked(String),
}

#[derive(Debug, Clone, Probe)]
#[probe(tags = "inlined")]
pub enum Config {
    LLM(LLMConfig),
    Confirm,
}

impl Default for Config {
    fn default() -> Self {
        Self::LLM(LLMConfig::default())
    }
}

#[derive(Debug, Clone, Probe)]
pub struct LLMConfig {
    #[probe(hide_label)]
    pub question: probe::TextEditor,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            question: probe::TextEditor::new(),
        }
    }
}

impl Recall {
    pub fn new(memory_manager: &memory::Manager) -> Self {
        Self {
            config: Config::default(),
            checked: vec![false; memory_manager.memories.len()],
            busy: false,
            streaming_text: String::new(),
            streaming_reasoning: String::new(),
            streaming_reasoning_md: Vec::new(),
            reasoning_expanded: false,
        }
    }
}

impl State for Recall {
    type Message = RecallMessage;

    fn update(
        &mut self,
        message: RecallMessage,
        data: &mut crate::Data,
    ) -> (Task<RecallMessage>, Option<std::any::TypeId>) {
        // Sync checked with current memory count
        let n = data.memory_manager.memories.len();
        if self.checked.len() < n {
            self.checked.resize(n, false);
        }
        match message {
            RecallMessage::ConfigProbe(msg) => {
                probe::probe_update(&mut self.config, msg);
                (Task::none(), None)
            }
            RecallMessage::Toggle(i, checked) => {
                if i < self.checked.len() {
                    self.checked[i] = checked;
                }
                (Task::none(), None)
            }
            RecallMessage::ToggleReasoning => {
                self.reasoning_expanded = !self.reasoning_expanded;
                (Task::none(), None)
            }
            RecallMessage::Run => match &mut self.config {
                Config::LLM(llm_config) => {
                    if self.busy {
                        return (Task::none(), None);
                    }
                    let question_text = llm_config.question.text().to_string();
                    if question_text.trim().is_empty() {
                        return (Task::none(), None);
                    }
                    llm_config.question = probe::TextEditor::new();
                    self.busy = true;
                    self.streaming_text.clear();
                    self.streaming_reasoning.clear();
                    self.streaming_reasoning_md = Vec::new();

                    let memories_display = data.memory_manager.list_memories().to_string();
                    if memories_display.is_empty() {
                        self.busy = false;
                        return (Task::none(), None);
                    }

                    let mut history = vec![];
                    history.push(Message::user(question_text));
                    history.push(Message::assistant(format!(
                        "{}\n{}",
                        LIST_MEMORY_PROMPT, memories_display
                    )));
                    (
                        Task::done(RecallMessage::StreamIntent(StreamIntent {
                            owner: Self::type_id(),
                            task_id: 0,
                            prompt: SELECT_MEMORY_PROMPT.to_string(),
                            history,
                            json: true,
                        })),
                        None,
                    )
                }
                Config::Confirm => {
                    let indices: Vec<usize> = self
                        .checked
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| **c)
                        .map(|(i, _)| i)
                        .collect();
                    if indices.is_empty() {
                        return (Task::none(), None);
                    }
                    let (node_id, _, _) = data.memory_manager.find(&indices);
                    data.parent_memory = node_id;
                    data.messages.clear();
                    data.markdown_states.clear();
                    data.reasoning_markdown_states.clear();
                    (Task::none(), Some(std::any::TypeId::of::<super::chat::Chat>()))
                }
            },
            RecallMessage::StreamIntent(_) => {
                (Task::none(), None)
            }
            RecallMessage::LinkClicked(_) => (Task::none(), None),
        }
    }

    fn handle_stream_response(
        &mut self,
        content: StreamMsg,
        _data: &mut crate::Data,
    ) -> (Task<RecallMessage>, Option<std::any::TypeId>) {
        match content {
            StreamMsg::Started => (Task::none(), None),
            StreamMsg::Text(text) => {
                self.streaming_text.push_str(&text);
                (Task::none(), None)
            }
            StreamMsg::Reasoning(reasoning) => {
                self.streaming_reasoning.push_str(&reasoning);
                self.streaming_reasoning_md = markdown::parse(&self.streaming_reasoning).collect();
                (Task::none(), None)
            }
            StreamMsg::Done => {
                let text = std::mem::take(&mut self.streaming_text);
                self.busy = false;

                let indices: Vec<usize> = serde_json::from_str(&text).unwrap_or_default();
                self.checked.fill(false);
                for &i in &indices {
                    if i < self.checked.len() {
                        self.checked[i] = true;
                    }
                }
                (Task::none(), None)
            }
            StreamMsg::Error(err) => {
                self.busy = false;
                self.streaming_text.clear();
                self.streaming_reasoning.clear();
                self.streaming_reasoning_md = Vec::new();
                eprintln!("Recall stream error: {}", err);
                (Task::none(), None)
            }
        }
    }

    fn view<'a>(&'a self, data: &'a crate::Data) -> Element<'a, RecallMessage> {
        let mut children: Vec<Element<'a, RecallMessage>> = vec![];

        let mut memory_items: Vec<Element<'_, RecallMessage>> = Vec::new();
        for (i, _) in data.memory_manager.memories.iter().enumerate() {
            let checked = self.checked.get(i).copied().unwrap_or(false);
            let md: Element<'_, RecallMessage> = match data.memory_markdowns.get(i) {
                Some(state) => markdown::view(state, Theme::Dark)
                    .map(|_| RecallMessage::LinkClicked(String::new()))
                    .into(),
                None => text("").into(),
            };
            memory_items.push(
                row![
                    toggler(checked).on_toggle(move |v| RecallMessage::Toggle(i, v)),
                    md,
                ]
                .spacing(8)
                .into(),
            );
        }
        // Append live streaming content to the scrollable memory list
        if !self.streaming_text.is_empty() || !self.streaming_reasoning.is_empty() {
            let mut streaming_col = vec![];
            if !self.streaming_reasoning.is_empty() {
                streaming_col.push(if self.reasoning_expanded {
                    let md: Element<'_, RecallMessage> =
                        markdown::view(&self.streaming_reasoning_md, Theme::Dark)
                            .map(|_| RecallMessage::LinkClicked(String::new()))
                            .into();
                    let boxed: Element<'_, RecallMessage> = container(md)
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
                        button(text("Hide thinking")).on_press(RecallMessage::ToggleReasoning),
                        boxed,
                    ]
                    .spacing(4)
                    .into()
                } else {
                    button(text("Show thinking..."))
                        .on_press(RecallMessage::ToggleReasoning)
                        .into()
                });
            }
            if !self.streaming_text.is_empty() {
                streaming_col.push(text(&self.streaming_text).size(12).into());
            }
            memory_items.push(
                container(column(streaming_col).spacing(4))
                    .padding(8)
                    .into(),
            );
        }

        children.push(
            scrollable(column(memory_items).spacing(4))
                .height(Length::Fill)
                .into(),
        );

        let bottom: Vec<Element<'_, RecallMessage>> = vec![
            probe::probe_view(&self.config).map(RecallMessage::ConfigProbe),
            button(text("run"))
                .on_press_maybe(if self.busy {
                    None
                } else {
                    Some(RecallMessage::Run)
                })
                .width(Length::Shrink)
                .into(),
        ];
        children.push(container(column(bottom).spacing(8)).padding(8).into());

        column(children).into()
    }
}
