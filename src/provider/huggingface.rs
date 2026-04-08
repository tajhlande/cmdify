thin_provider! {
    provider_name = "huggingface",
    api_key_env = "HUGGINGFACE_API_KEY",
    default_base_url = "https://router.huggingface.co/v1",
    default_model = "mistralai/Mistral-7B-Instruct-v0.3",
    endpoint_path = "/chat/completions",
    backend_mod = completions,
    backend_ty = CompletionsProvider,
}
