//! Openidea Collab: an OpenIDEA plugin.

use openidea_plugin::{Error, Invocation, NotificationLevel, Plugin, host, register_plugin};

#[derive(Default)]
pub struct Main {
    greetings: u32,
}

impl Plugin for Main {
    fn run_command(&mut self, id: &str, ctx: &Invocation, _arg: Option<&str>) -> Result<Option<String>, Error> {
        match id {
            "say-hello" => {
                // Instance state lasts until the plugin is idle for a while;
                // keep what must survive in `host::storage`.
                self.greetings += 1;
                let body = format!("Hello #{} from Openidea Collab!", self.greetings);
                host::notify(NotificationLevel::Info, "Openidea Collab", &body);
                Ok(None)
            }
            "line-count" => {
                let editor = ctx.editor().ok_or("no file is open")?;
                let text = editor.document()?.text;
                Ok(Some(format!("{} lines in {}", text.lines().count(), editor.path())))
            }
            _ => Err(format!("unknown command {id}").into()),
        }
    }
}

register_plugin!(Main);

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn says_hello() {
        let host = MockHost::install();
        let mut plugin = Main::default();
        plugin.run_command("say-hello", &host.invocation(), None).unwrap();
        plugin.run_command("say-hello", &host.invocation(), None).unwrap();
        assert_eq!(host.notifications()[1].body, "Hello #2 from Openidea Collab!");
    }

    #[test]
    fn counts_lines() {
        let host = MockHost::install();
        host.open_editor("/project/src/main.rs", "rust", "fn main() {\n}\n", &[]);
        let out = Main::default().run_command("line-count", &host.invocation(), None).unwrap();
        assert_eq!(out.as_deref(), Some("2 lines in /project/src/main.rs"));
    }
}
