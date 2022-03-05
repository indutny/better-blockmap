use better_blockmap::*;

use byteorder::{LittleEndian, WriteBytesExt};
use clap::Parser;
use flate2::write::{DeflateEncoder, GzEncoder};
use flate2::Compression;
use serde::Serialize;
use std::default::Default;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;

#[derive(clap::ArgEnum, PartialEq, Debug, Clone)]
enum CompressionType {
    Gzip,
    Deflate,
}

impl Default for CompressionType {
    fn default() -> Self {
        CompressionType::Gzip
    }
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Input binary file
    #[clap(short, long)]
    input: String,

    /// Output blockmap file
    #[clap(short, long)]
    output: Option<String>,

    /// Compression
    #[clap(short, long, arg_enum, default_value_t)]
    compression: CompressionType,

    /// Use zip file boundaries for splitting chunks
    #[clap(short = 'z', long)]
    detect_zip_boundary: bool,
}

#[derive(Serialize)]
struct BlockmapFile {
    name: String,
    offset: usize,
    checksums: Vec<String>,
    sizes: Vec<usize>,
}

#[derive(Serialize)]
struct JSONStats {
    size: usize,
    sha512: String,
}

#[derive(Serialize)]
struct Blockmap {
    version: String,
    files: Vec<BlockmapFile>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let mut chunker = Chunker::new(ChunkerOptions {
        detect_zip_boundary: args.detect_zip_boundary,

        ..ChunkerOptions::default()
    });

    let mut input = File::open(&args.input)?;
    let mut buffer = [0; 16384];

    loop {
        let bytes_read = input.read(&mut buffer).expect("Failed to read bytes");

        chunker.update(&buffer[0..bytes_read]);
        if bytes_read != buffer.len() {
            break;
        }
    }

    let stats = chunker.finalize_reset();
    let chunks: Vec<Chunk> = chunker.collect();

    let blockmap = Blockmap {
        version: "2".to_string(),
        files: vec![BlockmapFile {
            name: "file".to_string(),
            offset: 0,
            checksums: chunks
                .iter()
                .map(|chunk| base64::encode(&chunk.digest))
                .collect(),
            sizes: chunks.iter().map(|chunk| chunk.size).collect(),
        }],
    };

    let json = serde_json::to_string(&blockmap).expect("JSON serialization");

    let compressed = match args.compression {
        CompressionType::Gzip => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(json.as_bytes())?;
            encoder.finish()?
        }
        CompressionType::Deflate => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(json.as_bytes())?;
            encoder.finish()?
        }
    };

    let mut output = match &args.output {
        // Create new file
        Some(path) => File::create(path)?,
        // Append to input
        None => OpenOptions::new().append(true).open(&args.input)?,
    };
    output.write_all(&compressed)?;
    if args.output.is_none() {
        let mut size = vec![];
        size.write_u32::<LittleEndian>(compressed.len() as u32)?;
        output.write_all(&size)?;
    }

    println!(
        "{}",
        serde_json::to_string(&JSONStats {
            size: stats.size,
            sha512: base64::encode(&stats.sha512),
        })
        .expect("JSON serialization")
    );

    Ok(())
}
