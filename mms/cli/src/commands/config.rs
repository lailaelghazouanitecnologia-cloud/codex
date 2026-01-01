use anyhow::Result;
use mms_config::ConfigLoader;
use tracing::info;

pub async fn show_config() -> Result<()> {
    info!("Showing configuration");

    let loader = ConfigLoader::from_default_home()?;
    let config = loader.load()?;

    println!("MMS Configuration");
    println!("=================");
    println!();
    println!("Home Directory: {}", config.mms_home.display());
    println!("Model: {}", config.model.as_deref().unwrap_or("default"));
    println!("Provider: {}", config.provider_id);
    println!("Approval Mode: {:?}", config.approval_mode);
    println!();
    println!("Enabled Features:");

    for feature in config.features.enabled_features() {
        println!("  - {:?}", feature);
    }

    if !config.providers.is_empty() {
        println!();
        println!("Configured Providers:");
        for (name, _provider) in &config.providers {
            println!("  - {}", name);
        }
    }

    // Sandbox configuration
    println!();
    println!("Sandbox Configuration:");
    println!("  Enabled: {}", config.sandbox.enabled);
    println!("  Network: {:?}", config.sandbox.network);
    println!("  Disk Read: {:?}", config.sandbox.disk_read);
    println!("  Disk Write: {:?}", config.sandbox.disk_write);

    if !config.sandbox.writable_paths.is_empty() {
        println!("  Writable Paths:");
        for path in &config.sandbox.writable_paths {
            println!("    - {}", path.display());
        }
    }

    if !config.sandbox.readable_paths.is_empty() {
        println!("  Readable Paths:");
        for path in &config.sandbox.readable_paths {
            println!("    - {}", path.display());
        }
    }

    if let Some(policy_file) = &config.sandbox.exec_policy_file {
        println!("  Exec Policy File: {}", policy_file.display());
    }

    // Tools configuration
    println!();
    println!("Tools Configuration:");
    println!("  Shell: {}", if config.tools.shell_enabled { "enabled" } else { "disabled" });
    println!("  File Read: {}", if config.tools.file_read_enabled { "enabled" } else { "disabled" });
    println!("  File Write: {}", if config.tools.file_write_enabled { "enabled" } else { "disabled" });
    println!("  Web Search: {}", if config.tools.web_search_enabled { "enabled" } else { "disabled" });

    Ok(())
}
