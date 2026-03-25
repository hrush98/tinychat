use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

use crate::config::{AppConfig, BackendFlavor, BackendType};
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

pub struct ModelClient {
    http: Client,
    config: AppConfig,
}

impl ModelClient {
    pub fn new(config: AppConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.server.timeout_secs))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http, config })
    }

    pub fn resolve_thinking_preference(&self, profile: &InferenceProfile) -> Option<bool> {
        profile.prefer_thinking
    }

    pub fn supports_trace_stream(&self) -> bool {
        self.config.model.supports_reasoning && self.config.model.reasoning_field.is_some()
    }

    pub fn trace_field_label(&self) -> &str {
        self.config
            .model
            .reasoning_field
            .as_deref()
            .unwrap_or("none")
    }

    pub fn template_path_label(&self) -> &str {
        self.config
            .model
            .chat_template_path
            .as_deref()
            .unwrap_or("none")
    }

    pub fn thinking_toggle_mode_label(&self) -> &'static str {
        match self.config.model.thinking_toggle_path.as_deref() {
            Some("chat_template_kwargs.enable_thinking") => "chat_template_kwargs.enable_thinking",
            Some(_) => "unsupported",
            None => "unavailable",
        }
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
            .unwrap_or_else(|| self.config.server.default_model.clone());
        let url = format!(
            "{}/chat/completions",
            self.config.server.base_url.trim_end_matches('/')
        );
        let request = ChatCompletionRequest {
            model: &effective_model,
            messages,
            temperature: profile.temperature,
            top_p: profile.top_p,
            max_tokens: profile.max_tokens,
            stream: profile.stream,
            chat_template_kwargs: self.build_chat_template_kwargs(profile),
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
                let parsed: Value = serde_json::from_str(payload)
                    .with_context(|| format!("failed to parse streaming payload: {payload}"))?;
                if seen_model.is_none() {
                    seen_model = parsed
                        .get("model")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                }

                if let Some(choices) = parsed.get("choices").and_then(Value::as_array) {
                    for choice in choices {
                        let Some(delta) = choice.get("delta") else {
                            continue;
                        };
                        if let Some(reasoning_delta) = self.extract_reasoning_delta(delta) {
                            if first_reasoning_at.is_none() {
                                first_reasoning_at = Some(Instant::now());
                            }
                            on_event(StreamEvent::Reasoning(&reasoning_delta));
                            reasoning_content.push_str(&reasoning_delta);
                        }
                        if let Some(content_delta) = delta.get("content").and_then(Value::as_str) {
                            if first_content_at.is_none() {
                                first_content_at = Some(Instant::now());
                            }
                            on_event(StreamEvent::Content(content_delta));
                            content.push_str(content_delta);
                        }
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

    fn build_chat_template_kwargs(&self, profile: &InferenceProfile) -> Option<ChatTemplateKwargs> {
        if !self.config.model.supports_thinking_toggle {
            return None;
        }

        let prefer_thinking = profile.prefer_thinking?;
        match self.config.model.thinking_toggle_path.as_deref() {
            Some("chat_template_kwargs.enable_thinking") => Some(ChatTemplateKwargs {
                enable_thinking: prefer_thinking,
            }),
            _ => None,
        }
    }

    fn extract_reasoning_delta(&self, delta: &Value) -> Option<String> {
        if !self.config.model.supports_reasoning {
            return None;
        }

        let field = self.config.model.reasoning_field.as_deref()?;
        delta
            .get(field)
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    }

    pub fn backend_label(&self) -> &'static str {
        match (
            &self.config.backend.backend_type,
            &self.config.backend.flavor,
        ) {
            (BackendType::OpenAiCompatible, BackendFlavor::Generic) => "openai_compatible/generic",
            (BackendType::OpenAiCompatible, BackendFlavor::Llamacpp) => {
                "openai_compatible/llamacpp"
            }
        }
    }
}
