thin_provider! {
    provider_name = "openrouter",
    api_key_env = "OPENROUTER_API_KEY",
    default_base_url = "https://openrouter.ai/api",
    default_model = "openai/gpt-4",
    endpoint_path = "/v1/chat/completions",
    backend_mod = completions,
    backend_ty = CompletionsProvider,
}
