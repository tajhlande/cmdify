//! cmdify — turn natural language into shell commands with AI.
//!
//! Architecture: `cli` → `config` → `app` (overrides + safety gate) → `orchestrator`
//! → `provider` (LLM communication) ↔ `tools` (tool-use loop).
//! `prompt` assembles the system prompt from modular pieces; `safety` performs
//! semantic analysis on generated commands; `spinner` / `debug` / `logger` handle
//! UI, tracing, and history.

pub mod app;
pub mod cli;
pub mod config;
pub mod debug;
pub mod error;
pub mod logger;
pub mod orchestrator;
pub mod prompt;
#[macro_use]
pub mod provider;
pub mod safety;
pub mod spinner;
pub mod tools;
