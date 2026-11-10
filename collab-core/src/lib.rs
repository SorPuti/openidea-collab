//! CRDT document state; transport, authentication, and persistence are separate.
use yrs::updates::decoder::Decode;
use yrs::{Doc, OffsetKind, Options, ReadTxn, StateVector, Transact, Update};

/// Bound untrusted update allocations; transport must enforce its own frame limit too.
pub const MAX_UPDATE_BYTES: usize = 1024 * 1024;

/// Merge encoded updates without losing causally pending operations.
pub fn merge_updates(left: &[u8], right: &[u8]) -> Result<Vec<u8>, String> {
    if left.len() > MAX_UPDATE_BYTES || right.len() > MAX_UPDATE_BYTES {
        return Err("update exceeds size limit".into());
    }
    yrs::merge_updates_v1([left, right]).map_err(|_| "invalid update".into())
}

pub struct SharedDocument {
    doc: Doc,
}

impl Default for SharedDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedDocument {
    pub fn new() -> Self {
        Self {
            doc: Doc::with_options(Options {
                offset_kind: OffsetKind::Utf16,
                ..Default::default()
            }),
        }
    }

    pub fn apply(&self, data: &[u8]) -> Result<(), String> {
        if data.len() > MAX_UPDATE_BYTES {
            return Err("update exceeds size limit".into());
        }
        let update = Update::decode_v1(data).map_err(|_| "invalid update".to_string())?;
        self.doc
            .transact_mut()
            .apply_update(update)
            .map_err(|_| "could not apply update".to_string())
    }

    pub fn diff(&self, vector: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let vector = match vector {
            Some(data) if data.len() <= MAX_UPDATE_BYTES => {
                StateVector::decode_v1(data).map_err(|_| "invalid state vector".to_string())?
            }
            Some(_) => return Err("state vector exceeds size limit".into()),
            None => StateVector::default(),
        };
        Ok(self.doc.transact().encode_state_as_update_v1(&vector))
    }
}

impl SharedDocument {
    /// Visible plain text for editor integration.
    pub fn text(&self) -> String {
        use yrs::GetString;
        let text = self.doc.get_or_insert_text("content");
        text.get_string(&self.doc.transact())
    }

    /// Apply an edit at UTF-16 code-unit boundaries and return an incremental Yrs update.
    pub fn edit(&self, from: u32, to: u32, insert: &str) -> Result<Vec<u8>, String> {
        use yrs::{GetString, Text};
        if insert.len() > MAX_UPDATE_BYTES {
            return Err("insert exceeds size limit".into());
        }
        let text = self.doc.get_or_insert_text("content");
        let txn = self.doc.transact();
        let current = text.get_string(&txn);
        let vector = txn.state_vector();
        drop(txn);
        let valid_boundary = |wanted: u32| {
            let mut index = 0u32;
            if wanted == 0 {
                return true;
            }
            for ch in current.chars() {
                index += ch.len_utf16() as u32;
                if index == wanted {
                    return true;
                }
            }
            false
        };
        if from > to || !valid_boundary(from) || !valid_boundary(to) {
            return Err("invalid UTF-16 range".into());
        }
        let mut txn = self.doc.transact_mut();
        if from != to {
            text.remove_range(&mut txn, from, to - from);
        }
        if !insert.is_empty() {
            text.insert(&mut txn, from, insert);
        }
        Ok(txn.encode_state_as_update_v1(&vector))
    }
}
