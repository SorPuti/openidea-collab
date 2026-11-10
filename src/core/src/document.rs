use openidea_collab_protocol::MAX_TRANSFER;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use uuid::Uuid;
use yrs::{
    Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Text, Transact, Update,
    updates::decoder::Decode,
};

pub const MAX_TEXT: usize = 1024 * 1024;
pub const MAX_OUTBOX: usize = 8 * 1024 * 1024;
const MAX_OPERATIONS: usize = 256;
pub struct Operation {
    pub id: Uuid,
    pub update: Vec<u8>,
}
pub struct Document {
    doc: Doc,
    pending: VecDeque<Operation>,
    generation: u64,
}
fn empty() -> Doc {
    Doc::with_options(Options {
        offset_kind: OffsetKind::Utf16,
        ..Default::default()
    })
}
fn content(doc: &Doc) -> String {
    doc.get_or_insert_text("content")
        .get_string(&doc.transact())
}
fn snapshot(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}
fn apply(doc: &Doc, update: &[u8]) -> Result<(), &'static str> {
    if update.len() > MAX_TRANSFER {
        return Err("update too large");
    }
    doc.transact_mut()
        .apply_update(Update::decode_v1(update).map_err(|_| "invalid CRDT update")?)
        .map_err(|_| "invalid CRDT update")
}
impl Default for Document {
    fn default() -> Self {
        Self {
            doc: empty(),
            pending: VecDeque::new(),
            generation: 0,
        }
    }
}
impl Document {
    pub fn text(&self) -> String {
        content(&self.doc)
    }
    pub fn pending(&self) -> &VecDeque<Operation> {
        &self.pending
    }
    pub fn state_vector(&self) -> Vec<u8> {
        use yrs::updates::encoder::Encode;
        self.doc.transact().state_vector().encode_v1()
    }
    pub fn diff(&self, vector: &[u8]) -> Result<Vec<u8>, &'static str> {
        if vector.len() > 64 * 1024 {
            return Err("state vector too large");
        }
        let vector = StateVector::decode_v1(vector).map_err(|_| "invalid state vector")?;
        Ok(self.doc.transact().encode_state_as_update_v1(&vector))
    }
    pub fn receive(&mut self, update: &[u8]) -> Result<(), &'static str> {
        let staged = empty();
        apply(&staged, &snapshot(&self.doc))?;
        apply(&staged, update)?;
        if content(&staged).len() > MAX_TEXT || snapshot(&staged).len() > MAX_TRANSFER {
            return Err("document limit");
        }
        self.doc = staged;
        self.generation += 1;
        Ok(())
    }
    pub fn edit(&mut self, from: u32, to: u32, insert: &str) -> Result<Uuid, &'static str> {
        if insert.len() > MAX_TEXT || self.pending.len() >= MAX_OPERATIONS {
            return Err("outbox or document limit");
        }
        let current = self.text();
        let boundary = |offset| {
            if offset == 0 {
                return true;
            }
            let mut at = 0;
            current.chars().any(|ch| {
                at += ch.len_utf16() as u32;
                at == offset
            })
        };
        if from > to || !boundary(from) || !boundary(to) {
            return Err("invalid UTF-16 range");
        }
        let staged = empty();
        apply(&staged, &snapshot(&self.doc))?;
        let text = staged.get_or_insert_text("content");
        let vector = staged.transact().state_vector();
        {
            let mut txn = staged.transact_mut();
            text.remove_range(&mut txn, from, to - from);
            text.insert(&mut txn, from, insert);
        }
        let update = staged.transact().encode_state_as_update_v1(&vector);
        if self.pending.iter().map(|op| op.update.len()).sum::<usize>() + update.len() > MAX_OUTBOX
            || content(&staged).len() > MAX_TEXT
            || snapshot(&staged).len() > MAX_TRANSFER
        {
            return Err("outbox or document limit");
        }
        let id = Uuid::new_v4();
        self.pending.push_back(Operation { id, update });
        self.doc = staged;
        self.generation += 1;
        Ok(id)
    }
    /// Only call after a peer's persistence acknowledgement; the current CRDT
    /// snapshot remains local even when an outbox record is compacted away.
    pub fn acknowledge(&mut self, id: Uuid) {
        self.pending.retain(|op| op.id != id);
        self.generation += 1;
    }
    /// A checkpoint is not a durability acknowledgement. The host must persist it first.
    pub fn checkpoint(&self, project: [u8; 32]) -> Vec<u8> {
        let state = snapshot(&self.doc);
        let mut out = Vec::new();
        out.extend_from_slice(b"OIJ1");
        out.extend_from_slice(&self.generation.to_be_bytes());
        out.extend_from_slice(&project);
        out.extend_from_slice(&(state.len() as u32).to_be_bytes());
        out.extend_from_slice(&state);
        out.extend_from_slice(&(self.pending.len() as u32).to_be_bytes());
        for op in &self.pending {
            out.extend_from_slice(op.id.as_bytes());
            out.extend_from_slice(&(op.update.len() as u32).to_be_bytes());
            out.extend_from_slice(&op.update);
        }
        let digest = Sha256::digest(&out);
        out.extend_from_slice(&digest);
        out
    }
    pub fn restore(bytes: &[u8], project: [u8; 32]) -> Result<Self, &'static str> {
        if bytes.len() < 84
            || bytes.len() > MAX_OUTBOX + MAX_TRANSFER + 16 * 1024
            || &bytes[..4] != b"OIJ1"
        {
            return Err("invalid checkpoint");
        }
        let (data, digest) = bytes.split_at(bytes.len() - 32);
        if Sha256::digest(data).as_slice() != digest || bytes[12..44] != project {
            return Err("checkpoint integrity or project mismatch");
        }
        let generation = u64::from_be_bytes(bytes[4..12].try_into().unwrap());
        let mut rest = &data[44..];
        fn length(input: &mut &[u8]) -> Result<usize, &'static str> {
            if input.len() < 4 {
                return Err("truncated checkpoint");
            }
            let value = u32::from_be_bytes(input[..4].try_into().unwrap()) as usize;
            *input = &input[4..];
            Ok(value)
        }
        fn take<'a>(input: &mut &'a [u8], count: usize) -> Result<&'a [u8], &'static str> {
            if input.len() < count {
                return Err("truncated checkpoint");
            }
            let value = &input[..count];
            *input = &input[count..];
            Ok(value)
        }
        let state_len = length(&mut rest)?;
        if state_len > MAX_TRANSFER {
            return Err("snapshot too large");
        }
        let doc = empty();
        apply(&doc, take(&mut rest, state_len)?)?;
        let count = length(&mut rest)?;
        if count > MAX_OPERATIONS {
            return Err("outbox limit");
        }
        let mut pending = VecDeque::new();
        let mut size = 0;
        for _ in 0..count {
            let id = Uuid::from_slice(take(&mut rest, 16)?).map_err(|_| "invalid operation ID")?;
            let len = length(&mut rest)?;
            size += len;
            if len > MAX_TRANSFER || size > MAX_OUTBOX {
                return Err("outbox limit");
            }
            pending.push_back(Operation {
                id,
                update: take(&mut rest, len)?.to_vec(),
            });
        }
        if !rest.is_empty() || content(&doc).len() > MAX_TEXT {
            return Err("invalid checkpoint contents");
        }
        Ok(Self {
            doc,
            pending,
            generation,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_unicode_edits_converge_and_duplicate_updates_are_safe() {
        let mut a = Document::default();
        a.edit(0, 0, "A😀B").unwrap();
        let initial = a.checkpoint([1; 32]);
        let mut b = Document::restore(&initial, [1; 32]).unwrap();
        a.edit(1, 1, "α").unwrap();
        b.edit(4, 4, "β").unwrap();
        let av = a.pending.back().unwrap().update.clone();
        let bv = b.pending.back().unwrap().update.clone();
        a.receive(&bv).unwrap();
        a.receive(&bv).unwrap();
        b.receive(&av).unwrap();
        assert_eq!(a.text(), "Aα😀Bβ");
        assert_eq!(a.text(), b.text());
        assert!(a.edit(3, 3, "x").is_err());
    }
    #[test]
    fn recovery_retains_pending_work_and_refuses_corruption_or_other_projects() {
        let mut doc = Document::default();
        let id = doc.edit(0, 0, "pending").unwrap();
        let mut checkpoint = doc.checkpoint([2; 32]);
        let mut restored = Document::restore(&checkpoint, [2; 32]).unwrap();
        assert_eq!(restored.text(), "pending");
        assert_eq!(restored.pending().len(), 1);
        restored.acknowledge(id);
        assert!(restored.pending().is_empty());
        assert_eq!(restored.text(), "pending");
        assert!(Document::restore(&checkpoint, [3; 32]).is_err());
        checkpoint[50] ^= 1;
        assert!(Document::restore(&checkpoint, [2; 32]).is_err());
    }
}
