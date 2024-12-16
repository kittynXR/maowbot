use maowbot::Database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Current Working Directory: {:?}", std::env::current_dir()?);

    let data_dir = std::path::Path::new("data");
    println!("Data Directory Exists: {}", data_dir.exists());
    println!(
        "Data Directory Contents: {:?}",
        std::fs::read_dir(data_dir).map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>()
        })
    );

    let db = Database::new("data/bot.db").await?;
    db.migrate().await?;
    println!("Database initialized and migrated successfully!");

    Ok(())
}
