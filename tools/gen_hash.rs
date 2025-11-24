// Simple utility to generate bcrypt hash for a password
// Usage: cargo run --bin gen_hash

fn main() {
    let password = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "admin123".to_string());

    let hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
        .expect("Failed to hash password");

    println!("Password: {}", password);
    println!("Bcrypt hash: {}", hash);

    // Verify it works
    let verified = bcrypt::verify(&password, &hash)
        .expect("Failed to verify");
    println!("Verification: {}", verified);
}
