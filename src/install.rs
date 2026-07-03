//! `--install` / `--uninstall` subcommand — writes an integration
//! manifest at `~/.config/mnml/integrations/docdb.toml` so mnml
//! picks up the rail chip + palette command + chord binding on
//! next startup.

use anyhow::Result;
use mnml_bridge::{
    ChipSpec, CommandSpec, IntegrationSpec, install_integration, uninstall_integration,
};

const INTEGRATION_ID: &str = "docdb";

pub fn install() -> Result<()> {
    let spec = IntegrationSpec {
        id: INTEGRATION_ID.into(),
        name: "DocumentDB".into(),
        description: Some("Amazon DocumentDB / MongoDB — find + document view".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        binary: "mnml-db-docdb".into(),
        category: Some("db".into()),
        chip: Some(ChipSpec {
            glyph: "\u{F01BC}".into(),
            fallback: "Dd".into(),
            color: "teal".into(),
            tooltip: Some("Amazon DocumentDB / MongoDB — find + document view".into()),
            enabled: true,
            in_palette_bar: false,
            badge_key: Some(INTEGRATION_ID.into()),
        }),
        commands: vec![CommandSpec {
            id: "docdb.open".into(),
            title: "DocumentDB: open".into(),
            group: Some("integrations".into()),
            keys: vec!["<leader>id".into()],
            run: ":term mnml-db-docdb".into(),
        }],
        ..Default::default()
    };
    let path = install_integration(&spec)?;
    println!("wrote manifest: {}", path.display());
    println!("run mnml + `integrations.refresh` (or restart) to pick up the rail chip");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let removed = uninstall_integration(INTEGRATION_ID)?;
    if removed {
        println!("removed manifest for {INTEGRATION_ID}");
    } else {
        println!("no manifest for {INTEGRATION_ID} (already uninstalled)");
    }
    Ok(())
}
