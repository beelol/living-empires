//! Bounded reader for the host's one committed layout-policy file.
//! The caller polls on its existing content boundary, stages into each faction's
//! long-lived planner and reports the ACTIVE decision hash separately.
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{fs::OpenOptions, io::Read, path::PathBuf};

#[derive(Debug)]
pub struct PolicySource {
    pub source: String,
    /// Observed bytes, not proof that this policy has been promoted.
    pub sha256: String,
}

pub struct PolicyFile {
    path: PathBuf,
    observed_hash: Option<String>,
}
impl PolicyFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            observed_hash: None,
        }
    }

    /// Returns each changed source once, even if Lua later rejects it. Missing
    /// legacy-pack files return an error; caller retains the packaged/active
    /// policy and an explicit status rather than resetting any city.
    pub fn poll(&mut self) -> Result<Option<PolicySource>> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options
            .open(&self.path)
            .context("cannot open city layout policy")?;
        let metadata = file
            .metadata()
            .context("cannot inspect city layout policy")?;
        ensure!(
            metadata.is_file(),
            "city layout policy must be a regular file"
        );
        ensure!(metadata.len() <= 65536, "city layout policy exceeds 64 KiB");
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .context("cannot read city layout policy")?;
        ensure!(bytes.len() <= 65536, "city layout policy exceeds 64 KiB");
        let source = String::from_utf8(bytes).context("city layout policy is not UTF-8")?;
        let sha256 = format!("{:x}", Sha256::digest(source.as_bytes()));
        if self.observed_hash.as_ref() == Some(&sha256) {
            return Ok(None);
        }
        self.observed_hash = Some(sha256.clone());
        Ok(Some(PolicySource { source, sha256 }))
    }
}
