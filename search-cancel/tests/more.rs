use search_cancel::CancellationToken;

#[test]
fn multiple_tokens_cancelled_independently() {
    let t1 = CancellationToken::new(1);
    assert!(!t1.is_cancelled());
    let t2 = CancellationToken::new(2);
    assert!(t1.is_cancelled());
    assert!(!t2.is_cancelled());
    let t3 = CancellationToken::new(3);
    assert!(t2.is_cancelled());
    assert!(!t3.is_cancelled());
}
