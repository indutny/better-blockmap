extern crate base64;
extern crate better_blockmap;
extern crate clap;
extern crate flate2;
extern crate serde;
extern crate serde_json;

use better_blockmap::*;

use clap::Parser;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use std::fs::File;
use std::io::prelude::*;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Input binary file
    #[clap(short, long)]
    input: String,

    /// Output blockmap file
    #[clap(short, long)]
    output: String,

    /// Use zip file boundaries for splitting chunks
    #[clap(short, long)]
    zip_boundary: bool,
}

#[derive(Serialize)]
struct BlockmapFile {
    name: String,
    offset: usize,
    checksums: Vec<String>,
    sizes: Vec<usize>,
}

#[derive(Serialize)]
struct Blockmap {
    version: String,
    files: Vec<BlockmapFile>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let mut chunker = Chunker::new(ChunkerOptions {
        zip_boundary: args.zip_boundary,

        ..ChunkerOptions::default()
    });

    let mut input = File::open(args.input)?;
    let mut buffer = [0; 16384];

    let mut chunks = Vec::new();
    loop {
        let bytes_read = input.read(&mut buffer).expect("Failed to read bytes");

        chunks.append(&mut chunker.update(&buffer[0..bytes_read]));
        if bytes_read != buffer.len() {
            break;
        }
    }
    if let Some(last) = chunker.flush() {
        chunks.push(last)
    }

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

    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(json.as_bytes())?;

    let mut output = File::create(args.output)?;
    output.write_all(&encoder.finish()?)?;

    Ok(())
}
