use openidea_collab_core::SharedDocument;

#[test]
fn merged_updates_preserve_out_of_order_causal_dependencies() {
    let author = SharedDocument::new();
    let first = author.edit(0, 0, "A").unwrap();
    let second = author.edit(1, 1, "B").unwrap();
    let merged = openidea_collab_core::merge_updates(&second, &first).unwrap();
    let reader = SharedDocument::new();
    reader.apply(&merged).unwrap();
    assert_eq!(reader.text(), "AB");
}

#[test]
fn edits_use_utf16_offsets_and_reject_split_surrogates() {
    let doc = SharedDocument::new();
    doc.edit(0, 0, "A😀B").unwrap();
    assert_eq!(doc.text(), "A😀B");
    assert!(doc.edit(2, 2, "!").is_err());
    assert_eq!(doc.text(), "A😀B");
    doc.edit(3, 4, "C").unwrap();
    assert_eq!(doc.text(), "A😀C");
}

#[test]
fn clients_merge_independent_edits_against_the_same_baseline() {
    let first = SharedDocument::new();
    let seed = first.edit(0, 0, "ab").unwrap();
    let second = SharedDocument::new();
    second.apply(&seed).unwrap();
    let a = first.edit(1, 1, "X").unwrap();
    let b = second.edit(1, 1, "Y").unwrap();
    first.apply(&b).unwrap();
    second.apply(&a).unwrap();
    assert_eq!(first.text(), second.text());
    assert!(first.text().contains('X') && first.text().contains('Y'));
}
