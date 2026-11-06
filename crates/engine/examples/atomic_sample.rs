fn main() -> Result<(), Box<dyn std::error::Error>> {
    let package = materialcraft_engine::atomic_sample()?;
    let bytes = materialcraft_engine::formats::sbs::write_sbs(&package)?;
    let path = std::env::args().nth(1).unwrap_or_else(|| "atomic_sample.sbs".into());
    std::fs::write(path, bytes)?;
    Ok(())
}
