use super::{SO_CONTEXT_BLOCK_END, SO_CONTEXT_BLOCK_START, remove_block_from_text};

#[test]
fn removes_managed_block_and_keeps_other_content() {
    let text = format!(
        "# Global Agent Instructions\n\n{}\n@/tmp/SO-CONTEXT.md\n{}\n\n@/tmp/OTHER.md\n",
        SO_CONTEXT_BLOCK_START, SO_CONTEXT_BLOCK_END
    );

    let cleaned = remove_block_from_text(&text);
    assert_eq!(cleaned, "# Global Agent Instructions\n\n@/tmp/OTHER.md\n");
}

#[test]
fn leaves_unmanaged_text_unchanged() {
    let text = "# Global Agent Instructions\n\n@/tmp/OTHER.md\n";
    assert_eq!(remove_block_from_text(text), text);
}
