//! Runs the test double of the sync storage for manual checks.
//!
//! It serves one sync file with the ETag rules of Azure Blob Storage. Point
//! `sync.toml` of two temporary profiles and `HUSKER_SYNC_URL` of
//! `husker serve` at the printed URL, and nothing leaves this computer.
//!
//! ```text
//! cargo run --example fake_blob
//! ```

#[path = "../tests/support/mod.rs"]
mod support;

fn main() {
    let blob = support::FakeBlob::start();
    println!("Sync file URL: {}", blob.url());
    println!("The file exists after the first `husker sync`. Press Ctrl+C to stop.");
    loop {
        std::thread::park();
    }
}
