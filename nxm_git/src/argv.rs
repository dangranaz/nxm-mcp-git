//! Shared helpers for building safe git/gh argument arrays.

use anyhow::Result;

/// Reject anything git/gh would parse as an option (`--force`, `-u`, ...)
/// rather than a ref or name. Shared by every shell-out so a caller-supplied
/// branch/remote/base can never be smuggled in as a flag (argument injection).
pub fn ensure_ref_arg(name: &str) -> Result<()> {
    anyhow::ensure!(
        !name.is_empty() && !name.starts_with('-'),
        "invalid git name: {name:?}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_names_rejects_option_like_and_empty() {
        assert!(ensure_ref_arg("main").is_ok());
        assert!(ensure_ref_arg("feature/x").is_ok());
        assert!(ensure_ref_arg("origin").is_ok());
        assert!(ensure_ref_arg("").is_err());
        assert!(ensure_ref_arg("--force").is_err());
        assert!(ensure_ref_arg("-u").is_err());
    }
}
