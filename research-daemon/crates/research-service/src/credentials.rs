use std::fs;
use std::path::PathBuf;

use research_core::Config;
use secretspec::{NamedResolution, SecretSpecError, Secrets};

use crate::ServiceError;

const REASON: &str = "research-daemon live provider calls";

/// Resolve the provider key from the environment first, then from SecretSpec.
///
/// SecretSpec uses the provider configured in `~/.config/secretspec/config.toml`
/// (the system keyring by default). The key is never written to a file or log.
pub fn resolve_api_key(config: &Config) -> Result<String, ServiceError> {
    if let Ok(key) = config.api_key() {
        return Ok(key);
    }

    let name = &config.provider.tavily.api_key_env;
    match resolve_from_secretspec(config, name) {
        Ok(Some(key)) => Ok(key),
        Ok(None) => Err(ServiceError::Message(format!(
            "{name} is set neither in the environment nor in SecretSpec. \
             Store it with `just secrets-set`, or export it before starting the daemon."
        ))),
        Err(SecretSpecError::NoManifest) => Err(ServiceError::Message(format!(
            "{name} is not in the environment, and no secretspec.toml was found. \
             Start from the repository root, set SECRETSPEC_FILE, or configure \
             `provider.tavily.secretspec_file`."
        ))),
        Err(err) => Err(ServiceError::Message(format!(
            "could not resolve {name} through SecretSpec: {err}. \
             Run `just secrets-check` to inspect the configured provider."
        ))),
    }
}

fn resolve_from_secretspec(config: &Config, name: &str) -> Result<Option<String>, SecretSpecError> {
    resolve_with_provider(config, name, None)
}

fn resolve_with_provider(
    config: &Config,
    name: &str,
    provider: Option<&str>,
) -> Result<Option<String>, SecretSpecError> {
    let manifest = config
        .provider
        .tavily
        .secretspec_file
        .clone()
        .or_else(|| std::env::var_os("SECRETSPEC_FILE").map(PathBuf::from));
    let mut secrets = match manifest {
        Some(path) => Secrets::load_from(&path)?,
        None => Secrets::load()?,
    };
    if let Some(provider) = provider {
        secrets.set_provider(provider);
    }
    let secrets = secrets.with_reason(REASON);

    match secrets.resolve_named(name)? {
        NamedResolution::Resolved(secret) => {
            if let Some(value) = secret.value {
                if !value.trim().is_empty() {
                    return Ok(Some(value));
                }
            }

            // A secret declared with `as_path` exposes a temporary file rather
            // than an inline value.
            if let Some(path) = secret.path {
                let value = fs::read_to_string(path)?;
                if !value.trim().is_empty() {
                    return Ok(Some(value));
                }
            }

            Ok(None)
        }
        NamedResolution::Missing { .. } | NamedResolution::Undeclared => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn environment_takes_precedence() {
        let name = format!("RESEARCH_TEST_KEY_{}", std::process::id());
        // SAFETY: the name is unique to this test, so no other test observes it.
        unsafe { std::env::set_var(&name, "from-environment") };

        let mut config = Config::default();
        config.provider.tavily.api_key_env = name.clone();

        assert_eq!(resolve_api_key(&config).unwrap(), "from-environment");
        unsafe { std::env::remove_var(&name) };
    }

    #[test]
    fn secretspec_manifest_defaults_are_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("secretspec.toml");
        let name = format!("RESEARCH_TEST_SECRET_{}", std::process::id());
        std::fs::write(
            &manifest,
            format!(
                r#"
[project]
name = "research-station-test"
revision = "1.0"

[profiles.default]
{name} = {{ default = "from-secretspec", description = "test key" }}
"#
            ),
        )
        .unwrap();

        let mut config = Config::default();
        config.provider.tavily.api_key_env = name.clone();
        config.provider.tavily.secretspec_file = Some(PathBuf::from(&manifest));

        assert_eq!(
            resolve_with_provider(&config, &name, Some("env")).unwrap(),
            Some("from-secretspec".to_owned())
        );
    }
}
