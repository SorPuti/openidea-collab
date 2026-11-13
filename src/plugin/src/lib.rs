//! Explicitly scoped realtime collaboration over the project-bound host API.
pub mod client;
pub mod panel;
pub mod realtime;
pub mod state;
use openidea_collab_protocol::{DEFAULT_ORIGIN, Role};
use openidea_plugin::{
    Context, Error, Event, EventKind, Invocation, NotificationLevel, Plugin, Project, host,
    register_plugin,
    views::{Body, Button, View, ViewEvent, Viewport, Views},
};
const PANEL: &str = "collab-panel";
fn ready_status() -> &'static str {
    if host::locale().starts_with("pt") {
        "Sessão preparada. Conecte o realtime e compartilhe explicitamente o documento aberto."
    } else {
        "Session prepared. Connect realtime, then explicitly share the open document."
    }
}
#[derive(Default)]
pub struct CollabPlugin {
    invitation: Option<String>,
    realtime: Option<realtime::Realtime>,
    page: panel::Page,
    panel_error: Option<String>,
    activity: Vec<String>,
}
impl Plugin for CollabPlugin {
    fn activate(&mut self, _context: &Context) -> Result<(), Error> {
        // Remove the unsafe legacy auto-restored room. No network or file access.
        host::storage::delete("room_state")?;
        host::subscribe(EventKind::WebsocketReady)?;
        Ok(())
    }
    fn deactivate(&mut self) {
        self.realtime = None;
        host::unsubscribe(EventKind::DocumentChanged);
    }
    fn run_command(
        &mut self,
        id: &str,
        ctx: &Invocation,
        _argument: Option<&str>,
    ) -> Result<Option<String>, Error> {
        match id {
            "create-session" | "join-session" => {
                if state::for_project(ctx)?.is_some() {
                    return Err(Error::Invalid("Leave the existing session first".into()));
                }
                if !host::confirm(
                    "Prepare collaboration session",
                    &format!(
                        "Relay: {DEFAULT_ORIGIN}\nOnly documents you explicitly share will be transmitted. The relay can inspect transient encrypted-transport payloads."
                    ),
                )? {
                    return Ok(None);
                }
                let relay = client::relay_origin()?;
                let credential = if id == "create-session" {
                    if !host::secret_is_set(client::ADMISSION_SECRET) {
                        if let Some(token) = host::input(
                            "Relay admission token (leave blank if the relay permits open creation)",
                            None,
                        )? {
                            let token = token.trim();
                            if !token.is_empty() {
                                host::secret_set(client::ADMISSION_SECRET, token)?;
                            }
                        }
                    }
                    client::create(&relay)?
                } else {
                    let Some(invitation) = host::input("Paste the invitation URL", None)? else {
                        return Ok(None);
                    };
                    client::join(&relay, invitation.trim())?
                };
                state::save(ctx, relay, credential)?;
                self.connect(ctx)?;
            }
            "create-invite" => {
                let session = state::for_project(ctx)?
                    .ok_or_else(|| Error::Invalid("Prepare a session first".into()))?;
                if session.role != Role::Owner {
                    return Err(Error::Denied("Only the session owner can invite".into()));
                }
                self.invitation =
                    Some(client::invite(&session.relay, &session.session_id.to_string())?.url);
            }
            "leave-session" => {
                if let Some(session) = state::load()? {
                    if !host::confirm(
                        "Leave collaboration",
                        "End the prepared session? Leaving as owner closes it for all members. No project files will be changed.",
                    )? {
                        return Ok(None);
                    }
                    // If the server cannot revoke, retain the credential and report the error.
                    client::leave(
                        &session.relay,
                        &session.session_id.to_string(),
                        &session.member_id.to_string(),
                    )?;
                    self.disconnect()?;
                    state::forget()?;
                }
                self.invitation = None;
            }
            "connect-realtime" => self.connect(ctx)?,
            "disconnect-realtime" => self.disconnect()?,
            "share-current-document" => self.share_current(ctx)?,
            "share-open-documents" => self.share_open_documents(ctx)?,
            "stop-sharing-current-document" => self.stop_sharing_current(ctx)?,
            _ => return Err(Error::NotFound("Unknown collaboration action".into())),
        }
        host::refresh_view(PANEL);
        Ok(None)
    }

    fn handle_events(&mut self, events: &[Event], ctx: &Invocation) {
        let Some(_) = self.realtime else { return };
        let project = self.project(ctx);
        let result = project.and_then(|project| {
            let connected = self
                .realtime
                .as_mut()
                .expect("checked above")
                .handle_events(&project, events)?;
            if !connected {
                self.realtime = None;
            }
            Ok(())
        });
        if let Err(error) = result {
            host::notify(
                NotificationLevel::Error,
                "Collaboration update refused",
                &error.to_string(),
            );
        }
        host::refresh_view(PANEL);
    }
}

impl CollabPlugin {
    fn record(&mut self, message: impl Into<String>) {
        self.activity.insert(0, message.into());
        self.activity.truncate(8);
    }

    fn panel_command(&mut self, id: &str, ctx: &Invocation) {
        match self.run_command(id, ctx, None) {
            Ok(_) => {
                self.panel_error = None;
                self.record(match id {
                    "create-session" => "Sessão preparada para este projeto",
                    "join-session" => "Convite aceito para este projeto",
                    "connect-realtime" => "Realtime conectado",
                    "disconnect-realtime" => "Realtime desconectado",
                    "share-current-document" => "Documento aberto compartilhado",
                    "share-open-documents" => "Documentos abertos compartilhados",
                    "stop-sharing-current-document" => "Documento removido da sessão",
                    "leave-session" => "Sessão removida deste projeto",
                    "create-invite" => "Convite de colaborador criado",
                    _ => "Ação de colaboração concluída",
                });
            }
            Err(error) => {
                self.panel_error = Some(error.to_string());
                self.record("Uma ação foi recusada; reveja os detalhes no painel");
            }
        }
        host::refresh_view(PANEL);
    }

    fn project(&self, ctx: &Invocation) -> Result<Project, Error> {
        let session =
            state::load()?.ok_or_else(|| Error::Invalid("Prepare a session first".into()))?;
        if let Some(project) = ctx.project()
            && project_identity_matches(&project, session.project)?
        {
            return Ok(project);
        }
        host::projects()?
            .into_iter()
            .find_map(|project| {
                project_identity_matches(&project, session.project)
                    .ok()
                    .filter(|same| *same)
                    .map(|_| project)
            })
            .ok_or_else(|| {
                Error::NotFound(
                    "The project bound to this collaboration session is not open".into(),
                )
            })
    }

    fn connect(&mut self, ctx: &Invocation) -> Result<(), Error> {
        if self.realtime.is_some() {
            return Ok(());
        }
        let session = state::for_project(ctx)?
            .ok_or_else(|| Error::Invalid("Prepare a session first".into()))?;
        let project = ctx
            .project()
            .ok_or_else(|| Error::Invalid("Open the session project first".into()))?;
        self.realtime = Some(realtime::Realtime::connect(&project, &session)?);
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), Error> {
        self.realtime = None;
        host::unsubscribe(EventKind::DocumentChanged);
        Ok(())
    }

    fn share_current(&mut self, ctx: &Invocation) -> Result<(), Error> {
        let editor = ctx
            .editor()
            .ok_or_else(|| Error::Invalid("Focus the document you want to share".into()))?;
        self.share_paths(ctx, vec![editor.path()], "Share document")
    }

    /// Shares only eligible documents already opened by the user. This never
    /// scans or opens project files, keeping consent and host path checks intact.
    fn share_open_documents(&mut self, ctx: &Invocation) -> Result<(), Error> {
        let paths = host::open_editors()?
            .into_iter()
            .map(|editor| editor.path)
            .collect();
        self.share_paths(ctx, paths, "Share open documents")
    }

    fn share_paths(
        &mut self,
        ctx: &Invocation,
        paths: Vec<String>,
        title: &str,
    ) -> Result<(), Error> {
        self.connect(ctx)?;
        let project = ctx
            .project()
            .ok_or_else(|| Error::Invalid("Open the session project first".into()))?;
        let mut session = state::for_project(ctx)?
            .ok_or_else(|| Error::Invalid("Prepare a session first".into()))?;
        let mut pending = Vec::new();
        for path in paths {
            let Ok(shared) = realtime::shared_document(&project, &path, session.session_id) else {
                continue;
            };
            if !session
                .documents
                .iter()
                .any(|document| document.document_id == shared.document_id)
                && !pending.iter().any(|document: &state::SharedDocument| {
                    document.document_id == shared.document_id
                })
            {
                pending.push(shared);
            }
        }
        if pending.is_empty() {
            return Err(Error::Invalid(
                "No additional eligible open documents to share".into(),
            ));
        }
        let limit = realtime::max_shared_documents();
        if session.documents.len() + pending.len() > limit {
            return Err(Error::Invalid(format!(
                "Sharing these documents exceeds the session limit of {limit}; stop sharing some documents first"
            )));
        }
        if !host::confirm(
            title,
            &format!(
                "Share these {} document(s) with up to two authenticated collaborators? Their unsaved editor text and later edits will be transmitted through {DEFAULT_ORIGIN}.\n\n{}",
                pending.len(),
                pending
                    .iter()
                    .map(|document| document.path.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )? {
            return Ok(());
        }
        let mut added = Vec::new();
        for shared in pending {
            if let Err(error) = self
                .realtime
                .as_mut()
                .expect("connected above")
                .add_document(&project, shared.clone())
            {
                for shared in added.iter().rev() {
                    let _ = self
                        .realtime
                        .as_mut()
                        .expect("connected above")
                        .remove_document(shared);
                }
                return Err(error);
            }
            added.push(shared);
        }
        session.documents.extend(added.iter().cloned());
        if let Err(error) = state::store(&session) {
            for shared in added.iter().rev() {
                let _ = self
                    .realtime
                    .as_mut()
                    .expect("connected above")
                    .remove_document(shared);
            }
            return Err(error);
        }
        Ok(())
    }

    fn stop_sharing_current(&mut self, ctx: &Invocation) -> Result<(), Error> {
        let project = ctx
            .project()
            .ok_or_else(|| Error::Invalid("Open the session project first".into()))?;
        let editor = ctx
            .editor()
            .ok_or_else(|| Error::Invalid("Focus the shared document".into()))?;
        let mut session = state::for_project(ctx)?
            .ok_or_else(|| Error::Invalid("Prepare a session first".into()))?;
        let shared = realtime::shared_document(&project, &editor.path(), session.session_id)?;
        let Some(index) = session
            .documents
            .iter()
            .position(|document| document.document_id == shared.document_id)
        else {
            return Err(Error::Invalid("This document is not shared".into()));
        };
        if !host::confirm(
            "Stop sharing",
            &format!(
                "Stop sharing {}? Local content will be preserved.",
                shared.path
            ),
        )? {
            return Ok(());
        }
        let shared = session.documents.remove(index);
        state::store(&session)?;
        if let Some(runtime) = &mut self.realtime
            && let Err(error) = runtime.remove_document(&shared)
        {
            self.realtime = None;
            return Err(error);
        }
        Ok(())
    }
}

fn project_identity_matches(project: &Project, expected: [u8; 32]) -> Result<bool, Error> {
    Ok(openidea_collab_core::policy::project_identity(&project.root()?) == expected)
}

impl Views for CollabPlugin {
    fn render(
        &mut self,
        view: &str,
        _viewport: Option<Viewport>,
        ctx: &Invocation,
    ) -> Result<View, Error> {
        if view != PANEL {
            return Err(Error::NotFound("Unknown collaboration view".into()));
        }
        let session = state::for_project(ctx);
        if !matches!(&session, Ok(Some(_))) {
            self.invitation = None;
        }
        let status = self.realtime.as_ref().map_or_else(
            || ready_status().to_owned(),
            |runtime| runtime.status().to_owned(),
        );
        let relay = client::relay_origin().unwrap_or_else(|_| DEFAULT_ORIGIN.into());
        let mut page = View::new(Body::Html(panel::html(
            self.page,
            session.as_ref().ok().and_then(Option::as_ref),
            &relay,
            self.realtime.as_ref().map(|_| status.as_str()),
            self.panel_error.as_deref(),
            self.invitation.as_deref(),
            &self.activity,
        )));
        for candidate in [
            panel::Page::Start,
            panel::Page::Session,
            panel::Page::Documents,
            panel::Page::Activity,
            panel::Page::Settings,
        ] {
            page = page.button(
                Button::new(&format!("view:{}", candidate.id()), candidate.label())
                    .enabled(candidate != self.page),
            );
        }
        match session {
            Ok(Some(session)) => {
                if session.role == Role::Owner {
                    page = page.button(Button::new("create-invite", "Criar convite").icon("add"));
                }
                if self.realtime.is_some() {
                    page = page
                        .button(
                            Button::new("share-current-document", "Compartilhar documento")
                                .icon("add"),
                        )
                        .button(
                            Button::new("share-open-documents", "Compartilhar abertos").icon("add"),
                        )
                        .button(
                            Button::new("stop-sharing-current-document", "Parar compartilhamento")
                                .icon("remove"),
                        )
                        .button(
                            Button::new("disconnect-realtime", "Desconectar realtime")
                                .icon("debug-disconnect"),
                        );
                } else {
                    page = page.button(
                        Button::new("connect-realtime", "Conectar realtime").icon("debug-connect"),
                    );
                }
                page = page.button(Button::new("leave-session", "Sair da sessão").icon("close"));
            }
            Ok(None) => {
                page = page
                    .button(Button::new("create-session", "Preparar sessão").icon("add"))
                    .button(Button::new("join-session", "Aceitar convite").icon("arrow-right"));
            }
            Err(_) => {
                page = page
                    .status("Abra um projeto ou encerre a sessão vinculada ao projeto anterior.")
                    .button(Button::new("leave-session", "Sair da sessão anterior").icon("close"));
            }
        }
        Ok(page)
    }
    fn handle(&mut self, view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        if view != PANEL {
            return Ok(());
        }
        match event {
            ViewEvent::Button(id) => {
                if let Some(page) = id.strip_prefix("view:").and_then(panel::Page::parse) {
                    self.page = page;
                    self.panel_error = None;
                    host::refresh_view(PANEL);
                } else {
                    self.panel_command(&id, ctx);
                }
            }
            ViewEvent::Message(message) => {
                let parsed = serde_json::from_str::<serde_json::Value>(&message).ok();
                let page = parsed
                    .as_ref()
                    .filter(|value| {
                        value.get("op").and_then(serde_json::Value::as_str) == Some("page")
                    })
                    .and_then(|value| value.get("page").and_then(serde_json::Value::as_str))
                    .and_then(panel::Page::parse);
                if let Some(page) = page {
                    self.page = page;
                    self.panel_error = None;
                    host::refresh_view(PANEL);
                }
            }
            _ => {}
        }
        Ok(())
    }
}
// Declaring a contributed view in plugin.toml is not enough: the component
// must explicitly export the optional `view-provider` WIT interface.  Keep
// this capability declaration beside the implementation so a future manifest
// change cannot produce a panel that the host cannot render.
register_plugin!(CollabPlugin, views);
