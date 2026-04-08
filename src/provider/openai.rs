thin_provider! {
    provider_name = "openai",
    api_key_env = "OPENAI_API_KEY",
    default_base_url = "https://api.openai.com",
    default_model = "gpt-5",
    endpoint_path = "/v1/responses",
    backend_mod = responses,
    backend_ty = ResponsesProvider,
}
