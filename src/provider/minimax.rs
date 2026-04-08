thin_provider! {
    provider_name = "minimax",
    api_key_env = "MINIMAX_API_KEY",
    default_base_url = "https://api.minimax.io",
    default_model = "MiniMax-M2.7",
    endpoint_path = "/v1/chat/completions",
    backend_mod = completions,
    backend_ty = CompletionsProvider,
}
