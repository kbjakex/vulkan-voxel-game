
use std::{path::Path, fs::File};

use anyhow::Result;
use lz4::block::CompressionMode;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("Usage `./compress <filepath> [output path]");
        return Ok(());
    }
    
    let path = Path::new(&args[0]);
    if !path.exists() {
        println!("Path doesn't exist");
        return Ok(());
    }
    if !path.is_file() {
        println!("Not a file");
        return Ok(());
    }

    let file = std::fs::read(path)?;
    let compressed = lz4::block::compress(&file, Some(CompressionMode::HIGHCOMPRESSION(12)), true)?;
    println!("Original size: {}", file.len());
    println!("Compressed size: {}", compressed.len());
    println!("Compression ratio: {:3}", file.len() as f32 / compressed.len() as f32);

    if args.len() >= 2 {
        let out_path = Path::new(&args[1]);
        if out_path.is_dir() {
            println!("Output path is a directory!");
            return Ok(());
        }

        std::fs::write(out_path, &compressed)?;
        println!("Wrote to {}", out_path.to_str().unwrap());
    }

    Ok(())
}
