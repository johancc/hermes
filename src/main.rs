mod providers;
use env_logger;
use providers::{google::GoogleFitProvider, Provider};
use std::path::Path;

fn validate_env() {
    let google_client_secret_path = std::env::var("GOOGLE_CLIENT_SECRET")
        .expect("Missing env variable: `GOOGLE_CLIENT_SECRET`");
    if !Path::new(&google_client_secret_path).exists() {
        panic!(
            "Invalid Google client secret path: {}",
            google_client_secret_path
        )
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    validate_env();
    let secret_path = std::env::var("GOOGLE_CLIENT_SECRET").unwrap();
    let gfit = GoogleFitProvider::new(Some(secret_path))
        .await
        .expect("Failed to initialize GoogleFitProvider");
    let steps = gfit.daily_steps().await;
    match steps {
        Ok(n) => println!("You have taken {:?} steps today!", n),
        Err(e) => panic!("Failed to retrieve steps :( {}", e),
    }
}
