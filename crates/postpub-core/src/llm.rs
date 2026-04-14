use postpub_types::CustomLlmProvider;

pub const LLM_MAX_TOKENS_MIN: usize = 1;
pub const LLM_MAX_TOKENS_MAX: usize = 131_072;

pub fn normalize_llm_max_tokens(max_tokens: usize) -> usize {
    max_tokens.clamp(LLM_MAX_TOKENS_MIN, LLM_MAX_TOKENS_MAX)
}

pub fn repair_llm_provider(provider: &mut CustomLlmProvider) -> bool {
    let normalized = normalize_llm_max_tokens(provider.max_tokens);
    if provider.max_tokens == normalized {
        return false;
    }

    provider.max_tokens = normalized;
    true
}

#[cfg(test)]
mod tests {
    use super::{normalize_llm_max_tokens, LLM_MAX_TOKENS_MAX, LLM_MAX_TOKENS_MIN};

    #[test]
    fn clamps_llm_max_tokens_to_supported_range() {
        assert_eq!(normalize_llm_max_tokens(0), LLM_MAX_TOKENS_MIN);
        assert_eq!(normalize_llm_max_tokens(8_192), 8_192);
        assert_eq!(
            normalize_llm_max_tokens(LLM_MAX_TOKENS_MAX + 1),
            LLM_MAX_TOKENS_MAX
        );
    }
}
