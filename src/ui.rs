use std::io::{self, Write};

use anyhow::Result;

use crate::client::ModelClient;
use crate::config::AppConfig;
use crate::profiles::ProfileName;
use crate::router::choose_profile;
use crate::session::Session;

pub async fn run_repl(config: AppConfig) -> Result<()> {
    let client = ModelClient::new(config.server.clone())?;
    let mut session = Session::new();
    let mut debug = config.app.debug;
    let mut profile_override: Option<ProfileName> = None;

    print_banner(&config, debug, profile_override.as_ref());

    let stdin = io::stdin();
    let mut input = String::new();

    loop {
        print!("you> ");
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
                "[router] profile={} reasoning={} reason={}",
                route.profile, profile.reasoning, route.reason
            );
        }

        session.push_user(trimmed.to_string());
        let request_messages = session.build_request_messages(&profile.system_prompt);

        print!("assistant> ");
        io::stdout().flush()?;
        let response = client
            .chat_streaming(profile, &request_messages, |token| {
                print!("{token}");
                let _ = io::stdout().flush();
            })
            .await;
        println!();

        match response {
            Ok(response) => {
                if debug {
                    let total_ms = response.metrics.total_duration().as_millis();
                    let first_ms = response
                        .metrics
                        .first_token_latency()
                        .map(|value| value.as_millis().to_string())
                        .unwrap_or_else(|| "n/a".to_string());
                    println!(
                        "[metrics] model={} total_ms={} first_token_ms={}",
                        response.effective_model, total_ms, first_ms
                    );
                }
                session.push_assistant(response.content);
            }
            Err(error) => {
                eprintln!("[error] {error:#}");
            }
        }
    }

    Ok(())
}

fn print_banner(config: &AppConfig, debug: bool, profile_override: Option<&ProfileName>) {
    println!("tinychat");
    println!(
        "server={} model={} default_profile={} debug={} override={}",
        config.server.base_url,
        config.server.default_model,
        config.app.default_profile,
        debug,
        profile_override.map(ProfileName::as_str).unwrap_or("auto")
    );
    println!("type /help for commands");
}

fn handle_command(
    input: &str,
    session: &mut Session,
    debug: &mut bool,
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
            println!("/help");
            println!("/quit");
            println!("/reset");
            println!("/profile");
            println!("/profile <direct|reasoning|tool|agent>");
            println!("/debug");
            println!("/debug <on|off>");
        }
        "/quit" => return Ok(true),
        "/reset" => {
            session.reset();
            println!("[session] reset");
        }
        "/profile" => {
            if let Some(value) = parts.next() {
                let profile = value.parse::<ProfileName>()?;
                *profile_override = Some(profile.clone());
                println!("[profile] override={profile}");
            } else {
                let active = profile_override
                    .as_ref()
                    .map(ProfileName::as_str)
                    .unwrap_or("auto");
                println!(
                    "[profile] active={} default={} available={}",
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
                println!("[debug] on");
            }
            Some("off") => {
                *debug = false;
                println!("[debug] off");
            }
            None => {
                *debug = !*debug;
                println!("[debug] {}", if *debug { "on" } else { "off" });
            }
            Some(other) => {
                println!("[debug] invalid value '{other}', expected on|off");
            }
        },
        _ => {
            println!("[command] unknown command");
        }
    }

    Ok(true)
}
