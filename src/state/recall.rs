use std::sync::Arc;

use iced::{
    Alignment, Element, Length, Task,
    widget::{button, column, row, scrollable, text, text_editor},
};
use rig::completion::{Prompt, PromptError};

use crate::{
    Erased, ErasedState, State,
    llm::LLM,
    memory::{self, SELECT_MEMORY_PROMPT},
    probe,
};

use super::chat::Chat;

pub struct Recall {
    pub question: probe::TextEditor,
    memory_display: String,
    busy: bool,
}

#[derive(Debug, Clone)]
pub enum RecallMessage {
    Run,
    TextAction(probe::TextEditorAction),
    Response(Result<String, String>),
}

impl Recall {
    pub fn new(memory_manager: &memory::Manager) -> Self {
        Self {
            question: probe::TextEditor::new(),
            memory_display: memory_manager.display_memories().to_string(),
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
            RecallMessage::TextAction(action) => {
                self.question.perform(action);
                (Task::none(), None)
            }
            RecallMessage::Run => {
                if self.busy {
                    return (Task::none(), None);
                }
                let question_text = self.question.text().to_string();
                if question_text.trim().is_empty() {
                    return (Task::none(), None);
                }
                self.question = probe::TextEditor::new();
                self.busy = true;

                if self.memory_display.is_empty() {
                    self.busy = false;
                    return (Task::none(), Some(Box::new(Erased::new(Chat::default()))));
                }

                let prompt = format!(
                    "{}\n\nUser question: {}\n\nMemories:\n{}",
                    SELECT_MEMORY_PROMPT, question_text, self.memory_display,
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
            RecallMessage::Response(result) => {
                self.busy = false;
                match result {
                    Ok(response) => {
                        let indices: Vec<usize> =
                            serde_json::from_str(&response).unwrap_or_default();
                        if indices.is_empty() {
                            return (Task::none(), Some(Box::new(Erased::new(Chat::default()))));
                        }
                        let (node_id, _, _) = memory_manager.find(&indices);
                        let chat = Chat::with_parent_memory(node_id);
                        (Task::none(), Some(Box::new(Erased::new(chat))))
                    }
                    Err(err) => {
                        eprintln!("LLM error during memory selection: {}", err);
                        (Task::none(), Some(Box::new(Erased::new(Chat::default()))))
                    }
                }
            }
        }
    }

    fn view(&self) -> Element<'_, RecallMessage> {
        let memories_label = if self.memory_display.is_empty() {
            "No memories yet."
        } else {
            "Available memories:"
        };

        let memories_text = if self.memory_display.is_empty() {
            text(memories_label)
        } else {
            text(format!("{}\n\n{}", memories_label, self.memory_display))
        };

        column![
            scrollable(memories_text).height(Length::FillPortion(3)),
            text_editor(&self.question)
                .on_action(RecallMessage::TextAction)
                .height(Length::Fixed(80.0)),
            row![
                button(text("run"))
                    .on_press_maybe(if self.busy {
                        None
                    } else {
                        Some(RecallMessage::Run)
                    })
                    .width(Length::Shrink),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .padding(8)
        .into()
    }
}
