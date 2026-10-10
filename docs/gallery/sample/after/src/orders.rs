//! One order line, and how to read it.

/// The kinds of order the shop handles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// A normal sale.
    Sale,
    /// Money back for an earlier sale.
    Refund,
    /// One item swapped for another.
    Exchange,
}

/// A single line of the orders file.
#[derive(Debug, Clone)]
pub struct Order {
    /// The order's id, as written in the file.
    pub id: String,
    /// What kind of order it is.
    pub kind: Kind,
    /// How many items.
    pub quantity: u32,
    /// Price of one item, in cents.
    pub price: u64,
}

/// Read one comma-separated line: id, kind, quantity, price.
pub fn parse(line: &str) -> Result<Order, String> {
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return Err(format!("expected 4 fields, found {}", parts.len()));
    }
    let kind = match parts[1] {
        "sale" => Kind::Sale,
        "refund" => Kind::Refund,
        "exchange" => Kind::Exchange,
        other => return Err(format!("unknown kind {other}")),
    };
    let quantity = parts[2].parse().map_err(|_| "quantity is not a number".to_string())?;
    let price = parts[3].parse().map_err(|_| "price is not a number".to_string())?;
    Ok(Order { id: parts[0].to_string(), kind, quantity, price })
}

/// The amount an order adds to the day's total, in cents; refunds count against it.
pub fn amount(order: &Order) -> i64 {
    let gross = order.price as i64 * order.quantity as i64;
    match order.kind {
        Kind::Refund => -gross,
        Kind::Sale | Kind::Exchange => gross,
    }
}

/// The most one item may cost, in cents; anything above is taken as a typo.
pub const MAX_PRICE: u64 = 1_000_000;

/// Check an order's price is believable: not zero for a sale, and not above `MAX_PRICE`.
pub fn check_price(order: &Order) -> Result<(), String> {
    if order.kind == Kind::Sale && order.price == 0 {
        return Err("a sale cannot be free".to_string());
    }
    if order.price > MAX_PRICE {
        return Err(format!("price {} is above the limit", order.price));
    }
    Ok(())
}
