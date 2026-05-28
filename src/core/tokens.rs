//! Canonical token counting for so-context metrics.
//!
//! We use a single tokenizer across all agents/tools so savings statistics are
//! stable and comparable over time. The chosen encoding is `cl100k_base`, which
//! is widely used for code/text workloads and much closer to real model token
//! counts than a chars/4 heuristic.

use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();

fn tokenizer() -> &'static CoreBPE {
    TOKENIZER.get_or_init(|| {
        tiktoken_rs::cl100k_base().expect("failed to initialize cl100k_base tokenizer")
    })
}

/// Count tokens using the canonical `cl100k_base` tokenizer.
pub fn count_tokens(text: &str) -> i64 {
    tokenizer().encode_with_special_tokens(text).len() as i64
}
