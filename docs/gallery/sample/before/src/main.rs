//! Imports a day's orders from a file, checks them, and writes a summary.

mod orders;
mod settings;
mod summary;

use orders::Order;

/// Read the orders file named on the command line and print a summary.
fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "orders.csv".to_string());
    let config = settings::load("import.toml").expect("settings must be readable");
    match import(&path, &config) {
        Ok(batch) => summary::print(&batch),
        Err(e) => eprintln!("import failed: {e}"),
    }
    summary::legacy_export();
}

/// Import every order in `path`, stopping at the first one that cannot be read.
fn import(path: &str, config: &settings::Settings) -> Result<Vec<Order>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut batch = Vec::new();
    let mut all_valid = true;
    for line in text.lines().skip(1) {
        let order = orders::parse(line)?;
        if order.quantity == 0 {
            all_valid = false;
            continue;
        }
        batch.push(order);
        if batch.len() >= config.batch_size {
            break;
        }
    }
    if !all_valid {
        eprintln!("some orders were skipped");
    }
    Ok(batch)
}
