use collab_server::collaboration::SharedDocument;
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

#[test]
fn concurrent_updates_converge_after_duplicate_delivery() {
    let a = Doc::new();
    let b = Doc::new();
    let text_a = a.get_or_insert_text("content");
    let text_b = b.get_or_insert_text("content");
    text_a.push(&mut a.transact_mut(), "Olá ");
    text_b.push(&mut b.transact_mut(), "🌍");
    let update_a = a
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let update_b = b
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let shared = SharedDocument::new();
    shared.apply(&update_b).unwrap();
    shared.apply(&update_a).unwrap();
    shared.apply(&update_a).unwrap();
    let full = shared.diff(None).unwrap();
    a.transact_mut()
        .apply_update(Update::decode_v1(&full).unwrap())
        .unwrap();
    b.transact_mut()
        .apply_update(Update::decode_v1(&full).unwrap())
        .unwrap();
    let result = text_a.get_string(&a.transact());
    assert_eq!(result, text_b.get_string(&b.transact()));
    assert!(result.contains("Olá ") && result.contains('🌍'));
}

#[test]
fn invalid_update_does_not_modify_document() {
    let shared = SharedDocument::new();
    let before = shared.diff(None).unwrap();
    assert!(shared.apply(&[0xff, 0xff]).is_err());
    assert_eq!(shared.diff(None).unwrap(), before);
}
