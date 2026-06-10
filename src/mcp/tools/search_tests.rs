use super::{
    build_search_structured_content, count_search_results, line_matches_terms,
    parse_fallback_terms,
};

#[test]
fn no_results_count_as_zero() {
    assert_eq!(count_search_results("No results."), 0);
}

#[test]
fn search_result_metadata_marks_limit_saturation_as_partial() {
    let content = build_search_structured_content("foo", "/tmp/project", 2, 2, "graph_fts");

    assert_eq!(content["content_kind"], "search_results");
    assert_eq!(content["shown_result_count"], 2);
    assert_eq!(content["result_complete"], false);
    assert_eq!(content["may_have_more_results"], true);
}

#[test]
fn fallback_terms_support_or_queries() {
    assert_eq!(
        parse_fallback_terms("\"foo\" OR bar"),
        vec!["foo".to_string(), "bar".to_string()]
    );
}

#[test]
fn fallback_line_matching_is_case_sensitive() {
    assert!(line_matches_terms(
        "ConfigRepository loads recommendation files",
        &["ConfigRepository".to_string()]
    ));
    assert!(!line_matches_terms(
        "ConfigRepository loads recommendation files",
        &["configrepository".to_string()]
    ));
}
