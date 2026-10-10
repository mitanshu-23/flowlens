//! What the import found.

use crate::orders::{self, Order};

/// Print how many orders were imported and the day's total.
pub fn print(batch: &[Order]) {
    let total: i64 = batch.iter().map(orders::amount).sum();
    println!("{} orders, total {}", batch.len(), cents(total));
}

/// Write cents as a price: 1250 → "12.50".
fn cents(value: i64) -> String {
    format!("{}.{:02}", value / 100, value.abs() % 100)
}

/// Write the old CSV export that the accounting tool used to read.
pub fn legacy_export() {
    println!("legacy export is no longer produced");
}
