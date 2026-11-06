use std::fs;
use std::path::{Path, PathBuf};

use research_core::Config;
use secretspec::{NamedResolution, SecretSpecError, Secrets};

use crate::ServiceError;

const REASON: &str = "research-daemon live provider calls";

/// Resolve the Tavily key from the environment first, then from SecretSpec.
///
/// SecretSpec uses the provider configured in `~/.config/secretspec/config.toml`
/// (the system keyring by default). The key is never written to a file or log.
pub fn resolve_api_key(config: &Config) -> Result<String, ServiceError> {
    if let Ok(key) = config.api_key() {
        return Ok(key);
    }

    let name = &config.provider.tavily.api_key_env;
    let manifest = config.provider.tavily.secretspec_file.as_deref();
    match resolve_from_secretspec(manifest, name) {
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

/// Resolve the model provider key from the environment, then SecretSpec.
///
/// The primary variable is `llm.api_key_env`; older names such as
/// `OPENCODE_API_KEY` are tried in order. The key is never logged.
pub fn resolve_llm_api_key(config: &Config) -> Result<String, ServiceError> {
    let names = config.llm.api_key_names();
    if names.is_empty() {
        return Err(ServiceError::Message(
            "llm.api_key_env is empty; set it to the variable that carries the model API key"
                .to_owned(),
        ));
    }

    for name in &names {
        if let Ok(value) = std::env::var(name) {
            if !value.trim().is_empty() {
                return Ok(value);
            }
        }
    }

    let tavily_manifest = config.provider.tavily.secretspec_file.as_deref();
    let manifest = config.llm.secretspec_file.as_deref().or(tavily_manifest);
    let mut no_manifest = false;
    for name in &names {
        match resolve_from_secretspec(manifest, name) {
            Ok(Some(key)) => return Ok(key),
            Ok(None) => {}
            Err(SecretSpecError::NoManifest) => no_manifest = true,
            Err(err) => {
                return Err(ServiceError::Message(format!(
                    "could not resolve {name} through SecretSpec: {err}. \
                     Run `just secrets-check` to inspect the configured provider."
                )));
            }
        }
    }

    let joined = names.join(" or ");
    if no_manifest {
        Err(ServiceError::Message(format!(
            "{joined} is not in the environment, and no secretspec.toml was found. \
             Start from the repository root, set SECRETSPEC_FILE, or configure \
             `llm.secretspec_file`."
        )))
    } else {
        Err(ServiceError::Message(format!(
            "{joined} is set neither in the environment nor in SecretSpec. \
             Store it with `just secrets-set-llm`, or export it before starting the daemon."
        )))
    }
}

fn resolve_from_secretspec(
    manifest: Option<&Path>,
    name: &str,
) -> Result<Option<String>, SecretSpecError> {
    resolve_with_manifest(manifest, name, None)
}

fn resolve_with_manifest(
    manifest: Option<&Path>,
    name: &str,
    provider: Option<&str>,
) -> Result<Option<String>, SecretSpecError> {
    let manifest = manifest
        .map(Path::to_path_buf)
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
    fn llm_environment_takes_precedence() {
        let name = format!("RESEARCH_TEST_LLM_KEY_{}", std::process::id());
        // SAFETY: the name is unique to this test, so no other test observes it.
        unsafe { std::env::set_var(&name, "llm-from-environment") };

        let mut config = Config::default();
        config.llm.api_key_env = name.clone();
        config.llm.fallback_api_key_envs.clear();

        assert_eq!(
            resolve_llm_api_key(&config).unwrap(),
            "llm-from-environment"
        );
        unsafe { std::env::remove_var(&name) };
    }

    #[test]
    fn llm_falls_back_to_an_alternative_environment_name() {
        let primary = format!("RESEARCH_TEST_LLM_PRIMARY_{}", std::process::id());
        let fallback = format!("RESEARCH_TEST_LLM_FALLBACK_{}", std::process::id());
        // SAFETY: the names are unique to this test.
        unsafe { std::env::remove_var(&primary) };
        unsafe { std::env::set_var(&fallback, "llm-from-fallback") };

        let mut config = Config::default();
        config.llm.api_key_env = primary;
        config.llm.fallback_api_key_envs = vec![fallback.clone()];

        assert_eq!(resolve_llm_api_key(&config).unwrap(), "llm-from-fallback");
        unsafe { std::env::remove_var(&fallback) };
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
            resolve_with_manifest(Some(&manifest), &name, Some("env")).unwrap(),
            Some("from-secretspec".to_owned())
        );
    }

    #[test]
    fn llm_secretspec_manifest_is_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("secretspec.toml");
        let name = format!("RESEARCH_TEST_LLM_SECRET_{}", std::process::id());
        std::fs::write(
            &manifest,
            format!(
                r#"
[project]
name = "research-station-test"
revision = "1.0"

[profiles.default]
{name} = {{ default = "llm-from-secretspec", description = "test key" }}
"#
            ),
        )
        .unwrap();

        let mut config = Config::default();
        config.llm.api_key_env = name.clone();
        config.llm.fallback_api_key_envs.clear();
        config.llm.secretspec_file = Some(PathBuf::from(&manifest));

        assert_eq!(resolve_llm_api_key(&config).unwrap(), "llm-from-secretspec");
    }
}
