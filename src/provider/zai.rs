thin_provider! {
    provider_name = "zai",
    api_key_env = "ZAI_API_KEY",
    default_base_url = "https://api.z.ai/api/paas/v4",
    default_model = "glm-4",
    endpoint_path = "/chat/completions",
    backend_mod = completions,
    backend_ty = CompletionsProvider,
}
