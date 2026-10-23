//! `require_pre_push` hard gate for [`crate::write::push`].
//!
//! Each command from [`crate::policy::Policy::require_pre_push`] runs inside
//! the repository as an argv array — never as an interpolated shell string.
//! An empty entry or an empty list is a no-op; the first command that fails
//! (non-zero exit or spawn error) aborts the push with a structured
//! [`PrePushError`], so `git push` never runs.

use std::process::Command;

use thiserror::Error;
use tracing::info;

use crate::resolve::ResolvedRepo;

/// A `require_pre_push` command failed, so the push was not run.
#[derive(Debug, Error)]
#[error("pre-push command {command:?} failed: {stderr}")]
pub struct PrePushError {
    /// The command line that failed.
    pub command: String,
    /// stderr of the failed command (or the spawn error message).
    pub stderr: String,
}

impl PrePushError {
    /// Structured JSON payload for MCP clients: `kind` plus details.
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "pre_push_failed",
            "command": self.command,
            "stderr": self.stderr,
        })
    }
}

/// Run the configured commands in `repo`, aborting on the first failure.
///
/// Each entry is split on whitespace into argv: empty entries are skipped.
/// Entry/exit of every command is logged on the `nxm::git::write` target,
/// including the failing one.
pub fn run(repo: &ResolvedRepo, commands: &[String]) -> Result<(), PrePushError> {
    for command in commands {
        let mut argv = command.split_whitespace();
        let Some(program) = argv.next() else {
            continue;
        };
        info!(target: "nxm::git::write", op = "pre_push", command = %command, "pre-push ENTER");
        let result = match Command::new(program)
            .args(argv)
            .current_dir(&repo.path)
            .output()
        {
            Err(e) => Err(PrePushError {
                command: command.clone(),
                stderr: e.to_string(),
            }),
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => Err(PrePushError {
                command: command.clone(),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            }),
        };
        info!(target: "nxm::git::write", op = "pre_push", command = %command, "pre-push EXIT");
        result?;
    }
    Ok(())
}
