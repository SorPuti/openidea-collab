use ide_host::plugins::{Consent, PluginEvent, PluginHost, PluginWorkspace, web::WebPtyCommand};
use ide_plugin::{HostError, Limits};
use std::{path::{Path, PathBuf}, sync::Arc};

struct Workspace(PathBuf);
impl PluginWorkspace for Workspace {
    fn projects(&self) -> Vec<PathBuf> { vec![self.0.clone()] }
    fn buffer_text(&self, _: &Path) -> Option<String> { None }
    fn document(&self, _: &Path) -> Option<(u64, String)> { None }
    fn apply_edits(&self, _: &Path, _: u64, _: &[ide_core::TextEdit]) -> Result<u64, HostError> { Err(HostError::NotFound("test".into())) }
    fn trusted(&self, root: &Path) -> bool { root == self.0 }
    fn project_of(&self, _: &str) -> Option<PathBuf> { Some(self.0.clone()) }
    fn publish_diagnostics(&self, _: &str, _: &Path, _: Vec<ide_core::Diagnostic>) -> Result<(), HostError> { Ok(()) }
}

#[test]
fn actual_host_resolves_the_custom_editor_and_requires_approved_helper() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let plugin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../openidea-collab").canonicalize().unwrap();
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../collab-helper/target/debug/openidea-collab-helper").canonicalize().unwrap();
    let host = PluginHost::new(root.path().join("plugins"), Arc::new(|_: Option<&str>, _: PluginEvent| {}), Limits::default()).unwrap();
    host.set_workspace(Some(Arc::new(Workspace(project.clone()))));
    host.load_all(); host.startup_finished();
    let id = "sorputi.openidea-collab";
    host.install_from_dir(&plugin, &Consent::default()).unwrap();
    let view = "collab-experimental";
    let page = host.web_page(id, view).unwrap();
    assert!(page.path.ends_with("/web/collab-editor/index.html"));
    let command = || WebPtyCommand { program: "openidea-collab-helper".into(), args: vec![], env: Default::default(), cols: 100, rows: 28 };
    assert!(host.web_pty_options(id, view, "main", command()).is_err());
    host.choose_program(id, "process:openidea-collab-helper", Some(&project), Some(&helper)).unwrap();
    let options = host.web_pty_options(id, view, "main", command()).unwrap();
    assert_eq!(PathBuf::from(options.shell.unwrap()), helper);
    assert_eq!(options.cwd.0, project);
    assert!(host.web_pty_options(id, view, "main", WebPtyCommand { program: "sh".into(), ..command() }).is_err());
    host.set_capability(id, "process:openidea-collab-helper", false).unwrap();
    assert!(host.web_pty_options(id, view, "main", command()).is_err());
}
