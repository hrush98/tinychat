# Tinychat Project Plan

## Goal

Build a fast, lightweight terminal chat client for a headless Ubuntu inference box running a local model server. The first version is a chat-based router that selects an inference profile, streams output cleanly to the terminal, and exposes enough debug information to tune the model instead of guessing.

## Product Shape

`tinychat` is a local-first CLI focused on:

- low-overhead terminal chat
- deterministic profile routing
- inspectable request decisions
- easy iteration on prompts and sampler settings

The first release is intentionally narrow. It should make the upgraded model easy to probe, compare, and tune before tool execution, Hermes integration, or a secondary router model are introduced.

## Scope For v0.1

### In scope

- Rust CLI binary
- local config file
- in-memory chat session
- streaming assistant output in the terminal
- rules-based routing between `direct` and `reasoning`
- placeholder profile definitions for `tool` and `agent`
- slash commands for session control and debug visibility
- request timing and router reason output

### Out of scope

- persistent sessions
- tool execution runtime
- Hermes integration
- multi-model routing
- prompt summarization
- remote auth or hosted APIs

## Architecture

### Core modules

- `config`: load server and profile settings from TOML
- `profiles`: typed inference profile definitions
- `router`: deterministic profile selection with a reason string
- `client`: HTTP chat transport and streaming parser
- `session`: in-memory conversation history
- `ui`: terminal loop, slash commands, and rendering helpers

### Data flow

1. User enters a message in the terminal.
2. Router chooses a profile and records why.
3. Client builds a request using session history and selected profile.
4. Model server streams tokens back.
5. UI renders output incrementally and records latency metrics.
6. Session stores both user and assistant turns.

## Initial Protocol Assumption

The first implementation targets an OpenAI-compatible local chat endpoint. Default assumptions:

- `POST /v1/chat/completions`
- SSE-style streaming with `data:` frames
- assistant deltas at `choices[0].delta.content`

This boundary is intentionally isolated in `client` so it can be swapped if the live Llama ROCm server exposes a different shape.

## Profile Model

Each profile owns:

- system prompt
- temperature
- top_p
- max_tokens
- stream flag
- optional reasoning flag
- optional model override

Initial profiles:

- `direct`: concise, low-latency default
- `reasoning`: larger output budget and explicit chain-style instruction
- `tool`: reserved schema only
- `agent`: reserved schema only

## Routing Rules For v0.1

The first router is deterministic and transparent.

- explicit `/profile <name>` always wins
- prompts with planning, debugging, design, compare, or step-by-step language prefer `reasoning`
- short direct asks prefer `direct`
- anything else falls back to configured default

Every decision prints a reason when debug mode is enabled.

## CLI Commands

- `/help`
- `/quit`
- `/reset`
- `/profile`
- `/profile <name>`
- `/debug`
- `/debug on`
- `/debug off`

## Observability

Each request should surface:

- chosen profile
- router reason
- endpoint and effective model
- request duration
- first-token latency

This is required for prompt and parameter tuning.

## Milestones

### M1

- initialize repo
- add project plan
- scaffold core modules
- implement config loader

### M2

- implement OpenAI-compatible streaming client
- implement REPL and slash commands
- implement in-memory session and profile router

### M3

- add tests for router and config loading
- add README usage docs
- harden error reporting and metrics

### M4

- validate against the live local endpoint
- tune `direct` and `reasoning` profiles
- document protocol changes if the server differs

## Immediate Definition Of Done

The first useful checkpoint is:

- `cargo run`
- enter a prompt
- route to `direct` or `reasoning`
- stream a reply from the local server
- see why the router chose the profile

## Next After v0.1

- profile-specific stop sequences and penalties
- session persistence
- transcript logging
- tool runtime
- Hermes agent integration
- small model router
