set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default:
  just --list

fmt:
    cd research-daemon && cargo fmt --all

fmt-check:
    cd research-daemon && cargo fmt --all -- --check

clippy:
    cd research-daemon && cargo clippy --workspace --all-targets -- -D warnings

test:
    cd research-daemon && cargo test --workspace

check: fmt-check clippy test

nix-check:
    nix flake check

secretspec_reason := "research-daemon live provider calls"

# Configure a provider once with `secretspec config global init`, then
# `just secrets-set`; `just secrets-check` reports what is missing. The reason
# is recorded by providers that keep an audit log.
provider *args='':
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- provider {{args}}

# Store the required Tavily key in the configured secretspec provider.
secrets-set:
    secretspec set TAVILY_API_KEY

# Store the optional OpenCode Go key used by the daemon's model client.
secrets-set-llm:
    secretspec set OPENCODE_GO_API_KEY

# Store the optional Redditapis key that enables Reddit search.
secrets-set-reddit:
    secretspec set REDDIT_API_KEY

# Report which provider credentials are missing from the configured provider.
secrets-check:
    secretspec check --reason {{quote(secretspec_reason)}}

# Check configuration, credentials, storage, and optionally the model.
doctor *args='':
    cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- doctor {{args}}

# Search through the provider, with credentials from secretspec.
search query *args='':
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- provider search "{{query}}" {{args}}

# Extract URLs through the provider, with credentials from secretspec.
extract *urls:
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- provider extract {{urls}}

# Search Reddit through Redditapis, with credentials from secretspec.
reddit-search query *args='':
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- provider reddit-search "{{query}}" {{args}}

# Fetch one Reddit thread and its comments.
reddit-thread id_or_url *args='':
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- provider reddit-thread "{{id_or_url}}" {{args}}

# Verify the Reddit key with one live read.
reddit-test:
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- provider reddit-test

# Run a research question end to end with Tavily and optional Reddit.
research query *args='':
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- run "{{query}}" {{args}}

# Run a question end to end with the deterministic fake provider and engine.
demo query *args='':
    cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- run "{{query}}" --depth quick --fake --engine deterministic {{args}}

# Run the agent loop with the fake provider and fake model, without credentials.
agent-demo query *args='':
    cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- run "{{query}}" --depth quick --fake --fake-llm {{args}}

# Print a job's agent transcript.
transcript id *args='':
    cargo run -q --manifest-path research-daemon/Cargo.toml -p research-cli -- job transcript {{id}} {{args}}

# Process queued research jobs. Pass --once to drain the queue and exit.
worker *args='':
    secretspec run --reason {{quote(secretspec_reason)}} -- cargo run -q --manifest-path research-daemon/Cargo.toml -p research-daemon -- {{args}}

# Process queued jobs with the deterministic fake provider and fake model.
worker-fake *args='':
    cargo run -q --manifest-path research-daemon/Cargo.toml -p research-daemon -- --fake --fake-llm {{args}}

# Run the CLI with raw arguments, without resolving secrets.
run *args:
    cargo run --manifest-path research-daemon/Cargo.toml -p research-cli -- {{args}}
