use std::path::PathBuf;

pub fn find_assets_folder() -> PathBuf {
    let mut current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    println!("Current Working Directory: {:?}", current);

    // Search up to 5 levels parent directories
    for _ in 0..5 {
        let assets = current.join("assets");
        if assets.exists() && assets.is_dir() {
            return assets;
        }
        if !current.pop() {
            break;
        }
    }
    
    // Fallback if not found
    PathBuf::from("assets")
}
