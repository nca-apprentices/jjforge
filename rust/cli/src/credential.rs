//! The person's SSH key and the certificate an authority signed for it, as
//! ADR 0002 decides. `JJFORGE_KEY` names the private key, or its public key
//! when ssh-agent holds the private one, and the certificate sits next to it
//! as `{key}-cert.pub`, where `ssh-keygen -s` writes it.
//!
//! `jf` signs with `ssh-keygen -Y sign`, as jj's SSH signing does, so a key
//! in ssh-agent or behind a passphrase works the same as a plain file.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context as _;

/// The namespace of a session's signature, which vcs checks.
const NAMESPACE: &str = "jjforge-session";

#[derive(Debug, Clone)]
pub(crate) struct Credential {
    key: PathBuf,
}

impl Credential {
    /// The key that `JJFORGE_KEY` names, or `None` when it names none.
    pub(crate) fn from_env() -> Option<Self> {
        std::env::var_os("JJFORGE_KEY").map(|key| Self { key: key.into() })
    }

    fn certificate_path(&self) -> PathBuf {
        let key = self.key.to_string_lossy();
        let stem = key.strip_suffix(".pub").unwrap_or(&key);
        PathBuf::from(format!("{stem}-cert.pub"))
    }

    /// The certificate in OpenSSH form.
    pub(crate) fn certificate(&self) -> anyhow::Result<String> {
        let path = self.certificate_path();
        std::fs::read_to_string(&path)
            .with_context(|| format!("No certificate at {}", path.display()))
    }

    /// The SSHSIG of `data` in the session namespace.
    pub(crate) fn sign(&self, data: &[u8]) -> anyhow::Result<String> {
        let mut child = Command::new("ssh-keygen")
            .args(["-Y", "sign", "-n", NAMESPACE, "-f"])
            .arg(&self.key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to run ssh-keygen")?;
        child
            .stdin
            .take()
            .context("ssh-keygen has no input")?
            .write_all(data)?;
        let output = child.wait_with_output()?;
        anyhow::ensure!(
            output.status.success(),
            "ssh-keygen failed to sign with {}: {}",
            self.key.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
        Ok(String::from_utf8(output.stdout)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_certificate_sits_next_to_the_key_or_its_public_half() {
        for key in ["/home/ada/.ssh/id_ed25519", "/home/ada/.ssh/id_ed25519.pub"] {
            let credential = Credential { key: key.into() };

            assert_eq!(
                credential.certificate_path(),
                PathBuf::from("/home/ada/.ssh/id_ed25519-cert.pub")
            );
        }
    }

    #[test]
    fn ssh_keygen_signs_with_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("id_ed25519");
        let made = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&key)
            .status()
            .unwrap();
        assert!(made.success());

        let signature = Credential { key }.sign(b"nonce").unwrap();

        assert!(signature.starts_with("-----BEGIN SSH SIGNATURE-----"));
    }
}
