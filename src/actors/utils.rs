use std;
use std::sync::atomic::{AtomicUsize, Ordering};

use lazy_static::lazy_static;
use rand;

lazy_static! {
    static ref ID: AtomicUsize = { AtomicUsize::new(rand::random()) };
}

static U16_MAX: usize = std::u16::MAX as usize;

pub fn next_id() -> u16 {
    (ID.fetch_add(1, Ordering::SeqCst) % U16_MAX + 1) as u16
}
