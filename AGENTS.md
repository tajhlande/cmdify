# Agent Guidelines for the aicmd project

## General guidelines for the agent

- Stay inside the project directory. Do not take actions outside the project directory.
- Don't make git commits unless specifically asked.
- Ensure all tests pass before declaring a task completed.
- Maintain full test coverage for new code.
- Consider the requirements of the project and the architectural guidelines before making major changes.
- When the guidelines and existing project don't clearly show an architectural direction or preference for library, ask the user.
- Open design or architecture questions that the user needs to answer should be written to /agent-docs/QUESTIONS.md.  As those questions are answered, and the answers reflected in the design and implementation files, they should be removed fron the QUESTIONS.md doc.
- Do not run any scripts in the /examples directory. Those are to be run by human users only.

## Project Overview

- The `aicmd` project is a command line tool to help the user turn natural language requests into shell commands, with the assistance of LLM models. 
- The project is written in the Rust language, and uses standard Rust conventions.
- The built artifact should be a binary file, native to the platform for which it is built, with no externally linked libraries at runtime.

## Basic UI cycle

- `aicmd` should take all of the provided input parameters and assemble them into a string that becomes the input prompt to the LLM service.
- Using a system prompt built in at compile time, it should query the LLM to generate a shell command that matches the user's intent in the input prompt
- The command should be generated in the command output
- if `aicmd` is invoked with no input at all, print a help message with usage information and exit. 
- Generated commands should be compatible with both `bash` or `zsh`.

## LLM integration

- `aicmd` should work with local or remote models that are accessible through web APIs.It should work with any OpenAI compatible endpoints for `/completions` or `/responses`, or specifically with API configuration for the following providers: Anthropic, Google Gemini, OpenAI, Z.ai, Minimax, Qwen, Kimi, OpenRouter, HuggingFace, Mistral. 
- `aicmd` should present a tool to the model that allows the model to ask the user for additional information, if it is needed to generate a proper command. That question to the user should be formatted to be answerable in multiple choice form, such as "Y"/"N" or "A"/"B"/"C", or a similar list of single letter answers, that the user can provide on the input line, similar to Linux installation scripts needing choice input.
- `aicmd` should present a tool to the model that allows the model to discover what other commands are available on the command line, by search for specific commands, such as with `which $CMD`. 


## Configuration

- `aicmd` should be configured primarily through environment variables, with sensible defaults where possible.  This should include environment variables for API keys, API URLs, and other changes from default settings, for example:
    - `AICMD_PROVIDER_NAME` : required, values like `openai`, `completions`, `responses`, `anthropic`, etc 
    - `AICMD_MODEL_NAME` : required, the name of the model to use
    - `OPENAI_API_KEY` : the key to use for the `openai` provider
    - `AICMD_COMPLETIONS_KEY` : the key to use for the `completions` provider
    - `AICMD_RESPONSES_KEY` : the key to use for the `responses` provider
    - `AICMD_COMPLETIONS_URL` : the URL touse for the `completions` provider
    - `AICMD_RESPONSES_URL` : the URL touse for the `completions` provider

... and so on for all the providers

- `aicmd` should have command line options for a few specific behavior flags
  - `-q` or `--quiet` to turn off the question-answering tool
  - `-b` or `--blind` to turn off the command-finding tool
  - `-n` or `--no-tools` to turn off all tools

## Build process

## Deployments

## MCP tools

- Always use Context7 MCP when I need library or API documentation, code generation, setup, or configuration steps, without me having to explicitly ask to use Context7.

