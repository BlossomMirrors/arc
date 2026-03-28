use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use futures_util::StreamExt;
use libarc::{connect, ArcDaemonProxy};
use libarc::Package;
use serde::Deserialize;

// the daemon returns json strings over dbus, this catches the case where
// it returned an error object instead of the actual list
#[derive(Deserialize)]
struct DaemonError {
    error: String,
}

fn parse_packages(json: &str) -> Result<Vec<Package>> {
    if let Ok(e) = serde_json::from_str::<DaemonError>(json) {
        anyhow::bail!("Daemon error: {}", e.error);
    }
    Ok(serde_json::from_str(json).unwrap_or_default())
}

#[derive(Parser)]
#[command(name = "arc", about = "Arc Software Manager", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Install { app_id: String },
    Remove { app_id: String },
    Update,
    Search { query: String },
    List,
    RefreshCache,
}

// instead of polling are we done yet every 500ms we subscribe to the daemon's
// signals and block until we get the finished one
async fn wait_for_transaction(proxy: &ArcDaemonProxy<'_>, tx_id: &str) -> Result<()> {
    let mut progress_stream = proxy.receive_transaction_progress().await?;
    let mut finished_stream = proxy.receive_transaction_finished().await?;

    loop {
        tokio::select! {
            sig = progress_stream.next() => {
                if let Some(sig) = sig {
                    if let Ok(args) = sig.args() {
                        if *args.transaction_id() == tx_id {
                            print!("\r{} {}%", "Progress:".cyan(), args.progress());
                            let _ = std::io::Write::flush(&mut std::io::stdout());
                        }
                    }
                }
            }
            sig = finished_stream.next() => {
                match sig {
                    Some(sig) => {
                        if let Ok(args) = sig.args() {
                            if *args.transaction_id() == tx_id {
                                if *args.success() {
                                    println!("\r{}", "Done!".green());
                                } else {
                                    println!("\r{}: {}", "Failed".red(), args.message());
                                }
                                break;
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let proxy = connect().await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to connect to Arc daemon. Is arc-daemon running?\nError: {}",
            e
        )
    })?;

    match cli.command {
        Commands::Install { app_id } => {
            println!("{} {}", "Installing".cyan(), app_id.bold());
            let tx_id = proxy.install_package(&app_id).await?;
            println!("Transaction ID: {}", tx_id.dimmed());
            wait_for_transaction(&proxy, &tx_id).await?;
        }

        Commands::Remove { app_id } => {
            println!("{} {}", "Removing".yellow(), app_id.bold());
            let tx_id = proxy.remove_package(&app_id).await?;
            println!("Transaction ID: {}", tx_id.dimmed());
            wait_for_transaction(&proxy, &tx_id).await?;
        }

        Commands::Update => {
            println!("{}", "Checking for updates...".cyan());
            let json = proxy.list_updates().await?;
            let packages = parse_packages(&json)?;

            if packages.is_empty() {
                println!("{}", "No updates available.".green());
                return Ok(());
            }

            println!("{} update(s) available:", packages.len());
            for pkg in &packages {
                println!("  {} ({})", pkg.name.bold(), pkg.version.dimmed());
            }

            for pkg in &packages {
                println!("\n{} {}", "Updating".cyan(), pkg.id.bold());
                let tx_id = proxy.update_package(&pkg.id).await?;
                wait_for_transaction(&proxy, &tx_id).await?;
            }
        }

        Commands::Search { query } => {
            println!("{} '{}'", "Searching for".cyan(), query.bold());
            let json = proxy.search(&query).await?;
            let packages = parse_packages(&json)?;

            if packages.is_empty() {
                println!("{}", "No results found.".yellow());
                return Ok(());
            }

            println!(
                "{:<50} {:<30} {:<15}",
                "Application ID".bold(),
                "Name".bold(),
                "Version".bold()
            );
            println!("{}", "-".repeat(95).dimmed());
            for pkg in packages {
                println!(
                    "{:<50} {:<30} {:<15}",
                    pkg.id.cyan(),
                    pkg.name,
                    pkg.version.dimmed()
                );
                if !pkg.description.is_empty() {
                    println!("  {}", pkg.description.dimmed());
                }
            }
        }

        Commands::RefreshCache => {
            println!("{}", "Refreshing package cache...".cyan());
            let ok = proxy.refresh_cache().await?;
            if ok {
                println!("{}", "Cache refreshed.".green());
            } else {
                println!("{}", "Cache refresh failed.".red());
            }
        }

        Commands::List => {
            println!("{}", "Installed applications:".cyan());
            let json = proxy.list_installed().await?;
            let packages = parse_packages(&json)?;

            if packages.is_empty() {
                println!("{}", "No applications installed.".yellow());
                return Ok(());
            }

            println!(
                "{:<50} {:<30} {:<15}",
                "Application ID".bold(),
                "Name".bold(),
                "Version".bold()
            );
            println!("{}", "-".repeat(95).dimmed());
            for pkg in packages {
                println!(
                    "{:<50} {:<30} {:<15}",
                    pkg.id.cyan(),
                    pkg.name,
                    pkg.version.dimmed()
                );
            }
        }
    }

    Ok(())
}
