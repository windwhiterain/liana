use std::sync::Arc;

use iced::{
    Element, Length, Task,
    widget::{button, column, container, row, scrollable, text, toggler},
};
use liana_probe::Probe;
use rig::completion::{Prompt, PromptError};

use crate::{
    Erased, ErasedState, State,
    llm::LLM,
    memory::{self, SELECT_MEMORY_PROMPT},
    probe,
};

use super::chat::Chat;

pub struct Recall {
    pub config: Config,
    checked: Vec<bool>,
    busy: bool,
}

#[derive(Debug, Clone)]
pub enum RecallMessage {
    Run,
    Toggle(usize, bool),
    ConfigProbe(probe::ProbeMsg<ConfigProbeMsg>),
    Response(Result<String, String>),
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
        }
    }
}

impl State for Recall {
    type Message = RecallMessage;

    fn update(
        &mut self,
        message: RecallMessage,
        llm: &Arc<LLM>,
        memory_manager: &mut memory::Manager,
    ) -> (Task<RecallMessage>, Option<Box<dyn ErasedState>>) {
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

                    let memories_display = memory_manager.display_memories().to_string();
                    if memories_display.is_empty() {
                        self.busy = false;
                        return (Task::none(), None);
                    }

                    let prompt = format!(
                        "{}\n\nUser question: {}\n\nMemories:\n{}",
                        SELECT_MEMORY_PROMPT, question_text, memories_display,
                    );
                    let llm = Arc::clone(llm);

                    (
                        Task::perform(
                            async move {
                                llm.prompt(prompt)
                                    .await
                                    .map_err(|e: PromptError| e.to_string())
                            },
                            RecallMessage::Response,
                        ),
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
                    let (node_id, _, _) = memory_manager.find(&indices);
                    let chat = Chat::with_parent_memory(node_id);
                    (Task::none(), Some(Box::new(Erased::new(chat))))
                }
            },
            RecallMessage::Response(result) => {
                self.busy = false;
                match result {
                    Ok(response) => {
                        let indices: Vec<usize> =
                            serde_json::from_str(&response).unwrap_or_default();
                        self.checked.fill(false);
                        for &i in &indices {
                            if i < self.checked.len() {
                                self.checked[i] = true;
                            }
                        }
                        (Task::none(), None)
                    }
                    Err(err) => {
                        eprintln!("LLM error during memory selection: {}", err);
                        (Task::none(), None)
                    }
                }
            }
        }
    }

    fn view<'a>(&'a self, memory_manager: &'a memory::Manager) -> Element<'a, RecallMessage> {
        let memory_items: Vec<Element<'_, RecallMessage>> = memory_manager
            .memories
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let checked = self.checked.get(i).copied().unwrap_or(false);
                row![
                    toggler(checked).on_toggle(move |v| RecallMessage::Toggle(i, v)),
                    text(m.summary.as_str()),
                ]
                .spacing(8)
                .into()
            })
            .collect();

        let bottom: Vec<Element<'_, RecallMessage>> = vec![
            probe::probe_view(&self.config).map(RecallMessage::ConfigProbe),
            button(text("run"))
                .on_press_maybe(if self.busy { None } else { Some(RecallMessage::Run) })
                .width(Length::Shrink)
                .into(),
        ];

        column![
            scrollable(column(memory_items).spacing(4)).height(Length::Fill),
            container(column(bottom).spacing(8)).padding(8),
        ]
        .into()
    }
}
