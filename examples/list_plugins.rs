//! List all available AudioUnit plugins on the system

use rack::prelude::*;

fn main() -> Result<()> {
    println!("Scanning for AudioUnit plugins...\n");

    let scanner = Scanner::new();
    let plugins = scanner.scan()?;

    if plugins.is_empty() {
        println!("No plugins found.");
        return Ok(());
    }

    println!("Found {} plugin(s):\n", plugins.len());

    for (i, plugin) in plugins.iter().enumerate() {
        println!("{}. {}", i + 1, plugin);
        println!("   Path: {}", plugin.path.display());
        println!("   ID: {}", plugin.unique_id);
        println!();
    }

    Ok(())
}
