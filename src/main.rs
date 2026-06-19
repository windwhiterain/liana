use std::io::{self, Write};
use std::iter::once;

use crossterm::event::{Event, KeyCode, KeyEventKind, read};
use crossterm::terminal;
use liana::memory::{
    DisplayMemories, MEMORY_DESCRIBE_PROMPT, Memory, MemoryIterator, SELECT_MEMORY_PROMPT,
    SYSTEM_PROMPT,
};

use liana::config::Config;
use liana::{
    api::{self, Message},
    memory::Manager,
};

#[tokio::main]
async fn main() {
    println!("Liana Agent {}", env!("CARGO_PKG_VERSION"));
    let config = Config::load().unwrap_or_else(|| Config::setup());
    let mut memory_manager = Manager::new();
    loop {
        print!("user: ");
        io::stdout().flush().ok();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        let trimmed = input.trim();
        let input = if trimmed.is_empty() {
            println!("{{");
            let input = read_multiline();
            println!("}}");
            input
        } else {
            trimmed.to_string()
        };

        respond(&config, &mut memory_manager, input).await;
        println!();
    }
}

/// Read characters one-by-one until ESC is pressed.
/// Each Enter inserts a real newline into the buffer.
fn read_multiline() -> String {
    terminal::enable_raw_mode().expect("raw mode");
    let mut buf = String::new();

    loop {
        match read().expect("read event") {
            Event::Key(ke) if ke.kind == KeyEventKind::Press => match ke.code {
                KeyCode::Esc => break,
                KeyCode::Enter => {
                    buf.push('\n');
                    print!("\r\n");
                }
                KeyCode::Backspace => {
                    buf.pop();
                    print!("\x08 \x08");
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    print!("{c}");
                }
                _ => {}
            },
            _ => {}
        }
        io::stdout().flush().ok();
    }

    terminal::disable_raw_mode().expect("cooked mode");
    println!(); // move past the raw-mode line
    buf
}

async fn respond(config: &Config, manager: &mut Manager, input: String) {
    let user_message = Message {
        role: "user".to_string(),
        content: input.clone(),
    };

    let (node, theory_cache, memory_sparsity) = if manager.memories.len() > 0 {
        println!("agent: select memories from:\n");
        print!("{}", manager.display_memories());
        let mut selected_memories = api::chat_completion_stream(
            &config,
            manager.messages(manager.last_node()).into_iter().chain([
                &user_message,
                &Message {
                    role: "user".to_string(),
                    content: format!(
                        "{}\nmemory list:\n{}",
                        SELECT_MEMORY_PROMPT,
                        manager.display_memories()
                    ),
                },
            ]),
            None,
            Some(api::ResponseFormat::json()),
        )
        .await;

        let mut content = String::new();
        let mut in_reasoning = false;

        while let Some(event) = selected_memories.recv().await {
            match event {
                api::StreamEvent::Reasoning(text) => {
                    if !in_reasoning {
                        print!("assistant: reasoning:\n\x1b[2m");
                        in_reasoning = true;
                    }
                    print!("{text}");
                }
                api::StreamEvent::Content(text) => {
                    if in_reasoning {
                        print!("\x1b[22m");
                        println!();
                        in_reasoning = false;
                    }
                    content.push_str(&text);
                }
                api::StreamEvent::Done(u) => {
                    if in_reasoning {
                        print!("\x1b[22m");
                        println!();
                    }
                    break;
                }
                api::StreamEvent::Error(e) => {
                    if in_reasoning {
                        print!("\x1b[22m");
                    }
                    eprintln!("\nerror: {e}");
                    return;
                }
            }
        }
        io::stdout().flush().ok();

        let selected_memories: Vec<usize> = serde_json::from_str(&content).unwrap();
        print!(
            "assistant: selected memories:\n{}",
            DisplayMemories(selected_memories.iter().map(|x| (*x, &manager.memories[*x])))
        );
        // println!("manager: {:#?}", manager);
        manager.find(&selected_memories)
    } else {
        (None, None, None)
    };

    println!("agent: memory chain:");
    for memory in (MemoryIterator { manager, node }) {
        println!("- {}", memory.summary)
    }

    // println!("find node: {:#?}", node);
    // println!("manager: {:#?}", manager);

    let mut answer = api::chat_completion_stream(
        &config,
        once(&Message {
            role: "system".to_string(),
            content: SYSTEM_PROMPT.to_string(),
        })
        .chain(manager.messages(node))
        .chain(once(&user_message)),
        None,
        None,
    )
    .await;

    let mut content = String::new();
    let mut in_reasoning = false;
    let mut usage: Option<api::Usage> = None;

    while let Some(event) = answer.recv().await {
        match event {
            api::StreamEvent::Reasoning(text) => {
                if !in_reasoning {
                    print!("assistant: reasoning:\n\x1b[2m");
                    in_reasoning = true;
                }
                print!("{text}");
            }
            api::StreamEvent::Content(text) => {
                if in_reasoning {
                    println!("\x1b[22m");
                    println!("assistant:");
                    in_reasoning = false;
                }
                print!("{text}");
                content.push_str(&text);
            }
            api::StreamEvent::Done(u) => {
                if in_reasoning {
                    print!("\x1b[22m");
                    println!();
                }
                usage = u;
                break;
            }
            api::StreamEvent::Error(e) => {
                if in_reasoning {
                    print!("\x1b[22m");
                }
                eprintln!("\nerror: {e}");
                return;
            }
        }
    }
    io::stdout().flush().ok();
    if !content.ends_with('\n') {
        println!()
    }

    let mut size = 0;

    if let Some(ref usage) = usage {
        let (hit, total) = usage.cache_hit();
        if total > 0 {
            let cache = (hit as f64 / total as f64) * 100.0;
            let theory_cache = theory_cache.unwrap_or(0.0);
            let memory_sparsity = memory_sparsity.unwrap_or(0.0);
            eprintln!(
                "{{cache: {cache:.0}% (theory: {theory_cache:.0}%), context: {total} (sparsity: {memory_sparsity:.0}%), output: {}}}",
                usage.completion_tokens
            );
        }
        size += usage.completion_tokens;
    }

    let assistant_message = Message {
        role: "assistant".to_string(),
        content: content.trim().to_string(),
    };

    println!("agent: summarize memory:");
    let mut memory_summery = api::chat_completion_stream(
        &config,
        once(&Message {
            role: "system".to_string(),
            content: SYSTEM_PROMPT.to_string(),
        })
        .chain(manager.messages(node))
        .chain([
            &user_message,
            &assistant_message,
            &Message {
                role: "user".to_string(),
                content: MEMORY_DESCRIBE_PROMPT.to_string(),
            },
        ]),
        Some(false),
        None,
    )
    .await;

    let mut content = String::new();
    let mut in_reasoning = false;
    let mut usage: Option<api::Usage> = None;

    while let Some(event) = memory_summery.recv().await {
        match event {
            api::StreamEvent::Reasoning(text) => {
                if !in_reasoning {
                    print!("assistant: reasoning:\n\x1b[2m");
                    in_reasoning = true;
                }
                print!("{text}");
            }
            api::StreamEvent::Content(text) => {
                if in_reasoning {
                    println!("\x1b[22m");
                    println!("assistant: summarize memory:");
                    in_reasoning = false;
                }
                print!("{text}");
                content.push_str(&text);
            }
            api::StreamEvent::Done(u) => {
                if in_reasoning {
                    print!("\x1b[22m");
                    println!();
                }
                usage = u;
                break;
            }
            api::StreamEvent::Error(e) => {
                if in_reasoning {
                    print!("\x1b[22m");
                }
                eprintln!("\nerror: {e}");
                return;
            }
        }
    }
    io::stdout().flush().ok();
    if !content.ends_with('\n') {
        println!()
    }

    if let Some(ref usage) = usage {
        let (hit, total) = usage.cache_hit();
        if total > 0 {
            let hit_rate = (hit as f64 / total as f64) * 100.0;
            eprintln!(
                "[cache: {:.0}% context: {total} output: {}]",
                hit_rate, usage.completion_tokens
            );
        }
        size += usage.completion_tokens;
    }

    let last_memory = manager.add_memory(
        Memory::new(
            vec![user_message, assistant_message],
            content,
            size as usize,
        ),
        node,
    );

    manager.last_memory = Some(last_memory);
}
