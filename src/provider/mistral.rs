thin_provider! {
    provider_name = "mistral",
    api_key_env = "MISTRAL_API_KEY",
    default_base_url = "https://api.mistral.ai",
    default_model = "ministral-14b-2512",
    endpoint_path = "/v1/chat/completions",
    backend_mod = completions,
    backend_ty = CompletionsProvider,
}
