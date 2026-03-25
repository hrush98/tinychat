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
    pub first_reasoning_at: Option<Instant>,
    pub first_content_at: Option<Instant>,
    pub finished_at: Instant,
}

impl ResponseMetrics {
    pub fn total_duration(&self) -> Duration {
        self.finished_at.duration_since(self.started_at)
    }

    pub fn first_reasoning_latency(&self) -> Option<Duration> {
        self.first_reasoning_at
            .map(|first| first.duration_since(self.started_at))
    }

    pub fn first_token_latency(&self) -> Option<Duration> {
        self.first_content_at
            .map(|first| first.duration_since(self.started_at))
    }
}

#[derive(Debug)]
pub struct ChatResponse {
    pub content: String,
    pub reasoning_content: String,
    pub metrics: ResponseMetrics,
    pub effective_model: String,
}

#[derive(Debug)]
pub enum StreamEvent<'a> {
    Reasoning(&'a str),
    Content(&'a str),
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
    top_p: f32,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_template_kwargs: Option<ChatTemplateKwargs>,
}

#[derive(Debug, Clone, Serialize)]
struct ChatTemplateKwargs {
    enable_thinking: bool,
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
    reasoning_content: Option<String>,
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
        mut on_event: F,
    ) -> Result<ChatResponse>
    where
        F: FnMut(StreamEvent<'_>),
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
            chat_template_kwargs: profile
                .enable_thinking
                .map(|enable_thinking| ChatTemplateKwargs { enable_thinking }),
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
        let mut reasoning_content = String::new();
        let mut first_reasoning_at = None;
        let mut first_content_at = None;
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
                    if let Some(delta) = choice.delta.reasoning_content {
                        if first_reasoning_at.is_none() {
                            first_reasoning_at = Some(Instant::now());
                        }
                        on_event(StreamEvent::Reasoning(&delta));
                        reasoning_content.push_str(&delta);
                    }
                    if let Some(delta) = choice.delta.content {
                        if first_content_at.is_none() {
                            first_content_at = Some(Instant::now());
                        }
                        on_event(StreamEvent::Content(&delta));
                        content.push_str(&delta);
                    }
                }
            }
        }

        let finished_at = Instant::now();
        Ok(ChatResponse {
            content,
            reasoning_content,
            metrics: ResponseMetrics {
                started_at,
                first_reasoning_at,
                first_content_at,
                finished_at,
            },
            effective_model: seen_model.unwrap_or(effective_model),
        })
    }
}
