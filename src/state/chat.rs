use std::mem;

use eframe::egui::{Response, TextEdit, Ui, vec2};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_probe::{EguiProbe, Probe, Style};
use rig::{
    OneOrMany,
    agent::Text,
    completion::{Prompt, PromptError},
    message::{AssistantContent, Message, UserContent},
};
use tokio::sync::mpsc;

use crate::{
    App,
    memory::{self, Memory, SUMMARY_PROMPT},
    state::State,
};

pub struct Chat {
    pub messages: Vec<Message>,
    pub config: Config,
    pub receiver: mpsc::Receiver<TaskResult>,
    pub sender: mpsc::Sender<TaskResult>,
    pub markdown_cache: CommonMarkCache,
    pub parent_memory: Option<memory::NodeId>,
    pub busy: bool,
}

impl Default for Chat {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel(1);
        Self {
            messages: Default::default(),
            config: Default::default(),
            receiver,
            sender,
            markdown_cache: Default::default(),
            parent_memory: Default::default(),
            busy: false,
        }
    }
}

pub enum TaskResult {
    Message(Result<String, PromptError>),
    Summary(Result<String, PromptError>),
}

#[derive(EguiProbe)]
#[egui_probe(tags inlined)]
pub enum Config {
    Message(MessageConfig),
    Summary,
}

impl Default for Config {
    fn default() -> Self {
        Self::Message(MessageConfig::default())
    }
}

#[derive(Default, EguiProbe)]
#[egui_probe(transparent)]
pub struct MessageConfig {
    #[egui_probe(with Self::ui_message)]
    pub message: String,
}

impl MessageConfig {
    fn ui_message(message: &mut String, ui: &mut Ui, _: &Style) -> Response {
        ui.add_sized(
            vec2(ui.available_width(), 0.0),
            TextEdit::multiline(message),
        )
    }
}

impl Chat {
    fn context<'a>(
        parent_memory: Option<memory::NodeId>,
        messages: &'a Vec<Message>,
        app: &'a App,
    ) -> impl Iterator<Item = Message> {
        app.memory_manager
            .messages(parent_memory)
            .chain(messages.iter().cloned())
    }
}

impl State for Chat {
    fn ui_remainder(&mut self, ui: &mut eframe::egui::Ui) {
        eframe::egui::ScrollArea::vertical().show(ui, |ui| {
            for message in &self.messages {
                match message {
                    Message::User { content } => {
                        ui.label("You");
                        for item in content.iter() {
                            if let UserContent::Text(text) = item {
                                ui.label(&text.text);
                            }
                        }
                    }
                    Message::System { content } => {
                        ui.label("System");
                        ui.label(content);
                    }
                    Message::Assistant { content, .. } => {
                        ui.label("Liana");
                        for item in content.iter() {
                            if let AssistantContent::Text(text) = item {
                                CommonMarkViewer::new().show(
                                    ui,
                                    &mut self.markdown_cache,
                                    &text.text,
                                );
                            }
                        }
                    }
                }
            }
        });
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) {
        Probe::new(&mut self.config).show(ui);
    }

    fn run(&mut self, app: &mut App) {
        if self.busy {
            return;
        }
        self.busy = true;
        let &mut Chat {
            ref mut config,
            ref mut messages,
            ref sender,
            ..
        } = self;
        match config {
            Config::Message(config) => {
                let message = &mut config.message;
                {
                    let llm = app.llm.clone();
                    let message = message.clone();
                    let messages =
                        Self::context(self.parent_memory, messages, app).collect::<Vec<_>>();
                    let sender = sender.clone();
                    tokio::spawn(async move {
                        sender
                            .send(TaskResult::Message(
                                llm.prompt(message).history(messages).await,
                            ))
                            .await
                            .unwrap();
                    })
                };
                messages.push(Message::User {
                    content: OneOrMany::one(UserContent::Text(Text {
                        text: std::mem::take(message),
                        ..Default::default()
                    })),
                });
            }
            Config::Summary => {
                let message = SUMMARY_PROMPT;
                {
                    let llm = app.llm.clone();
                    let messages =
                        Self::context(self.parent_memory, messages, app).collect::<Vec<_>>();
                    let sender = sender.clone();
                    tokio::spawn(async move {
                        sender
                            .send(TaskResult::Message(
                                llm.prompt(message).history(messages).await,
                            ))
                            .await
                            .unwrap();
                    })
                };
                messages.push(Message::User {
                    content: OneOrMany::one(UserContent::Text(Text {
                        text: message.to_string(),
                        ..Default::default()
                    })),
                });
            }
        };
    }

    fn poll(&mut self, app: &mut App) {
        while let Ok(result) = self.receiver.try_recv() {
            self.busy = false;
            match result {
                TaskResult::Message(response) => match response {
                    Ok(response) => {
                        self.messages.push(Message::Assistant {
                            id: None,
                            content: OneOrMany::one(AssistantContent::Text(Text {
                                text: response,
                                ..Default::default()
                            })),
                        });
                    }
                    Err(err) => {
                        println!("{}", err)
                    }
                },
                TaskResult::Summary(response) => match response {
                    Ok(response) => {
                        app.memory_manager.add_memory(
                            Memory::new(mem::take(&mut self.messages), response),
                            self.parent_memory,
                        );
                    }
                    Err(err) => {
                        println!("{}", err)
                    }
                },
            }
        }
    }
}
