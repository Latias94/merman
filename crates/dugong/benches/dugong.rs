use criterion::{criterion_group, criterion_main};

#[path = "dugong/layout.rs"]
mod layout;
#[path = "dugong/rank.rs"]
mod rank;

criterion_group!(benches, layout::register, rank::register);
criterion_main!(benches);
