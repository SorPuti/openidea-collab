use openidea_collab_protocol::{Frame, Kind, MAX_PAYLOAD, MAX_TRANSFER};
use std::collections::HashMap;
use uuid::Uuid;

const MAX_IN_FLIGHT: usize = 4;
const TIMEOUT_MS: u64 = 30_000;
struct Pending {
    document: Uuid,
    target: Uuid,
    kind: Kind,
    total: usize,
    created: u64,
    bytes: usize,
    parts: Vec<Option<Vec<u8>>>,
}
#[derive(Default)]
pub struct Reassembly {
    pending: HashMap<(Uuid, Uuid), Pending>,
}
impl Reassembly {
    /// Input is already authenticated by the relay; caller supplies a monotonic time.
    pub fn push(
        &mut self,
        sender: Uuid,
        frame: &Frame<'_>,
        time_ms: u64,
    ) -> Result<Option<Vec<u8>>, &'static str> {
        self.pending
            .retain(|_, p| time_ms.saturating_sub(p.created) < TIMEOUT_MS);
        // Validate all bounds even when callers constructed the frame directly.
        frame.validate()?;
        if frame.parts == 1 {
            return Ok(Some(frame.payload.to_vec()));
        }
        let key = (sender, frame.message);
        if !self.pending.contains_key(&key) {
            if self.pending.len() >= MAX_IN_FLIGHT {
                return Err("too many transfers");
            }
            self.pending.insert(
                key,
                Pending {
                    document: frame.document,
                    target: frame.target,
                    kind: frame.kind,
                    total: frame.total as usize,
                    created: time_ms,
                    bytes: 0,
                    parts: vec![None; frame.parts as usize],
                },
            );
        }
        let pending = self.pending.get_mut(&key).unwrap();
        if pending.document != frame.document
            || pending.target != frame.target
            || pending.kind != frame.kind
            || pending.total != frame.total as usize
            || pending.parts.len() != frame.parts as usize
        {
            return Err("inconsistent transfer");
        }
        if let Some(previous) = &pending.parts[frame.part as usize] {
            if previous != frame.payload {
                return Err("conflicting duplicate chunk");
            }
            return Ok(None);
        }
        if pending.bytes + frame.payload.len() > pending.total {
            return Err("transfer exceeds declared size");
        }
        pending.bytes += frame.payload.len();
        pending.parts[frame.part as usize] = Some(frame.payload.to_vec());
        if pending.parts.iter().any(Option::is_none) {
            return Ok(None);
        }
        let pending = self.pending.remove(&key).unwrap();
        if pending.bytes != pending.total {
            return Err("incomplete transfer size");
        }
        let mut data = Vec::with_capacity(pending.total);
        for part in pending.parts {
            data.extend_from_slice(&part.unwrap());
        }
        Ok(Some(data))
    }
}

pub fn split(
    kind: Kind,
    document: Uuid,
    target: Uuid,
    message: Uuid,
    payload: &[u8],
) -> Result<Vec<Vec<u8>>, &'static str> {
    if payload.is_empty() || payload.len() > MAX_TRANSFER {
        return Err("transfer size limit");
    }
    let count = payload.len().div_ceil(MAX_PAYLOAD);
    payload
        .chunks(MAX_PAYLOAD)
        .enumerate()
        .map(|(part, data)| {
            Frame {
                kind,
                document,
                target,
                message,
                part: part as u16,
                parts: count as u16,
                total: payload.len() as u32,
                payload: data,
            }
            .encode()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunked_transfer_recovers_reverse_order_and_duplicates() {
        let bytes = vec![42; MAX_PAYLOAD * 2 + 10];
        let parts = split(
            Kind::Snapshot,
            Uuid::new_v4(),
            Uuid::nil(),
            Uuid::new_v4(),
            &bytes,
        )
        .unwrap();
        let sender = Uuid::new_v4();
        let mut receive = Reassembly::default();
        let mut result = None;
        for part in parts.iter().rev() {
            result = receive
                .push(sender, &Frame::decode(part).unwrap(), 0)
                .unwrap();
            if result.is_none() {
                assert!(
                    receive
                        .push(sender, &Frame::decode(part).unwrap(), 0)
                        .unwrap()
                        .is_none()
                );
            }
        }
        assert_eq!(result.unwrap(), bytes);
    }
}
