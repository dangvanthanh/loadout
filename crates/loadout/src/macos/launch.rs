//! Opens an agent in a folder (U9). Blocking; run on the background executor.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use loadout_core::launch::{self, Launch};

/// Errors are phrased for the palette footer.
pub fn open(agent: &str, dir: &Path, custom: &BTreeMap<String, String>) -> Result<(), String> {
    let name = launch::agent_name(agent);
    let launcher = launch::launcher(agent, dir, custom)
        .ok_or_else(|| format!("Loadout doesn't know how to open {name} yet."))?;
    match launcher {
        Launch::App(app) => {
            let ok = Command::new("/usr/bin/open")
                .args(["-a", app])
                .arg(dir)
                .stdin(Stdio::null())
                .output()
                .is_ok_and(|out| out.status.success());
            ok.then_some(())
                .ok_or_else(|| format!("Couldn't open {app}. Is it installed?"))
        }
        Launch::Terminal(command) => {
            // `command` comes from the built-in table, never from the user.
            let found = Command::new("/bin/zsh")
                .args(["-lc", &format!("command -v {command}")])
                .stdin(Stdio::null())
                .output()
                .is_ok_and(|out| out.status.success());
            if !found {
                return Err(format!("{name} isn't installed ({command} wasn't found)."));
            }
            let script = launch::script_path(agent).ok_or("Couldn't find the cache folder.")?;
            let write = || -> std::io::Result<()> {
                std::fs::create_dir_all(script.parent().unwrap_or(Path::new("/tmp")))?;
                std::fs::write(&script, launch::terminal_script(command, dir))?;
                std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            };
            write().map_err(|err| format!("Couldn't open {name}: {err}"))?;
            let ok = Command::new("/usr/bin/open")
                .arg(&script)
                .stdin(Stdio::null())
                .output()
                .is_ok_and(|out| out.status.success());
            ok.then_some(())
                .ok_or_else(|| format!("Couldn't open a terminal for {name}."))
        }
        Launch::Custom(command) => {
            let ok = Command::new("/bin/zsh")
                .args(["-lc", &command])
                .stdin(Stdio::null())
                .output()
                .is_ok_and(|out| out.status.success());
            ok.then_some(())
                .ok_or_else(|| format!("{name}'s launch command failed."))
        }
    }
}
