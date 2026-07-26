use std::sync::Arc;

use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_probe::{EguiProbe, Probe};
use rig::{
    OneOrMany,
    agent::Text,
    completion::{Prompt, PromptError},
    message::{AssistantContent, Message, UserContent},
};
use tokio::sync::oneshot;

use crate::{llm::LLM, state::State};

#[derive(Default)]
pub struct Chat {
    pub messages: Vec<Message>,
    pub config: Config,
    pub response: Option<oneshot::Receiver<Result<String, PromptError>>>,
    pub markdown_cache: CommonMarkCache,
}

#[derive(Default, EguiProbe)]
pub struct Config {
    #[egui_probe(multiline)]
    pub message: String,
}

impl Chat {
    fn poll_response(&mut self) {
        let Some(response) = &mut self.response else {
            return;
        };
        let Ok(response) = response.try_recv() else {
            return;
        };
        match response {
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
        }
        self.response = None;
    }
}

impl State for Chat {
    fn ui_remainder(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_response();

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
                                CommonMarkViewer::new().show(ui, &mut self.markdown_cache, &text.text);
                            }
                        }
                    }
                }
            }
        });
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, llm: &Arc<LLM>) {
        Probe::new(&mut self.config).show(ui);
    }

    fn run(&mut self, llm: &Arc<LLM>) {
        if self.response.is_some() {
            return;
        }
        let &mut Chat {
            config: Config {
                ref mut message, ..
            },
            ref mut messages,
            ref mut response,
            ..
        } = self;
        let (sender, receiver) = oneshot::channel();
        *response = Some(receiver);
        {
            let llm = llm.clone();
            let message = message.clone();
            let messages = messages.clone();
            tokio::spawn(async move { sender.send(llm.prompt(message).history(messages).await) })
        };
        messages.push(Message::User {
            content: OneOrMany::one(UserContent::Text(Text {
                text: std::mem::take(message),
                ..Default::default()
            })),
        });
    }
}
