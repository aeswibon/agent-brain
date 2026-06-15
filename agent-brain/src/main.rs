use std::sync::Arc;

use agent_brain::{config::Config, engine::Engine, install, mcp, packages};
use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("agent_brain=info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("serve");

    match cmd {
        "serve" => {
            let config = Config::load()?;
            let engine = Arc::new(Engine::new(config)?);
            if engine.config.bootstrap_background {
                tracing::info!(target: "agent_brain::bootstrap", "starting MCP; bootstrap runs in background");
                engine.spawn_bootstrap(None);
            } else {
                let n = engine.bootstrap(None)?;
                tracing::info!("indexed {n} items");
            }
            mcp::run_stdio(engine).await?;
        }
        "index" => {
            let mut config = Config::load()?;
            config.bootstrap_background = false;
            config.session_ingest_background = false;
            let engine = Arc::new(Engine::new(config)?);
            let n = engine.bootstrap(None)?;
            println!("Indexed {n} items");
        }
        "add" => {
            let source = args.get(2).context("missing package source (owner/repo or GitHub URL)")?;
            let git_ref = flag_value(&args, "--ref");
            let skip_index = args.iter().any(|a| a == "--no-index");
            let config = Config::load()?;
            let record = packages::add_package(&config, source, git_ref.as_deref())?;
            println!(
                "Installed package '{}' from {} ({})",
                record.name,
                record.source,
                record.commit.unwrap_or_else(|| "unknown".into())
            );
            if !skip_index {
                let mut config = config;
                config.bootstrap_background = false;
                config.session_ingest_background = false;
                let engine = Arc::new(Engine::new(config)?);
                let n = engine.bootstrap(None)?;
                println!("Indexed {n} items");
            }
        }
        "package" => {
            let sub = args.get(2).map(String::as_str).unwrap_or("list");
            let config = Config::load()?;
            match sub {
                "list" => {
                    for pkg in packages::list_packages(&config)? {
                        println!(
                            "{}  {}  ref={}  commit={}  path={}",
                            pkg.name,
                            pkg.source,
                            pkg.git_ref,
                            pkg.commit.unwrap_or_else(|| "-".into()),
                            pkg.install_path
                        );
                    }
                }
                "remove" => {
                    let name = args
                        .get(3)
                        .context("usage: agent-brain package remove <name>")?;
                    let purged = packages::remove_package(&config, name)?;
                    println!("Removed package '{name}' (purged {purged} indexed items)");
                }
                "update" => {
                    let name = args.get(3).map(String::as_str);
                    let updated = packages::update_packages(&config, name)?;
                    for pkg in updated {
                        println!("Updated {} ({})", pkg.name, pkg.commit.unwrap_or_default());
                    }
                    let mut config = config;
                    config.bootstrap_background = false;
                    config.session_ingest_background = false;
                    let engine = Arc::new(Engine::new(config)?);
                    let n = engine.bootstrap(None)?;
                    println!("Indexed {n} items");
                }
                _ => {
                    eprintln!("Unknown package subcommand: {sub}");
                    print_usage();
                    std::process::exit(1);
                }
            }
        }
        "install" => {
            let global = args.iter().any(|a| a == "--global");
            let print_only = args.iter().any(|a| a == "--print-only");
            install::run(global, print_only)?;
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Unknown command: {cmd}");
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|idx| args.get(idx + 1))
        .cloned()
}

fn print_usage() {
    eprintln!(
        r#"agent-brain — local MCP router for agents, skills, rules, and memory

Usage:
  agent-brain serve                         Start MCP server (stdio)
  agent-brain index                         Reindex local agents/skills/rules/memory
  agent-brain add <owner/repo|url>          Install a GitHub package and index it
  agent-brain add affaan-m/ecc --ref main   Install with explicit git ref
  agent-brain package list                  List installed packages
  agent-brain package update [name]         Update one or all packages
  agent-brain package remove <name>         Remove an installed package
  agent-brain install [--global]              Write Cursor MCP config for this binary

Examples:
  agent-brain add https://github.com/affaan-m/ecc
  agent-brain add affaan-m/ecc
  agent-brain package update ecc

Install on another machine:
  curl -fsSL https://raw.githubusercontent.com/aeswibon/agent-brain/master/scripts/install.sh | bash -s -- --global
  agent-brain add affaan-m/ecc

Cursor starts MCP automatically — you do not run 'serve' manually.
See docs/USAGE.md for the full guide.
"#
    );
}
