mod common;
use cardinal_syntax::*;
use common::*;

#[test]
fn and_optimizer_elides_empty_operands() {
    let expr = parse_ok("foo AND ");
    word_is(&expr, "foo");
}

#[test]
fn and_optimizer_returns_empty_when_all_operands_are_empty() {
    let expr = parse_ok(" AND ");
    assert!(is_empty(&expr));
}

#[test]
fn metadata_filters_move_after_non_metadata_terms() {
    let expr = parse_ok("dm:today foo dm:pastweek");
    let parts = as_and(&expr);
    word_is(&parts[0], "foo");
    filter_is_kind(&parts[1], &FilterKind::DateModified);
    filter_is_kind(&parts[2], &FilterKind::DateModified);
}

#[test]
fn metadata_filters_stay_in_place_when_already_at_tail() {
    let expr = parse_ok("foo dm:today dm:pastweek");
    let parts = as_and(&expr);
    word_is(&parts[0], "foo");
    filter_is_kind(&parts[1], &FilterKind::DateModified);
    filter_is_kind(&parts[2], &FilterKind::DateModified);
}

#[test]
fn or_optimizer_keeps_disjunction_without_empty_operands() {
    let expr = parse_ok("foo|bar");
    let parts = as_or(&expr);
    assert_eq!(parts.len(), 2);
    word_is(&parts[0], "foo");
    word_is(&parts[1], "bar");
}

#[test]
fn or_optimizer_collapses_when_empty_operand_present() {
    let expr = parse_ok("foo||bar");
    assert!(is_empty(&expr));
}
