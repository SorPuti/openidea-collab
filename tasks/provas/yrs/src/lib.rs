#[cfg(test)]
mod tests {
    use yrs::{Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Text, Transact, Update};
    use yrs::updates::decoder::Decode;

    #[test]
    fn utf16_conta_surrogate_pair() {
        let doc = Doc::with_options(Options { offset_kind: OffsetKind::Utf16, ..Default::default() });
        let text = doc.get_or_insert_text("content");
        {
            let mut txn = doc.transact_mut();
            text.push(&mut txn, "A😀B");
            assert_eq!(text.len(&txn), 4);
        }
        assert_eq!(text.get_string(&doc.transact()), "A😀B");
    }

    #[test]
    fn duas_replicas_convergem_apos_edicao_concorrente() {
        let a = Doc::new();
        let b = Doc::new();
        let ta = a.get_or_insert_text("content");
        let tb = b.get_or_insert_text("content");
        ta.push(&mut a.transact_mut(), "Ana");
        tb.push(&mut b.transact_mut(), "Bia");
        let ua = a.transact().encode_state_as_update_v1(&StateVector::default());
        let ub = b.transact().encode_state_as_update_v1(&StateVector::default());
        a.transact_mut().apply_update(Update::decode_v1(&ub).unwrap()).unwrap();
        b.transact_mut().apply_update(Update::decode_v1(&ua).unwrap()).unwrap();
        let result = ta.get_string(&a.transact());
        assert_eq!(result, tb.get_string(&b.transact()));
        assert!(result.contains("Ana") && result.contains("Bia"));
    }
}
