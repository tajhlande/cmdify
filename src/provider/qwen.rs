thin_provider! {
    provider_name = "qwen",
    api_key_env = "QWEN_API_KEY",
    default_base_url = "https://dashscope-intl.aliyuncs.com/compatible-mode",
    default_model = "qwen3.5-flash",
    endpoint_path = "/v1/chat/completions",
    backend_mod = completions,
    backend_ty = CompletionsProvider,
}
