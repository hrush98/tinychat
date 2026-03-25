use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::ServerConfig;
use crate::profiles::InferenceProfile;
use crate::session::ChatMessage;

#[derive(Debug)]
pub struct ResponseMetrics {
    pub started_at: Instant,
    pub first_token_at: Option<Instant>,
    pub finished_at: Instant,
}

impl ResponseMetrics {
    pub fn total_duration(&self) -> Duration {
        self.finished_at.duration_since(self.started_at)
    }

    pub fn first_token_latency(&self) -> Option<Duration> {
        self.first_token_at
            .map(|first| first.duration_since(self.started_at))
    }
}

#[derive(Debug)]
pub struct ChatResponse {
    pub content: String,
    pub metrics: ResponseMetrics,
    pub effective_model: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
    top_p: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    model: Option<String>,
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Debug, Default, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

pub struct ModelClient {
    http: Client,
    server: ServerConfig,
}

impl ModelClient {
    pub fn new(server: ServerConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(server.timeout_secs))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http, server })
    }

    pub async fn chat_streaming<F>(
        &self,
        profile: &InferenceProfile,
        messages: &[ChatMessage],
        mut on_token: F,
    ) -> Result<ChatResponse>
    where
        F: FnMut(&str),
    {
        let effective_model = profile
            .model
            .clone()
            .unwrap_or_else(|| self.server.default_model.clone());
        let url = format!(
            "{}/chat/completions",
            self.server.base_url.trim_end_matches('/')
        );
        let request = ChatCompletionRequest {
            model: &effective_model,
            messages,
            temperature: profile.temperature,
            top_p: profile.top_p,
            max_tokens: profile.max_tokens,
            stream: profile.stream,
        };

        let started_at = Instant::now();
        let response = self
            .http
            .post(url)
            .json(&request)
            .send()
            .await
            .context("request to model server failed")?
            .error_for_status()
            .context("model server returned an error response")?;

        let mut stream = response.bytes_stream();
        let mut content = String::new();
        let mut first_token_at = None;
        let mut seen_model = None;

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.context("failed while reading response stream")?;
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines() {
                let line = line.trim();
                if !line.starts_with("data:") {
                    continue;
                }
                let payload = line.trim_start_matches("data:").trim();
                if payload == "[DONE]" || payload.is_empty() {
                    continue;
                }
                let parsed: StreamChunk = serde_json::from_str(payload)
                    .with_context(|| format!("failed to parse streaming payload: {payload}"))?;
                if seen_model.is_none() {
                    seen_model = parsed.model.clone();
                }
                for choice in parsed.choices {
                    if let Some(delta) = choice.delta.content {
                        if first_token_at.is_none() {
                            first_token_at = Some(Instant::now());
                        }
                        on_token(&delta);
                        content.push_str(&delta);
                    }
                }
            }
        }

        let finished_at = Instant::now();
        Ok(ChatResponse {
            content,
            metrics: ResponseMetrics {
                started_at,
                first_token_at,
                finished_at,
            },
            effective_model: seen_model.unwrap_or(effective_model),
        })
    }
}
