# tinychat

`tinychat` is a lightweight terminal chat router and tuning workbench for local and self-hosted language models.

## Current status

`v0.1` is a narrow workbench for tuning a local chat model:

- OpenAI-compatible chat client
- streaming terminal output
- rules-based routing between `direct` and `reasoning`
- placeholder `tool` and `agent` profiles
- debug output for router decisions and latency
- optional streamed reasoning trace display
- manual backend and model compatibility configuration

## Quick start

1. Copy `config/tinychat.example.toml` to `config/tinychat.toml`.
2. Set the `server`, `backend`, and `model` sections for your setup.
3. Run `cargo run -- --config config/tinychat.toml`.

## Commands

- `/help`
- `/quit`
- `/reset`
- `/profile`
- `/profile <name>`
- `/debug`
- `/debug on`
- `/debug off`
- `/trace`
- `/trace on`
- `/trace off`

## Config Model

`tinychat` now separates configuration into four concerns:

- `server`: where the backend lives
- `backend`: how the server speaks
- `model`: reasoning and compatibility behavior
- `profiles`: user intent such as `direct` or `reasoning`

## Protocol

The initial client assumes an OpenAI-compatible endpoint:

- `POST /v1/chat/completions`
- streaming enabled
- server-sent event frames using `data:`
- optional reasoning deltas via a configurable reasoning field

If your backend differs, adjust the compatibility config first and the transport code second.

## Roadmap

Planned future work includes model-template awareness, where `tinychat` can inspect or track each model's chat template, infer model-specific prompt behavior, and detect when that template changes after an upgrade. That is intentionally deferred until the core local chat and profile-tuning loop is stable.
