use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use libarc::{connect, ArcDaemonProxy};
use libarc::{Package, Transaction, TransactionStatus};
use serde::Deserialize;
use tokio::time::{sleep, Duration};

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

async fn wait_for_transaction(proxy: &ArcDaemonProxy<'_>, tx_id: &str) -> Result<()> {
    loop {
        let json = proxy.get_transaction(tx_id).await?;
        if json == "null" {
            println!("{}", "Transaction not found".yellow());
            break;
        }

        let tx: Transaction = serde_json::from_str(&json)?;
        match &tx.status {
            TransactionStatus::Pending => {
                print!("\r{}", "Pending...".dimmed());
            }
            TransactionStatus::Running => {
                print!("\r{} {}%", "Progress:".cyan(), tx.progress);
            }
            TransactionStatus::Success => {
                println!("\r{}", "Done!".green());
                break;
            }
            TransactionStatus::Failed(msg) => {
                println!("\r{}: {}", "Failed".red(), msg);
                break;
            }
        }

        sleep(Duration::from_millis(500)).await;
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
