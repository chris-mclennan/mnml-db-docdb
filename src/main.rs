mod app;
mod config;
mod docdb;
mod keys;
mod theme;
mod ui;
mod install;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "mnml-db-docdb",
    version,
    about = "Amazon DocumentDB / MongoDB viewer for mnml"
)]
struct Cli {
    #[arg(long)]
    check: bool,
    /// Register this sibling with mnml — writes an integration
    /// manifest at ~/.config/mnml/integrations/<id>.toml so the
    /// rail chip + palette command + chord appear on the next
    /// mnml startup (or after `integrations.refresh`).
    #[arg(long)]
    install: bool,
    /// Remove the mnml integration manifest for this sibling.
    #[arg(long)]
    uninstall: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    // --install / --uninstall run before auth / config so the
    // first-run install doesn't require credentials to be set up.
    if cli.install {
        return install::install();
    }
    if cli.uninstall {
        return install::uninstall();
    }

    let cfg = config::load()?;
    if cli.check {
        println!("config: {}", config::config_path().display());
        println!("row_limit: {}", cfg.row_limit);
        for (i, c) in cfg.connections.iter().enumerate() {
            println!(
                "  connection {} ({}): {}  default_db={}",
                i + 1,
                c.name,
                scrub_uri(&c.uri),
                c.default_db
            );
        }
        return Ok(());
    }
    let mut app = app::App::new(cfg).await?;
    ui::run(&mut app).await
}

fn scrub_uri(uri: &str) -> String {
    let Some(scheme_end) = uri.find("://") else {
        return uri.to_string();
    };
    let rest = &uri[scheme_end + 3..];
    let Some(at) = rest.find('@') else {
        return uri.to_string();
    };
    let userinfo = &rest[..at];
    let Some(colon) = userinfo.find(':') else {
        return uri.to_string();
    };
    let user = &userinfo[..colon];
    let prefix = &uri[..scheme_end + 3];
    let suffix = &rest[at..];
    format!("{prefix}{user}:****{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_uri_hides_password() {
        let s = scrub_uri("mongodb://api:hunter2@docdb-cluster.example.com:27017");
        assert_eq!(s, "mongodb://api:****@docdb-cluster.example.com:27017");
    }

    #[test]
    fn scrub_uri_no_pass_idempotent() {
        let s = scrub_uri("mongodb://localhost:27017");
        assert_eq!(s, "mongodb://localhost:27017");
    }
}
