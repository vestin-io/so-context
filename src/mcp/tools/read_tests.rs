use rmcp::model::JsonObject;

use super::{
    ExcerptRequest, ReadStructuredContent, build_read_structured_content, parse_excerpt_request,
    render_excerpt,
};

#[test]
fn full_read_is_marked_authoritative() {
    let content = build_read_structured_content(&ReadStructuredContent {
        path: "/tmp/example.rs",
        mode: "full",
        line_count: 42,
        current_text_is_authoritative: true,
        current_text_contains_full_file: true,
        cached_notice_only: false,
        excerpt_range: None,
        line_numbers: false,
    });

    assert_eq!(content["content_kind"], "file_content");
    assert_eq!(content["current_text_is_authoritative"], true);
    assert_eq!(content["current_text_contains_full_file"], true);
    assert_eq!(content["result_complete"], true);
}

#[test]
fn cached_notice_is_not_marked_as_full_file_text() {
    let content = build_read_structured_content(&ReadStructuredContent {
        path: "/tmp/example.rs",
        mode: "full",
        line_count: 42,
        current_text_is_authoritative: false,
        current_text_contains_full_file: false,
        cached_notice_only: true,
        excerpt_range: None,
        line_numbers: false,
    });

    assert_eq!(content["content_kind"], "cached_read_notice");
    assert_eq!(content["current_text_is_authoritative"], false);
    assert_eq!(content["cached_notice_only"], true);
    assert_eq!(content["result_complete"], false);
}

#[test]
fn excerpt_render_can_include_line_numbers() {
    let excerpt = ExcerptRequest {
        start_line: 2,
        end_line: 3,
        line_numbers: true,
    };

    assert_eq!(render_excerpt("a\nb\nc\nd", &excerpt), "2: b\n3: c");
}

#[test]
fn parse_excerpt_defaults_end_line_to_start_line() {
    let args: JsonObject = serde_json::json!({ "start_line": 5 })
        .as_object()
        .cloned()
        .unwrap();
    let excerpt = parse_excerpt_request(&args)
        .expect("parse")
        .expect("excerpt");

    assert_eq!(excerpt.start_line, 5);
    assert_eq!(excerpt.end_line, 5);
    assert!(!excerpt.line_numbers);
}
