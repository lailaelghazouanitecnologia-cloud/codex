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

    Ok(())
}
