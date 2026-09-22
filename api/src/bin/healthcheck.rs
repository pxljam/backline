//! Container probe: exit 0 if the API answers.
fn main() {
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let port = addr.rsplit(':').next().unwrap_or("8080");
    let url = format!("http://127.0.0.1:{port}/health");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let ok =
        rt.block_on(async { matches!(reqwest::get(&url).await, Ok(r) if r.status().is_success()) });
    std::process::exit(if ok { 0 } else { 1 });
}
