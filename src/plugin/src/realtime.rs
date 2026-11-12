//! Project-bound realtime document synchronization over the host WebSocket API.

use crate::{
    client,
    state::{Session, SharedDocument},
};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use openidea_collab_core::{
    document::Document as CrdtDocument,
    policy::{project_identity, safe_relative_path},
    transfer::{Reassembly, split},
};
use openidea_collab_protocol::{DEFAULT_ORIGIN, Endpoint, Frame, Kind, Role, SUBPROTOCOL};
use openidea_plugin::{
    DocumentChanged, Error, Event, Position, Project, Range, TextEdit,
    host::{self, WebSocket, WebSocketRequest},
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const DRAIN_LIMIT: u32 = 32;
const MAX_SHARED_DOCUMENTS: usize = 32;

struct LiveDocument {
    shared: SharedDocument,
    absolute: String,
    crdt: CrdtDocument,
    initialized: bool,
    suppressed_versions: HashSet<u64>,
    editor_version: u64,
}

pub struct Realtime {
    socket: WebSocket,
    root: String,
    project: [u8; 32],
    session_id: Uuid,
    member_id: Uuid,
    role: Role,
    documents: HashMap<Uuid, LiveDocument>,
    by_path: HashMap<String, Uuid>,
    reassembly: Reassembly,
    status: String,
}

impl Realtime {
    pub fn connect(project: &Project, session: &Session) -> Result<Self, Error> {
        let root = project.root()?;
        if project_identity(&root) != session.project {
            return Err(Error::Invalid(
                "The collaboration session belongs to another project".into(),
            ));
        }
        let socket = WebSocket::connect(
            project,
            &WebSocketRequest {
                url: Endpoint::parse(DEFAULT_ORIGIN)
                    .map_err(|message| Error::Invalid(message.into()))?
                    .websocket(),
                headers: vec![(
                    "authorization".into(),
                    "Bearer {{secret:member-credential}}".into(),
                )],
                subprotocol: SUBPROTOCOL.into(),
            },
        )?;
        let mut runtime = Self {
            socket,
            root: root.trim_end_matches('/').to_owned(),
            project: session.project,
            session_id: session.session_id,
            member_id: session.member_id,
            role: session.role,
            documents: HashMap::new(),
            by_path: HashMap::new(),
            reassembly: Reassembly::default(),
            status: "Connected".into(),
        };
        for shared in &session.documents {
            runtime.load_document(project, shared.clone())?;
        }
        runtime.refresh_event_filter()?;
        let ids: Vec<_> = runtime.documents.keys().copied().collect();
        for id in ids {
            runtime.announce(project, id)?;
            runtime.resend_pending(project, id)?;
        }
        Ok(runtime)
    }

    pub fn connection(&self) -> u64 {
        self.socket.id()
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn shared_paths(&self) -> Vec<String> {
        let mut paths: Vec<_> = self
            .documents
            .values()
            .map(|document| document.shared.path.clone())
            .collect();
        paths.sort();
        paths
    }

    pub fn add_document(&mut self, project: &Project, shared: SharedDocument) -> Result<(), Error> {
        if self.documents.len() >= MAX_SHARED_DOCUMENTS
            && !self.documents.contains_key(&shared.document_id)
        {
            return Err(Error::Invalid(
                "At most 32 documents can be shared in one session".into(),
            ));
        }
        let id = shared.document_id;
        self.load_document(project, shared)?;
        self.refresh_event_filter()?;
        self.announce(project, id)
    }

    pub fn remove_document(&mut self, shared: &SharedDocument) -> Result<(), Error> {
        if let Some(document) = self.documents.remove(&shared.document_id) {
            self.by_path.remove(&document.absolute);
            self.socket.unsubscribe_document(&document.shared.path)?;
        }
        host::storage::delete(&checkpoint_key(self.session_id, shared.document_id))?;
        self.refresh_event_filter()
    }

    pub fn handle_events(&mut self, project: &Project, events: &[Event]) -> Result<bool, Error> {
        for event in events {
            match event {
                Event::DocumentChanged(change) => self.local_change(project, change)?,
                Event::WebSocketReady(ready)
                    if ready.connection == self.socket.id() && self.drain(project)? =>
                {
                    self.status = "Disconnected by relay".into();
                    host::subscribe_paths("document-changed", &[])?;
                    return Ok(false);
                }
                _ => {}
            }
        }
        Ok(true)
    }

    fn load_document(&mut self, project: &Project, shared: SharedDocument) -> Result<(), Error> {
        validate_relative(&shared.path)?;
        let subscription_path = shared.path.clone();
        let absolute = absolute_path(&self.root, &shared.path);
        let current = project.document(&absolute)?;
        if self.role == Role::Owner {
            client::register_document(&self.session_id.to_string(), shared.document_id)?;
        }
        let restored = restore_checkpoint(self.session_id, shared.document_id, self.project)?;
        let (mut crdt, initialized) = match restored {
            Some(document) => (document, true),
            None if self.role == Role::Owner => (
                CrdtDocument::from_text(&current.text).map_err(collab_error)?,
                true,
            ),
            None => (CrdtDocument::default(), false),
        };
        let editor_version = current.version;
        if initialized && crdt.text() != current.text {
            replace_crdt_text(&mut crdt, &current.text).map_err(collab_error)?;
        }
        if initialized {
            persist_checkpoint(self.session_id, shared.document_id, self.project, &crdt)?;
        }
        self.by_path.insert(absolute.clone(), shared.document_id);
        self.documents.insert(
            shared.document_id,
            LiveDocument {
                shared,
                absolute,
                crdt,
                initialized,
                suppressed_versions: HashSet::new(),
                editor_version,
            },
        );
        self.socket.subscribe_document(&subscription_path)?;
        Ok(())
    }

    fn refresh_event_filter(&self) -> Result<(), Error> {
        // The host derives the editor event bridge from WebSocket document
        // resources, avoiding a second independently scoped path filter.
        Ok(())
    }

    fn announce(&self, project: &Project, document_id: Uuid) -> Result<(), Error> {
        let document = self
            .documents
            .get(&document_id)
            .ok_or_else(|| Error::NotFound("shared document".into()))?;
        if document.initialized {
            self.send_payload(
                project,
                Kind::Snapshot,
                document_id,
                Uuid::nil(),
                Uuid::new_v4(),
                &document.crdt.snapshot(),
            )
        } else {
            self.send_payload(
                project,
                Kind::StateVector,
                document_id,
                Uuid::nil(),
                Uuid::new_v4(),
                &document.crdt.state_vector(),
            )
        }
    }

    fn send_payload(
        &self,
        _project: &Project,
        kind: Kind,
        document: Uuid,
        target: Uuid,
        message: Uuid,
        payload: &[u8],
    ) -> Result<(), Error> {
        for frame in split(kind, document, target, message, payload).map_err(collab_error)? {
            self.socket.send(&frame)?;
        }
        Ok(())
    }

    fn resend_pending(&self, project: &Project, document_id: Uuid) -> Result<(), Error> {
        let document = self
            .documents
            .get(&document_id)
            .ok_or_else(|| Error::NotFound("shared document".into()))?;
        for operation in document.crdt.pending() {
            self.send_payload(
                project,
                Kind::Update,
                document_id,
                Uuid::nil(),
                operation.id,
                &operation.update,
            )?;
        }
        Ok(())
    }

    fn local_change(&mut self, project: &Project, change: &DocumentChanged) -> Result<(), Error> {
        let Some(document_id) = self.by_path.get(&change.path).copied() else {
            return Ok(());
        };
        let document = self
            .documents
            .get_mut(&document_id)
            .expect("path index points to document");
        if document.suppressed_versions.remove(&change.version) {
            document.editor_version = document.editor_version.max(change.version);
            return Ok(());
        }
        if change.version <= document.editor_version {
            return Ok(());
        }
        if !self.role.can_write() {
            self.status = format!(
                "Local edits to {} are not shared by a viewer",
                document.shared.path
            );
            return Ok(());
        }
        if !document.initialized {
            self.status = format!(
                "Waiting for the initial state of {}; local content was preserved",
                document.shared.path
            );
            return Ok(());
        }
        if change.base_version != document.editor_version || change.changes.is_empty() {
            let current = project.document(&document.absolute)?;
            if document.crdt.text() == current.text {
                document.editor_version = current.version;
                return Ok(());
            }
            let operation =
                replace_crdt_text(&mut document.crdt, &current.text).map_err(collab_error)?;
            document.editor_version = current.version;
            persist_checkpoint(self.session_id, document_id, self.project, &document.crdt)?;
            self.send_operation(project, document_id, operation)?;
            return Ok(());
        }
        let edits: Vec<_> = change
            .changes
            .iter()
            .map(|edit| (edit.from, edit.to, edit.insert.as_str()))
            .collect();
        let first = document.crdt.edit_batch(&edits).map_err(collab_error)?;
        document.editor_version = change.version;
        let current = project.document(&document.absolute)?;
        let mut operations = vec![first];
        if document.crdt.text() != current.text {
            operations
                .push(replace_crdt_text(&mut document.crdt, &current.text).map_err(collab_error)?);
            document.editor_version = current.version;
        }
        persist_checkpoint(self.session_id, document_id, self.project, &document.crdt)?;
        for operation in operations {
            self.send_operation(project, document_id, operation)?;
        }
        Ok(())
    }

    fn send_operation(
        &self,
        project: &Project,
        document_id: Uuid,
        operation_id: Uuid,
    ) -> Result<(), Error> {
        let operation = self
            .documents
            .get(&document_id)
            .and_then(|document| document.crdt.operation(operation_id))
            .ok_or_else(|| Error::NotFound("pending collaboration update".into()))?;
        self.send_payload(
            project,
            Kind::Update,
            document_id,
            Uuid::nil(),
            operation.id,
            &operation.update,
        )
    }

    fn drain(&mut self, project: &Project) -> Result<bool, Error> {
        loop {
            let batch = self.socket.drain(DRAIN_LIMIT)?;
            let count = batch.messages.len();
            for message in batch.messages {
                self.receive(project, &message)?;
            }
            if batch.closed {
                return Ok(true);
            }
            if count < DRAIN_LIMIT as usize {
                return Ok(false);
            }
        }
    }

    fn receive(&mut self, project: &Project, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() < 16 {
            return Err(Error::Invalid(
                "Relay frame is missing its authenticated sender".into(),
            ));
        }
        let sender = Uuid::from_slice(&bytes[..16])
            .map_err(|_| Error::Invalid("Invalid relay sender".into()))?;
        if sender == self.member_id {
            return Err(Error::Invalid("Relay echoed a member's own update".into()));
        }
        let frame = Frame::decode(&bytes[16..]).map_err(collab_error)?;
        if !frame.target.is_nil() && frame.target != self.member_id {
            return Err(Error::Invalid("Relay frame targets another member".into()));
        }
        let kind = frame.kind;
        let document_id = frame.document;
        let message_id = frame.message;
        let payload = self
            .reassembly
            .push(sender, &frame, monotonic_ms())
            .map_err(collab_error)?;
        let Some(payload) = payload else {
            return Ok(());
        };
        if !self.documents.contains_key(&document_id) {
            return Ok(());
        }
        match kind {
            Kind::StateVector => {
                let update = self.documents[&document_id]
                    .crdt
                    .diff(&payload)
                    .map_err(collab_error)?;
                if !update.is_empty() {
                    self.send_payload(
                        project,
                        Kind::Snapshot,
                        document_id,
                        sender,
                        Uuid::new_v4(),
                        &update,
                    )?;
                }
            }
            Kind::Update | Kind::Snapshot => {
                self.apply_remote(project, document_id, &payload)?;
                self.send_payload(
                    project,
                    Kind::Ack,
                    document_id,
                    sender,
                    Uuid::new_v4(),
                    message_id.as_bytes(),
                )?;
            }
            Kind::Ack => {
                let operation = Uuid::from_slice(&payload)
                    .map_err(|_| Error::Invalid("Invalid ACK operation".into()))?;
                let document = self
                    .documents
                    .get_mut(&document_id)
                    .expect("document checked above");
                document.crdt.acknowledge(operation);
                persist_checkpoint(self.session_id, document_id, self.project, &document.crdt)?;
            }
            Kind::Presence => {}
        }
        Ok(())
    }

    fn apply_remote(
        &mut self,
        project: &Project,
        document_id: Uuid,
        update: &[u8],
    ) -> Result<(), Error> {
        let mut operation_to_send = None;
        let document = self
            .documents
            .get_mut(&document_id)
            .expect("document checked before apply");
        let current = project.document(&document.absolute)?;
        if document.initialized && document.crdt.text() != current.text {
            if self.role.can_write() {
                operation_to_send = Some(
                    replace_crdt_text(&mut document.crdt, &current.text).map_err(collab_error)?,
                );
                document.editor_version = current.version;
            } else {
                return Err(Error::Stale);
            }
        }
        let before = document.crdt.checkpoint(self.project);
        document.crdt.receive(update).map_err(collab_error)?;
        let merged = document.crdt.text();
        if !document.initialized && current.text != merged {
            document.crdt = CrdtDocument::restore(&before, self.project).map_err(collab_error)?;
            self.status = format!(
                "Conflict in {}: local content was preserved because it differs from the shared base",
                document.shared.path
            );
            return Err(Error::Stale);
        }
        document.initialized = true;
        if current.text != merged {
            let version = match project.apply_document_edits(
                &document.absolute,
                current.version,
                &[TextEdit {
                    range: whole_document(&current.text),
                    text: merged,
                }],
            ) {
                Ok(version) => version,
                Err(error) => {
                    document.crdt =
                        CrdtDocument::restore(&before, self.project).map_err(collab_error)?;
                    persist_checkpoint(self.session_id, document_id, self.project, &document.crdt)?;
                    return Err(error);
                }
            };
            document.editor_version = version;
            document.suppressed_versions.insert(version);
        }
        persist_checkpoint(self.session_id, document_id, self.project, &document.crdt)?;
        if let Some(operation) = operation_to_send {
            self.send_operation(project, document_id, operation)?;
        }
        Ok(())
    }
}

pub fn shared_document(
    project: &Project,
    path: &str,
    session: Uuid,
) -> Result<SharedDocument, Error> {
    let root = project.root()?;
    let relative = relative_path(&root, path)?;
    validate_relative(&relative)?;
    project.document(path)?;
    Ok(SharedDocument {
        document_id: Uuid::new_v5(&session, relative.as_bytes()),
        path: relative,
    })
}

fn relative_path(root: &str, path: &str) -> Result<String, Error> {
    let root = root.trim_end_matches('/');
    let relative = path
        .strip_prefix(root)
        .and_then(|value| value.strip_prefix('/'))
        .ok_or_else(|| Error::Invalid("The document is outside this project".into()))?;
    Ok(relative.to_owned())
}

fn validate_relative(path: &str) -> Result<(), Error> {
    if !safe_relative_path(path)
        || path
            .bytes()
            .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b']' | b'{' | b'}'))
    {
        return Err(Error::Invalid(
            "This document path cannot be shared safely".into(),
        ));
    }
    Ok(())
}

fn absolute_path(root: &str, relative: &str) -> String {
    format!("{}/{relative}", root.trim_end_matches('/'))
}

fn checkpoint_key(session: Uuid, document: Uuid) -> String {
    format!("checkpoint/{session}/{document}")
}

fn restore_checkpoint(
    session: Uuid,
    document: Uuid,
    project: [u8; 32],
) -> Result<Option<CrdtDocument>, Error> {
    let Some(encoded) = host::storage::get(&checkpoint_key(session, document))? else {
        return Ok(None);
    };
    let bytes = STANDARD_NO_PAD
        .decode(encoded)
        .map_err(|_| Error::Invalid("Stored collaboration checkpoint is corrupt".into()))?;
    CrdtDocument::restore(&bytes, project)
        .map(Some)
        .map_err(collab_error)
}

fn persist_checkpoint(
    session: Uuid,
    document: Uuid,
    project: [u8; 32],
    crdt: &CrdtDocument,
) -> Result<(), Error> {
    let encoded = STANDARD_NO_PAD.encode(crdt.checkpoint(project));
    host::storage::set(&checkpoint_key(session, document), &encoded)
}

fn replace_crdt_text(document: &mut CrdtDocument, target: &str) -> Result<Uuid, &'static str> {
    let current = document.text();
    let (from, to, insert) = single_edit(&current, target);
    document.edit(from, to, insert)
}

fn single_edit<'a>(current: &str, target: &'a str) -> (u32, u32, &'a str) {
    let mut prefix_bytes = 0;
    let mut prefix_utf16 = 0;
    for (left, right) in current.chars().zip(target.chars()) {
        if left != right {
            break;
        }
        prefix_bytes += right.len_utf8();
        prefix_utf16 += right.len_utf16() as u32;
    }
    let current_tail = &current[prefix_bytes..];
    let target_tail = &target[prefix_bytes..];
    let mut suffix_target_bytes = 0;
    let mut suffix_utf16 = 0;
    for (left, right) in current_tail.chars().rev().zip(target_tail.chars().rev()) {
        if left != right {
            break;
        }
        suffix_target_bytes += right.len_utf8();
        suffix_utf16 += left.len_utf16() as u32;
    }
    let current_utf16 = current.encode_utf16().count() as u32;
    let target_end = target.len() - suffix_target_bytes;
    (
        prefix_utf16,
        current_utf16 - suffix_utf16,
        &target[prefix_bytes..target_end],
    )
}

fn whole_document(text: &str) -> Range {
    let mut line = 0;
    let mut character = 0;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }
    Range::new(Position::new(0, 0), Position::new(line, character))
}

fn collab_error(message: &'static str) -> Error {
    Error::Invalid(message.into())
}

fn monotonic_ms() -> u64 {
    // Reassembly only compares relative ages; host event delivery is serialized.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_scope_and_document_ids_are_stable() {
        let session = Uuid::new_v4();
        let id = Uuid::new_v5(&session, b"src/main.rs");
        assert_eq!(id, Uuid::new_v5(&session, b"src/main.rs"));
        assert!(validate_relative("src/main.rs").is_ok());
        assert!(validate_relative("src/*.rs").is_err());
        assert!(validate_relative(".env").is_err());
    }

    #[test]
    fn text_reconciliation_uses_utf16_boundaries_and_preserves_common_text() {
        let mut document = CrdtDocument::from_text("A😀BC").unwrap();
        replace_crdt_text(&mut document, "A😀βC").unwrap();
        assert_eq!(document.text(), "A😀βC");
        assert_eq!(whole_document("A😀\nβ").end, Position::new(1, 1));
    }
}
