//! Simple loadtest stub: prints how to drive bots; full bots need live server.
//!
//! Usage:
//!   loadtest --help

fn main() {
    println!("nova-horizon loadtest (PR-16)");
    println!("1. Start Postgres + game-server");
    println!("2. Use scripts/smoke_auth.ps1 for REST");
    println!("3. Run multiple Godot clients for concurrent flight");
    println!("4. Metrics: GET http://127.0.0.1:8080/metrics");
    println!();
    println!("Future: automated WS bots for move/trade/fight density.");
}
