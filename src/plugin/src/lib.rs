//! Secure relay control plane. Realtime deliberately stays disabled until the
//! public host supports background sockets and native document transactions.
pub mod client;
pub mod state;
use openidea_collab_protocol::{DEFAULT_ORIGIN, Role};
use openidea_plugin::{
    Context, Error, Invocation, Plugin, host, register_plugin,
    views::{Body, Button, Field, FieldValue, View, ViewEvent, Viewport, Views},
};
const PANEL: &str = "collab-panel";
const LIMITATION: &str = "Native realtime is not available on this host API. Session preparation sends no project files. WebSocket and native document APIs are required before editing can be shared.";
fn limitation() -> &'static str {
    if host::locale().starts_with("pt") {
        "A edição em tempo real ainda precisa de novas APIs públicas da IDE. Preparar uma sessão não envia arquivos do projeto."
    } else {
        LIMITATION
    }
}
#[derive(Default)]
pub struct CollabPlugin {
    invitation: Option<String>,
}
impl Plugin for CollabPlugin {
    fn activate(&mut self, _context: &Context) -> Result<(), Error> {
        // Remove the unsafe legacy auto-restored room. No network or file access.
        host::storage::delete("room_state")?;
        Ok(())
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
                    &format!("Relay: {DEFAULT_ORIGIN}\n{}", limitation()),
                )? {
                    return Ok(None);
                }
                let credential = if id == "create-session" {
                    client::create()?
                } else {
                    let Some(invitation) = host::input("Paste the invitation URL", None)? else {
                        return Ok(None);
                    };
                    client::join(invitation.trim())?
                };
                state::save(ctx, credential)?;
            }
            "create-invite" => {
                let session = state::for_project(ctx)?
                    .ok_or_else(|| Error::Invalid("Prepare a session first".into()))?;
                if session.role != Role::Owner {
                    return Err(Error::Denied("Only the session owner can invite".into()));
                }
                self.invitation = Some(client::invite(&session.session_id.to_string())?.url);
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
                        &session.session_id.to_string(),
                        &session.member_id.to_string(),
                    )?;
                    state::forget()?;
                }
                self.invitation = None;
            }
            _ => return Err(Error::NotFound("Unknown collaboration action".into())),
        }
        host::refresh_view(PANEL);
        Ok(None)
    }
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
        let mut fields = vec![
            Field::new("relay", "Relay", FieldValue::label(DEFAULT_ORIGIN)),
            Field::new("status", "Status", FieldValue::label(limitation())),
        ];
        let session = state::for_project(ctx);
        if !matches!(&session, Ok(Some(_))) {
            self.invitation = None;
        }
        if let Some(invitation) = &self.invitation {
            fields.push(Field::new(
                "invitation",
                "Invite one of up to two collaborators (single use, expires in 5 minutes)",
                FieldValue::readonly_text(invitation, false),
            ));
        }
        let mut page = View::new(Body::Form(fields));
        match session {
            Ok(Some(session)) => {
                if session.role == Role::Owner {
                    page =
                        page.button(Button::new("create-invite", "Create Invitation").icon("add"));
                }
                page = page.button(Button::new("leave-session", "Leave Session").icon("close"));
            }
            Ok(None) => {
                page = page
                    .button(Button::new("create-session", "Prepare Session").icon("add"))
                    .button(Button::new("join-session", "Accept Invitation").icon("arrow-right"));
            }
            Err(_) => {
                page = page.status("Open a project, or leave the session associated with the previous project.")
                .button(Button::new("leave-session", "Leave Previous Session").icon("close"));
            }
        }
        Ok(page)
    }
    fn handle(&mut self, view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        if view != PANEL {
            return Ok(());
        }
        if let ViewEvent::Button(id) = event {
            self.run_command(&id, ctx, None)?;
        }
        Ok(())
    }
}
register_plugin!(CollabPlugin);
