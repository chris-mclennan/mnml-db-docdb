//! Config file at `~/.config/mnml-db-docdb.toml`.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_row_limit")]
    pub row_limit: u32,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

fn default_row_limit() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub name: String,
    /// MongoDB / DocumentDB URI —
    /// `mongodb://user:pass@host:port/?authSource=admin&tls=true`.
    pub uri: String,
    /// Default database the query layer queries when the input
    /// doesn't specify `<db>.<coll>.find(...)` form.
    #[serde(default)]
    pub default_db: String,
}

impl Config {
    pub const EXAMPLE: &'static str = r##"# mnml-db-docdb config. Edit and re-run.
# chmod 600 the file — passwords live in the URI.

# Result-row cap per query. DocumentDB cursors can produce millions
# of docs; 100 is a sane default for a TUI. Doubled at runtime by R.
row_limit = 100

[[connections]]
name = "local"
uri = "mongodb://localhost:27017"
default_db = "test"

# [[connections]]
# name = "docdb-prod"
# uri = "mongodb://api:${DOCDB_PASS}@docdb-cluster.cluster-xyz.us-east-1.docdb.amazonaws.com:27017/?tls=true&tlsCAFile=/etc/ssl/global-bundle.pem&replicaSet=rs0&readPreference=secondaryPreferred&retryWrites=false"
# default_db = "api"

# [[connections]]
# name = "mongodb-atlas"
# uri = "mongodb+srv://app:${ATLAS_PASS}@cluster0.abc123.mongodb.net/"
# default_db = "production"
"##;

    pub fn validate(&self) -> Result<()> {
        if self.connections.is_empty() {
            return Err(anyhow!(
                "config: at least one [[connections]] entry required"
            ));
        }
        if self.row_limit == 0 {
            return Err(anyhow!("config: row_limit must be > 0"));
        }
        for (i, c) in self.connections.iter().enumerate() {
            if c.name.trim().is_empty() {
                return Err(anyhow!("connection #{i}: `name` is required"));
            }
            if c.uri.trim().is_empty() {
                return Err(anyhow!("connection #{i} ({}): `uri` is required", c.name));
            }
        }
        Ok(())
    }

    pub fn expand_env(&mut self) {
        for c in self.connections.iter_mut() {
            c.uri = expand_env(&c.uri);
        }
    }
}

fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let mut name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            match std::env::var(&name) {
                Ok(v) => out.push_str(&v),
                Err(_) => {
                    out.push_str("${");
                    out.push_str(&name);
                    out.push('}');
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("mnml-db-docdb.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, Config::EXAMPLE)?;
        return Err(anyhow!(
            "wrote config template to {} — edit it (chmod 600!) then re-run",
            path.display()
        ));
    }
    let text = std::fs::read_to_string(&path)?;
    let mut cfg: Config = toml::from_str(&text)?;
    cfg.validate()?;
    cfg.expand_env();
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses_and_validates() {
        let cfg: Config = toml::from_str(Config::EXAMPLE).unwrap();
        cfg.validate().unwrap();
        assert!(!cfg.connections.is_empty());
    }

    #[test]
    fn validate_rejects_empty_connections() {
        let cfg: Config = toml::from_str("row_limit = 100").unwrap();
        assert!(cfg.validate().is_err());
    }
}
