//! Sandboxed, state-driven Collaboration panel.
//!
//! Navigation messages are deliberately presentation-only.  The host marks
//! HTML view messages non-interactive, so networking and file operations stay
//! behind native toolbar buttons, where the user made an explicit IDE action.
use crate::state::Session;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Start,
    Session,
    Documents,
    Activity,
    Settings,
}

impl Page {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "start" => Self::Start,
            "session" => Self::Session,
            "documents" => Self::Documents,
            "activity" => Self::Activity,
            "settings" => Self::Settings,
            _ => return None,
        })
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Session => "session",
            Self::Documents => "documents",
            Self::Activity => "activity",
            Self::Settings => "settings",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Começar",
            Self::Session => "Sessão",
            Self::Documents => "Documentos",
            Self::Activity => "Atividade",
            Self::Settings => "Configurações",
        }
    }
}

pub fn html(
    page: Page,
    session: Option<&Session>,
    relay: &str,
    realtime_status: Option<&str>,
    error: Option<&str>,
    invitation: Option<&str>,
    activity: &[String],
) -> String {
    let nav = [
        Page::Start,
        Page::Session,
        Page::Documents,
        Page::Activity,
        Page::Settings,
    ]
    .into_iter()
    .map(|candidate| {
        let selected = (candidate == page)
            .then_some(" aria-current=\"page\"")
            .unwrap_or("");
        format!(
            "<button class=\"tab{}\" type=\"button\" onclick=\"go('{}')\"{}>{}</button>",
            if candidate == page { " active" } else { "" },
            candidate.id(),
            selected,
            candidate.label()
        )
    })
    .collect::<String>();
    let notice = error
        .map(|error| format!("<div class=\"notice error\">{}</div>", escape(error)))
        .unwrap_or_default();
    let state = realtime_status.unwrap_or("Preparada para conectar");
    let content = match page {
        Page::Start => start(session, relay),
        Page::Session => session_page(session, state, invitation),
        Page::Documents => documents(session),
        Page::Activity => activity_page(activity),
        Page::Settings => settings(relay),
    };
    format!(
        r#"<main class="collab" aria-label="OpenIDEA Collab">
<style>
:root {{ color-scheme: light dark; }}
* {{ box-sizing:border-box; }}
body {{ margin:0; background:transparent; font:13px/1.45 system-ui,sans-serif; color:light-dark(#17202a,#e8edf5); }}
.collab {{ --surface:light-dark(#ffffff,#20252b); --soft:light-dark(#edf5f6,#192f33); --line:light-dark(#d9e3e7,#354149); --muted:light-dark(#667580,#a8b5c0); --accent:light-dark(#087d8e,#3bc7c9); --danger:light-dark(#b42342,#ff94a3); padding:16px; }}
.top {{ display:flex; justify-content:space-between; gap:12px; align-items:center; margin-bottom:15px; }}
.brand {{ display:flex; align-items:center; gap:9px; font-weight:600; letter-spacing:-.01em; }}
.mark {{ width:27px; height:27px; display:grid; place-items:center; border-radius:9px; color:#fff; background:linear-gradient(135deg,var(--accent),#4275ce); }}
.state {{ display:flex; align-items:center; gap:6px; color:var(--muted); font-size:12px; }}
.dot {{ width:7px; height:7px; border-radius:50%; background:var(--accent); animation:pulse 1.8s ease-in-out infinite; }}
@keyframes pulse {{ 50% {{ box-shadow:0 0 0 6px color-mix(in srgb,var(--accent) 0%,transparent); }} 0%,100% {{ box-shadow:0 0 0 1px color-mix(in srgb,var(--accent) 35%,transparent); }} }}
@media (prefers-reduced-motion:reduce) {{ .dot {{ animation:none; }} }}
.tabs {{ display:flex; overflow:auto; gap:3px; margin:0 -2px 18px; padding:0 2px; border-bottom:1px solid var(--line); }}
.tab {{ appearance:none; border:0; border-bottom:2px solid transparent; background:transparent; color:var(--muted); padding:8px 9px 9px; white-space:nowrap; font:inherit; cursor:pointer; }}
.tab.active {{ color:inherit; border-color:var(--accent); font-weight:600; }}
.card {{ border:1px solid var(--line); border-radius:12px; background:var(--surface); padding:16px; }}
.hero {{ padding:5px 1px 16px; }} .eyebrow {{ color:var(--accent); font-size:11px; font-weight:700; letter-spacing:.08em; text-transform:uppercase; }}
h1 {{ margin:6px 0 8px; font-size:23px; line-height:1.16; letter-spacing:-.035em; }} h2 {{ margin:0 0 11px; font-size:15px; }} p {{ margin:0; color:var(--muted); }}
.steps,.list {{ display:grid; gap:0; }} .row {{ display:flex; align-items:flex-start; gap:10px; padding:12px 0; border-top:1px solid var(--line); }} .row:first-child {{ padding-top:0; border-top:0; }} .num,.avatar {{ width:24px; height:24px; flex:none; display:grid; place-items:center; border-radius:50%; background:var(--soft); color:var(--accent); font-size:11px; font-weight:700; }}
.row-main {{ min-width:0; flex:1; }} .row-title {{ font-weight:600; }} .detail {{ margin-top:2px; color:var(--muted); font-size:12px; overflow-wrap:anywhere; }} .pill {{ flex:none; border-radius:99px; padding:2px 7px; background:var(--soft); color:var(--accent); font-size:11px; }}
.notice {{ margin-bottom:14px; border-radius:9px; padding:10px 11px; font-size:12px; }} .notice.error {{ background:color-mix(in srgb,var(--danger) 11%,transparent); color:var(--danger); }}
.grid {{ display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:12px; }} .label {{ color:var(--muted); font-size:11px; }} .value {{ margin-top:3px; font-weight:600; overflow-wrap:anywhere; }}
.hint {{ margin-top:14px; color:var(--muted); font-size:12px; }} code {{ font:11px ui-monospace,SFMono-Regular,monospace; }}
@media (max-width:360px) {{ .grid {{ grid-template-columns:1fr; }} h1 {{ font-size:21px; }} }}
</style>
<header class="top"><div class="brand"><span class="mark">C</span>OpenIDEA Collab</div><div class="state"><span class="dot"></span>{state}</div></header>
<nav class="tabs" aria-label="Colaboração">{nav}</nav>{notice}{content}
<script>function go(page){{openidea.postMessage(JSON.stringify({{op:'page',page:page}}));}}</script>
</main>"#,
        state = escape(state),
    )
}

fn start(session: Option<&Session>, relay: &str) -> String {
    if session.is_some() {
        return "<section class=\"hero\"><div class=\"eyebrow\">Sessão preparada</div><h1>Você já pode iniciar a colaboração.</h1><p>Use as abas para acompanhar pessoas, documentos compartilhados e atividade.</p></section>".into();
    }
    format!(
        "<section class=\"hero\"><div class=\"eyebrow\">Colaboração explícita</div><h1>Edite junto. Compartilhe apenas o que escolher.</h1><p>Prepare uma sessão ou aceite um convite. O plugin nunca varre o projeto: somente documentos abertos e confirmados entram na sessão.</p></section><section class=\"card steps\"><div class=\"row\"><span class=\"num\">1</span><div class=\"row-main\"><div class=\"row-title\">Prepare ou entre</div><div class=\"detail\">Cada participante recebe sua própria credencial.</div></div></div><div class=\"row\"><span class=\"num\">2</span><div class=\"row-main\"><div class=\"row-title\">Conecte o realtime</div><div class=\"detail\">A conexão usa TLS com {}</div></div></div><div class=\"row\"><span class=\"num\">3</span><div class=\"row-main\"><div class=\"row-title\">Confirme os documentos</div><div class=\"detail\">O conteúdo só é compartilhado após a confirmação.</div></div></div></section>",
        escape(relay)
    )
}

fn session_page(session: Option<&Session>, state: &str, invitation: Option<&str>) -> String {
    let Some(session) = session else {
        return "<section class=\"card\"><h2>Nenhuma sessão neste projeto</h2><p>Volte a Começar para preparar uma sessão ou aceitar um convite.</p></section>".into();
    };
    let invite = invitation.map(|url| format!("<div class=\"row\"><div class=\"row-main\"><div class=\"row-title\">Convite criado</div><div class=\"detail\"><code>{}</code></div></div></div>", escape(url))).unwrap_or_default();
    format!(
        "<section class=\"card\"><div class=\"grid\"><div><div class=\"label\">Papel</div><div class=\"value\">{:?}</div></div><div><div class=\"label\">Realtime</div><div class=\"value\">{}</div></div></div><div class=\"hint\">Convide até dois colaboradores. As ações ficam na barra superior do painel.</div>{}</section>",
        session.role,
        escape(state),
        invite
    )
}

fn documents(session: Option<&Session>) -> String {
    let Some(session) = session else {
        return "<section class=\"card\"><h2>Nenhum documento compartilhado</h2><p>Prepare uma sessão e confirme o documento aberto para começar.</p></section>".into();
    };
    let rows: String = if session.documents.is_empty() {
        "<div class=\"row\"><div class=\"row-main\"><div class=\"row-title\">Nada compartilhado ainda</div><div class=\"detail\">Use “Compartilhar documento” na barra superior.</div></div></div>".into()
    } else {
        session.documents.iter().map(|document| format!("<div class=\"row\"><span class=\"avatar\">↔</span><div class=\"row-main\"><div class=\"row-title\">{}</div><div class=\"detail\">Compartilhado explicitamente · sincronização protegida</div></div><span class=\"pill\">Ativo</span></div>", escape(&document.path))).collect()
    };
    format!("<section class=\"card list\">{rows}</section>")
}

fn activity_page(activity: &[String]) -> String {
    let rows: String = if activity.is_empty() {
        "<div class=\"row\"><div class=\"row-main\"><div class=\"row-title\">Aguardando atividade</div><div class=\"detail\">Conexões, compartilhamentos e recusas de segurança aparecerão aqui nesta sessão.</div></div></div>".into()
    } else {
        activity.iter().take(8).map(|line| format!("<div class=\"row\"><span class=\"avatar\">•</span><div class=\"row-main\"><div class=\"row-title\">{}</div></div></div>", escape(line))).collect()
    };
    format!("<section class=\"card list\">{rows}</section>")
}

fn settings(relay: &str) -> String {
    format!(
        "<section class=\"card\"><div class=\"grid\"><div><div class=\"label\">Relay atual</div><div class=\"value\">{}</div></div><div><div class=\"label\">Credenciais</div><div class=\"value\">Keychain do sistema</div></div></div><div class=\"hint\">Para usar um relay próprio, abra Configurações › OpenIDEA Collab e altere “Relay origin”. A origem é validada antes de qualquer conexão.</div></section>",
        escape(relay)
    )
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_whitelisted_and_render_navigation() {
        assert_eq!(Page::parse("documents"), Some(Page::Documents));
        assert_eq!(Page::parse("<script>"), None);
        let page = html(
            Page::Start,
            None,
            "https://relay.example",
            None,
            None,
            None,
            &[],
        );
        assert!(page.contains("openidea.postMessage"));
        assert!(page.contains("Compartilhe apenas o que escolher"));
    }

    #[test]
    fn untrusted_paths_and_errors_are_html_escaped() {
        let session = Session {
            schema: 2,
            project: [0; 32],
            relay: "https://relay.example".into(),
            session_id: uuid::Uuid::nil(),
            member_id: uuid::Uuid::nil(),
            role: openidea_collab_protocol::Role::Owner,
            expires_at: 0,
            documents: vec![crate::state::SharedDocument {
                path: "<script>alert(1)</script>.rs".into(),
                document_id: uuid::Uuid::nil(),
            }],
        };
        let page = html(
            Page::Documents,
            Some(&session),
            "https://relay.example",
            None,
            Some("<unsafe>"),
            None,
            &[],
        );
        assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;.rs"));
        assert!(page.contains("&lt;unsafe&gt;"));
        assert!(!page.contains("<unsafe>"));
    }
}
