thin_provider! {
    provider_name = "kimi",
    api_key_env = "KIMI_API_KEY",
    default_base_url = "https://api.moonshot.ai",
    default_model = "kimi-k2.5",
    endpoint_path = "/v1/chat/completions",
    backend_mod = completions,
    backend_ty = CompletionsProvider,
}
