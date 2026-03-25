use std::io::{self, Write};

use anyhow::Result;

use crate::client::{ModelClient, StreamEvent};
use crate::config::AppConfig;
use crate::profiles::ProfileName;
use crate::router::choose_profile;
use crate::session::Session;

const BANNER: &str = r#" _______ _                  _           _   
|__   __(_)                | |         | |  
   | |   _ _ __  _   _  ___| |__   __ _| |_ 
   | |  | | '_ \| | | |/ __| '_ \ / _` | __|
   | |  | | | | | |_| | (__| | | | (_| | |_ 
   |_|  |_|_| |_|\__, |\___|_| |_|\__,_|\__|
                   __/ |                    
                  |___/                     "#;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const BLUE: &str = "\x1b[34m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const MAGENTA: &str = "\x1b[35m";
const RED: &str = "\x1b[31m";

pub async fn run_repl(config: AppConfig) -> Result<()> {
    let client = ModelClient::new(config.clone())?;
    let mut session = Session::new();
    let mut debug = config.app.debug;
    let mut profile_override: Option<ProfileName> = None;
    let mut show_trace = false;

    print_banner(
        &config,
        &client,
        debug,
        show_trace,
        profile_override.as_ref(),
    );

    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        print!("{}{BOLD}you>{} ", CYAN, RESET);
        io::stdout().flush()?;
        input.clear();
        if stdin.read_line(&mut input)? == 0 {
            println!();
            break;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        if handle_command(
            trimmed,
            &mut session,
            &mut debug,
            &mut show_trace,
            &mut profile_override,
            &config,
        )? {
            if trimmed == "/quit" {
                break;
            }
            continue;
        }

        let route = choose_profile(
            trimmed,
            profile_override.clone(),
            config.app.default_profile.clone(),
        );
        let profile = config.profile(&route.profile)?;
        if debug {
            println!(
                "{}{DIM}[router]{} profile={} reasoning={} prefer_thinking={} toggle_mode={} reason={}",
                YELLOW,
                RESET,
                route.profile,
                profile.reasoning,
                client
                    .resolve_thinking_preference(profile)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                client.thinking_toggle_mode_label(),
                route.reason
            );
        }

        session.push_user(trimmed.to_string());
        let request_messages = session.build_request_messages(&profile.system_prompt);

        let mut saw_reasoning = false;
        let mut printed_trace_header = false;
        let mut printed_assistant_header = false;

        let response = client
            .chat_streaming(profile, &request_messages, |event| match event {
                StreamEvent::Reasoning(token) => {
                    saw_reasoning = true;
                    if show_trace {
                        if !printed_trace_header {
                            println!("{}{BOLD}trace>{} ", MAGENTA, RESET);
                            printed_trace_header = true;
                        }
                        print!("{}{token}{}", MAGENTA, RESET);
                        let _ = io::stdout().flush();
                    }
                }
                StreamEvent::Content(token) => {
                    if show_trace && printed_trace_header && !printed_assistant_header {
                        println!();
                    }
                    if !printed_assistant_header {
                        print!("{}{BOLD}assistant>{} ", GREEN, RESET);
                        printed_assistant_header = true;
                    }
                    print!("{token}");
                    let _ = io::stdout().flush();
                }
            })
            .await;
        println!();

        match response {
            Ok(response) => {
                if saw_reasoning && !show_trace {
                    println!(
                        "{}{DIM}[trace]{} hidden; use /trace on to stream it",
                        MAGENTA, RESET
                    );
                }
                if debug {
                    let total_ms = response.metrics.total_duration().as_millis();
                    let first_reasoning_ms = response
                        .metrics
                        .first_reasoning_latency()
                        .map(|value| value.as_millis().to_string())
                        .unwrap_or_else(|| "n/a".to_string());
                    let first_content_ms = response
                        .metrics
                        .first_token_latency()
                        .map(|value| value.as_millis().to_string())
                        .unwrap_or_else(|| "n/a".to_string());
                    println!(
                        "{}{DIM}[metrics]{} model={} total_ms={} first_reasoning_ms={} first_content_ms={} reasoning_chars={} content_chars={}",
                        BLUE,
                        RESET,
                        response.effective_model,
                        total_ms,
                        first_reasoning_ms,
                        first_content_ms,
                        response.reasoning_content.len(),
                        response.content.len()
                    );
                }
                session.push_assistant(response.content);
            }
            Err(error) => {
                eprintln!("{}{BOLD}[error]{} {error:#}", RED, RESET);
            }
        }
    }

    Ok(())
}

fn print_banner(
    config: &AppConfig,
    client: &ModelClient,
    debug: bool,
    show_trace: bool,
    profile_override: Option<&ProfileName>,
) {
    println!("{}{BOLD}{BANNER}{}", CYAN, RESET);
    println!("{}{DIM}local and self-hosted model chat{}", CYAN, RESET);
    println!();
    println!(
        "{}{DIM}server={} backend={} model={} trace_field={} default_profile={} debug={} trace={} override={}{}",
        BLUE,
        config.server.base_url,
        client.backend_label(),
        config.server.default_model,
        client.trace_field_label(),
        config.app.default_profile,
        debug,
        if show_trace { "on" } else { "off" },
        profile_override.map(ProfileName::as_str).unwrap_or("auto"),
        RESET
    );
    println!("{}{DIM}type /help for commands{}", BLUE, RESET);
    println!(
        "{}{DIM}template={} toggle_mode={} trace_supported={}{}",
        BLUE,
        client.template_path_label(),
        client.thinking_toggle_mode_label(),
        client.supports_trace_stream(),
        RESET
    );
}

fn handle_command(
    input: &str,
    session: &mut Session,
    debug: &mut bool,
    show_trace: &mut bool,
    profile_override: &mut Option<ProfileName>,
    config: &AppConfig,
) -> Result<bool> {
    if !input.starts_with('/') {
        return Ok(false);
    }

    let mut parts = input.split_whitespace();
    let command = parts.next().unwrap_or_default();
    match command {
        "/help" => {
            println!("{}{BOLD}/help{}", CYAN, RESET);
            println!("{}{BOLD}/quit{}", CYAN, RESET);
            println!("{}{BOLD}/reset{}", CYAN, RESET);
            println!("{}{BOLD}/profile{}", CYAN, RESET);
            println!(
                "{}{BOLD}/profile <direct|reasoning|tool|agent>{}",
                CYAN, RESET
            );
            println!("{}{BOLD}/debug{}", CYAN, RESET);
            println!("{}{BOLD}/debug <on|off>{}", CYAN, RESET);
            println!("{}{BOLD}/trace{}", CYAN, RESET);
            println!("{}{BOLD}/trace <on|off>{}", CYAN, RESET);
        }
        "/quit" => return Ok(true),
        "/reset" => {
            session.reset();
            println!("{}{DIM}[session]{} reset", YELLOW, RESET);
        }
        "/profile" => {
            if let Some(value) = parts.next() {
                let profile = value.parse::<ProfileName>()?;
                *profile_override = Some(profile.clone());
                println!("{}{DIM}[profile]{} override={profile}", YELLOW, RESET);
            } else {
                let active = profile_override
                    .as_ref()
                    .map(ProfileName::as_str)
                    .unwrap_or("auto");
                println!(
                    "{}{DIM}[profile]{} active={} default={} available={}",
                    YELLOW,
                    RESET,
                    active,
                    config.app.default_profile,
                    config
                        .profiles
                        .keys()
                        .map(ProfileName::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
        "/debug" => match parts.next() {
            Some("on") => {
                *debug = true;
                println!("{}{DIM}[debug]{} on", YELLOW, RESET);
            }
            Some("off") => {
                *debug = false;
                println!("{}{DIM}[debug]{} off", YELLOW, RESET);
            }
            None => {
                *debug = !*debug;
                println!(
                    "{}{DIM}[debug]{} {}",
                    YELLOW,
                    RESET,
                    if *debug { "on" } else { "off" }
                );
            }
            Some(other) => {
                println!(
                    "{}{DIM}[debug]{} invalid value '{}' expected on|off",
                    RED, RESET, other
                );
            }
        },
        "/trace" | "/think" => match parts.next() {
            Some("on") => {
                *show_trace = true;
                println!("{}{DIM}[trace]{} on", MAGENTA, RESET);
            }
            Some("off") => {
                *show_trace = false;
                println!("{}{DIM}[trace]{} off", MAGENTA, RESET);
            }
            None => {
                *show_trace = !*show_trace;
                println!(
                    "{}{DIM}[trace]{} {}",
                    MAGENTA,
                    RESET,
                    if *show_trace { "on" } else { "off" }
                );
            }
            Some(other) => {
                println!(
                    "{}{DIM}[trace]{} invalid value '{}' expected on|off",
                    RED, RESET, other
                );
            }
        },
        _ => {
            println!("{}{DIM}[command]{} unknown command", RED, RESET);
        }
    }

    Ok(true)
}
