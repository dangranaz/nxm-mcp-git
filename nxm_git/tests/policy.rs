//! Tests for `nxm_git::policy` — the pure, side-effect-free policy check and
//! the `.nxm-git-policy.toml` parser. No git access anywhere in this file.

use nxm_git::policy::{DenialReason, DevelopDirectCommit, POLICY_FILE};
use nxm_git::scan::FORBIDDEN_PATTERNS;
use nxm_git::{check_policy, GitOp, Policy, PolicyFlags};
use tempfile::TempDir;

/// Policy protecting exactly `branches`, rest at spec defaults.
fn protected(branches: &[&str]) -> Policy {
    Policy {
        protected_branches: branches.iter().map(|b| b.to_string()).collect(),
        ..Policy::default()
    }
}

fn flags(confirm: bool, allow_force: bool, branch: Option<&str>) -> PolicyFlags {
    PolicyFlags {
        confirm,
        allow_force,
        target_branch: branch.map(str::to_string),
        source_branch: None,
    }
}

#[test]
fn defaults_match_spec() {
    let p = Policy::default();
    assert_eq!(p.protected_branches, ["main"]);
    assert!(!p.allow_force);
    assert!(p.destructive_confirm);
    assert_eq!(
        p.forbidden_patterns,
        FORBIDDEN_PATTERNS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    );
    assert!(p.require_pre_push.is_empty());
}

#[test]
fn load_missing_file_gives_default() {
    let dir = TempDir::new().unwrap();
    assert_eq!(Policy::load(dir.path()).unwrap(), Policy::default());
}

#[test]
fn load_parses_full_toml() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(POLICY_FILE),
        r#"
protected_branches = ["main", "release"]
allow_force = true
destructive_confirm = false
forbidden_patterns = ["custom-secret"]
require_pre_push = ["cargo test"]
"#,
    )
    .unwrap();
    let p = Policy::load(dir.path()).unwrap();
    assert_eq!(p.protected_branches, ["main", "release"]);
    assert!(p.allow_force);
    assert!(!p.destructive_confirm);
    assert_eq!(p.forbidden_patterns, ["custom-secret"]);
    assert_eq!(p.require_pre_push, ["cargo test"]);
}

#[test]
fn default_scan_exclude_covers_definer_and_tests() {
    let p = Policy::default();
    // The file that defines the patterns and all test trees are excluded;
    // nothing broad like `*.md` is.
    assert!(p.scan_exclude.contains(&"nxm_git/src/scan.rs".to_string()));
    assert!(p.scan_exclude.contains(&"**/tests/**".to_string()));
    assert!(
        !p.scan_exclude
            .iter()
            .any(|e| e == "*.md" || e == "Cargo.toml"),
        "default must not broadly exclude metadata: {:?}",
        p.scan_exclude
    );
}

#[test]
fn load_parses_scan_exclude_and_missing_is_default() {
    let dir = TempDir::new().unwrap();
    // Explicit value parses.
    std::fs::write(
        dir.path().join(POLICY_FILE),
        "scan_exclude = [\"foo/bar\", \"**/x/**\"]\n",
    )
    .unwrap();
    let p = Policy::load(dir.path()).unwrap();
    assert_eq!(p.scan_exclude, ["foo/bar", "**/x/**"]);
    // Absent key → spec default (serde(default) keeps old files parsing).
    std::fs::write(dir.path().join(POLICY_FILE), "allow_force = true\n").unwrap();
    let p = Policy::load(dir.path()).unwrap();
    assert_eq!(p.scan_exclude, Policy::default().scan_exclude);
}

#[test]
fn load_partial_file_fills_spec_defaults() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(POLICY_FILE),
        "protected_branches = [\"trunk\"]\n",
    )
    .unwrap();
    let p = Policy::load(dir.path()).unwrap();
    assert_eq!(p.protected_branches, ["trunk"]);
    assert!(!p.allow_force);
    assert!(p.destructive_confirm);
    assert_eq!(p.forbidden_patterns, Policy::default().forbidden_patterns);
    assert!(p.require_pre_push.is_empty());
}

#[test]
fn load_malformed_or_unknown_key_is_error() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(POLICY_FILE), "not toml at all [").unwrap();
    assert!(Policy::load(dir.path()).is_err());
    // A typo'd key must fail loudly, not silently fall back to defaults.
    std::fs::write(dir.path().join(POLICY_FILE), "allow_forc = true\n").unwrap();
    assert!(Policy::load(dir.path()).is_err());
}

/// Table test covering the T5 denial rules. `None` = allowed.
#[test]
fn check_policy_table() {
    let cases: Vec<(&str, GitOp, Policy, PolicyFlags, Option<DenialReason>)> = vec![
        // Push on protected "main": denied without confirm, allowed with it.
        (
            "push main w/o confirm",
            GitOp::Push,
            protected(&["main"]),
            flags(false, false, Some("main")),
            Some(DenialReason::ProtectedBranch),
        ),
        (
            "push main w/ confirm",
            GitOp::Push,
            protected(&["main"]),
            flags(true, false, Some("main")),
            None,
        ),
        (
            "push unprotected branch",
            GitOp::Push,
            protected(&["main"]),
            flags(false, false, Some("dev")),
            None,
        ),
        // Force push: denied when force is not enabled (policy or flag).
        (
            "force w/o allow_force",
            GitOp::ForcePush,
            protected(&["main"]),
            flags(false, false, Some("dev")),
            Some(DenialReason::ForceNotAllowed),
        ),
        (
            "force w/o allow_force, w/ confirm",
            GitOp::ForcePush,
            protected(&["main"]),
            flags(true, false, Some("dev")),
            Some(DenialReason::ForceNotAllowed),
        ),
        // Force push allowed with force enabled (policy or flag) AND confirm.
        (
            "force policy-enabled + confirm",
            GitOp::ForcePush,
            Policy {
                allow_force: true,
                ..protected(&["main"])
            },
            flags(true, false, Some("main")),
            None,
        ),
        (
            "force flag-enabled + confirm",
            GitOp::ForcePush,
            protected(&["main"]),
            flags(true, true, Some("main")),
            None,
        ),
        // With force enabled, confirm is ALSO required.
        (
            "force policy-enabled, no confirm",
            GitOp::ForcePush,
            Policy {
                allow_force: true,
                ..protected(&["main"])
            },
            flags(false, false, Some("dev")),
            Some(DenialReason::ForceNeedsConfirm),
        ),
        (
            "force flag-enabled, no confirm",
            GitOp::ForcePush,
            protected(&["main"]),
            flags(false, true, Some("dev")),
            Some(DenialReason::ForceNeedsConfirm),
        ),
        // Commit on protected branch: denied without confirm.
        (
            "commit main w/o confirm",
            GitOp::Commit,
            protected(&["main"]),
            flags(false, false, Some("main")),
            Some(DenialReason::ProtectedBranch),
        ),
        (
            "commit main w/ confirm",
            GitOp::Commit,
            protected(&["main"]),
            flags(true, false, Some("main")),
            None,
        ),
        (
            "commit unprotected branch",
            GitOp::Commit,
            protected(&["main"]),
            flags(false, false, Some("dev")),
            None,
        ),
        // Fail-safe: branch-targeting op without target_branch is denied.
        (
            "commit without target_branch",
            GitOp::Commit,
            protected(&["main"]),
            flags(false, false, None),
            Some(DenialReason::UnknownBranch),
        ),
        (
            "push without target_branch",
            GitOp::Push,
            protected(&["main"]),
            flags(false, false, None),
            Some(DenialReason::UnknownBranch),
        ),
        // CreateBranch/Switch rewrite no history: unrestricted in T5.
        (
            "create branch",
            GitOp::CreateBranch,
            protected(&["main"]),
            flags(false, false, Some("main")),
            None,
        ),
        (
            "switch branch",
            GitOp::Switch,
            protected(&["main"]),
            flags(false, false, Some("main")),
            None,
        ),
    ];

    for (name, op, policy, flags, expected) in cases {
        match (expected, check_policy(op, &policy, &flags)) {
            (None, Ok(())) => {}
            (None, Err(denied)) => panic!("{name}: expected Ok, got {denied}"),
            (Some(reason), Err(denied)) => {
                assert_eq!(denied.reason, reason, "{name}");
                assert_eq!(denied.op, op, "{name}");
            }
            (Some(_), Ok(())) => panic!("{name}: expected denial {expected:?}, got Ok"),
        }
    }
}

fn workstream_policy() -> Policy {
    Policy {
        integration_branches: vec!["integration/*".to_string()],
        ..Policy::default()
    }
}

#[test]
fn policy_defaults_are_backcompat() {
    let p = Policy::default();
    assert!(p.integration_branches.is_empty());
    assert_eq!(p.develop_direct_commit, DevelopDirectCommit::Confirm);
    assert_eq!(p.develop_branch, "develop");
    assert_eq!(p.work_branch_prefix, "work/");
    // A policy loaded from a toml without the new keys behaves as today:
    // a direct commit on develop (not protected) is allowed.
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(POLICY_FILE),
        "protected_branches = [\"main\"]\n",
    )
    .unwrap();
    let p = Policy::load(dir.path()).unwrap();
    assert!(check_policy(
        GitOp::Commit,
        &p,
        &PolicyFlags {
            source_branch: None,
            ..flags(false, false, Some("develop"))
        },
    )
    .is_ok());
}

#[test]
fn unknown_field_still_errors() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(POLICY_FILE), "bogus_key = 1\n").unwrap();
    assert!(Policy::load(dir.path()).is_err());
    // A misspelled new key is rejected, not silently ignored.
    std::fs::write(
        dir.path().join(POLICY_FILE),
        "integration_branch = [\"integration/*\"]\n",
    )
    .unwrap();
    assert!(Policy::load(dir.path()).is_err());
}

#[test]
fn develop_direct_commit_parses_deny_and_confirm() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(POLICY_FILE),
        "develop_direct_commit = \"deny\"\n",
    )
    .unwrap();
    assert_eq!(
        Policy::load(dir.path()).unwrap().develop_direct_commit,
        DevelopDirectCommit::Deny
    );
    std::fs::write(
        dir.path().join(POLICY_FILE),
        "develop_direct_commit = \"confirm\"\n",
    )
    .unwrap();
    assert_eq!(
        Policy::load(dir.path()).unwrap().develop_direct_commit,
        DevelopDirectCommit::Confirm
    );
    std::fs::write(
        dir.path().join(POLICY_FILE),
        "develop_direct_commit = \"maybe\"\n",
    )
    .unwrap();
    assert!(Policy::load(dir.path()).is_err());
}

#[test]
fn direct_commit_on_develop_without_confirm_denied() {
    let policy = workstream_policy();
    let denied = check_policy(
        GitOp::Commit,
        &policy,
        &PolicyFlags {
            source_branch: None,
            ..flags(false, false, Some("develop"))
        },
    )
    .unwrap_err();
    assert_eq!(denied.reason, DenialReason::DevelopDirectCommit);
    assert!(check_policy(
        GitOp::Commit,
        &policy,
        &PolicyFlags {
            source_branch: None,
            ..flags(true, false, Some("develop"))
        },
    )
    .is_ok());
}

#[test]
fn direct_commit_on_develop_deny_mode_blocks_even_with_confirm() {
    let mut policy = workstream_policy();
    policy.develop_direct_commit = DevelopDirectCommit::Deny;
    let denied = check_policy(
        GitOp::Commit,
        &policy,
        &PolicyFlags {
            source_branch: Some("feature/x".to_string()),
            ..flags(true, false, Some("develop"))
        },
    )
    .unwrap_err();
    assert_eq!(denied.reason, DenialReason::DevelopDirectCommit);
}

#[test]
fn merge_from_integration_into_develop_allowed() {
    let policy = workstream_policy();
    assert!(check_policy(
        GitOp::Commit,
        &policy,
        &PolicyFlags {
            source_branch: Some("integration/foo".to_string()),
            ..flags(false, false, Some("develop"))
        },
    )
    .is_ok());
}

#[test]
fn merge_from_integration_into_develop_bypasses_deny_mode() {
    let mut policy = workstream_policy();
    policy.develop_direct_commit = DevelopDirectCommit::Deny;
    assert!(check_policy(
        GitOp::Commit,
        &policy,
        &PolicyFlags {
            source_branch: Some("integration/foo".to_string()),
            ..flags(false, false, Some("develop"))
        },
    )
    .is_ok());
}

#[test]
fn work_branch_from_main_denied() {
    let policy = workstream_policy();
    let denied = check_policy(
        GitOp::CreateBranch,
        &policy,
        &PolicyFlags {
            source_branch: Some("main".to_string()),
            ..flags(false, false, Some("work/07-x"))
        },
    )
    .unwrap_err();
    assert_eq!(denied.reason, DenialReason::WorkBranchBaseNotAllowed);
}

#[test]
fn work_branch_from_integration_allowed() {
    let policy = workstream_policy();
    assert!(check_policy(
        GitOp::CreateBranch,
        &policy,
        &PolicyFlags {
            source_branch: Some("integration/foo".to_string()),
            ..flags(false, false, Some("work/08-x"))
        },
    )
    .is_ok());
}

#[test]
fn work_branch_from_develop_needs_confirm() {
    let policy = workstream_policy();
    assert!(check_policy(
        GitOp::CreateBranch,
        &policy,
        &PolicyFlags {
            source_branch: Some("develop".to_string()),
            ..flags(false, false, Some("work/08-x"))
        },
    )
    .is_err());
    assert!(check_policy(
        GitOp::CreateBranch,
        &policy,
        &PolicyFlags {
            source_branch: Some("develop".to_string()),
            ..flags(true, false, Some("work/08-x"))
        },
    )
    .is_ok());
}

#[test]
fn work_branch_without_source_denied() {
    let policy = workstream_policy();
    let denied = check_policy(
        GitOp::CreateBranch,
        &policy,
        &PolicyFlags {
            source_branch: None,
            ..flags(false, false, Some("work/09-x"))
        },
    )
    .unwrap_err();
    assert_eq!(denied.reason, DenialReason::UnknownBase);
}

#[test]
fn empty_integration_branches_disables_new_guards() {
    // integration_branches empty: no new guard
    let policy = Policy::default();
    // Commit on develop without confirm: allowed, as today.
    assert!(check_policy(
        GitOp::Commit,
        &policy,
        &flags(false, false, Some("develop"))
    )
    .is_ok());
    // Work branch from main: allowed, no new guard active.
    assert!(check_policy(
        GitOp::CreateBranch,
        &policy,
        &PolicyFlags {
            source_branch: Some("main".to_string()),
            ..flags(false, false, Some("work/10-x"))
        },
    )
    .is_ok());
}
