use std::collections::HashMap;
use serde_json::Value as Json;
use crate::db::{connect, Pool};

pub struct Repo { pool: Pool }
pub struct Cache { map: HashMap<String, Json> }

trait Store { fn get(&self, k: &str) -> Option<String>; }

impl Repo {
    pub fn new() -> Self { Repo { pool: connect() } }
    pub fn load(&self, id: u32) -> Option<String> {
        let raw = self.fetch(id)?;
        Some(normalize(&raw))
    }
    fn fetch(&self, id: u32) -> Option<String> { self.pool.query(id) }
}

impl Store for Repo {
    fn get(&self, k: &str) -> Option<String> { self.load(k.parse().ok()?) }
}

impl Cache {
    fn get(&self, k: &str) -> Option<String> { self.map.get(k).map(|v| v.to_string()) }
}

fn normalize(s: &str) -> String { s.trim().to_lowercase() }

fn countdown(n: u32) -> u32 { if n == 0 { 0 } else { countdown(n - 1) } }

fn never_used() -> bool { true }

mod util {
    pub fn helper() -> u8 { 7 }
}

fn main() {
    let r = Repo::new();
    r.load(1);
    countdown(3);
    util::helper();
    let c = Cache { map: HashMap::new() };
    c.get("x");
}
