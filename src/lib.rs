extern crate blake2;

use blake2::{Blake2b, Digest};
use sha2::Sha512;
use std::collections::LinkedList;
use std::default::Default;

mod table;
#[cfg(not(feature = "window_size"))]
mod table_const;
#[cfg(feature = "window_size")]
mod table_gen;

use crate::table::*;

const DEGREE: usize = 64;
const ZIP_HEADER: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];

#[derive(Debug)]
pub struct ChunkerOptions {
    pub window_size: usize,
    pub min_chunk: usize,
    pub avg_chunk: usize,
    pub max_chunk: usize,
    pub detect_zip_boundary: bool,
}

impl Default for ChunkerOptions {
    fn default() -> Self {
        Self {
            window_size: DEFAULT_WINDOW_SIZE,
            min_chunk: 8 * 1024,
            avg_chunk: 16 * 1024,
            max_chunk: 32 * 1024,
            detect_zip_boundary: false,
        }
    }
}

#[derive(Debug)]
pub struct Chunk {
    pub size: usize,
    pub digest: Vec<u8>,
}

pub struct Stats {
    pub size: usize,
    pub sha512: Vec<u8>,
}

pub struct Chunker {
    table: Table,
    options: ChunkerOptions,
    hash: u64,
    hash_mask: u64,
    window: Vec<u8>,
    window_size: usize,
    window_offset: usize,
    chunk_size: usize,
    chunk_digest: Blake2b<blake2::digest::consts::U18>,
    digest: Sha512,
    total_size: usize,
    zip_header_offset: usize,
    chunks: LinkedList<Chunk>,
}

impl Chunker {
    pub fn new(options: ChunkerOptions) -> Self {
        let hash_mask = options.avg_chunk - 1;

        Self {
            table: Table::new(options.window_size),
            hash: 0,
            hash_mask: hash_mask as u64,
            window: vec![0; options.window_size],
            window_size: options.window_size,
            window_offset: 0,
            chunk_size: 0,
            chunk_digest: Blake2b::new(),
            digest: Sha512::new(),
            total_size: 0,
            zip_header_offset: 0,
            chunks: LinkedList::new(),

            options,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        let window_size = self.options.window_size;

        let mut chunk_start = 0;

        self.digest.update(data);
        self.total_size += data.len();

        for i in 0..data.len() {
            self.chunk_size += 1;

            if self.options.detect_zip_boundary && self.zip_header_offset < ZIP_HEADER.len() {
                let b = data[i];
                if ZIP_HEADER[self.zip_header_offset] == b {
                    self.zip_header_offset += 1;
                } else {
                    self.zip_header_offset = 0;
                }
            }

            let seen_zip_header = self.zip_header_offset == ZIP_HEADER.len();

            // Skip until we are `window_size`  bytes behind minimum chunk size
            if self.chunk_size + self.window_size <= self.options.min_chunk && !seen_zip_header {
                continue;
            }

            let b = data[i];
            let dropped_byte = self.window[self.window_offset] as usize;
            let shifted_byte = self.hash >> (DEGREE - 8 - 1);
            self.window[self.window_offset] = b;
            self.window_offset = (self.window_offset + 1) % window_size;

            self.hash <<= 8;
            self.hash ^= b as u64;
            self.hash ^= self.table.drop[dropped_byte];
            self.hash ^= self.table.shift[shifted_byte as usize];

            if !(seen_zip_header
                || (self.chunk_size >= self.options.min_chunk
                    && (self.hash & self.hash_mask) == self.hash_mask)
                || self.chunk_size >= self.options.max_chunk)
            {
                continue;
            }

            self.chunk_digest.update(&data[chunk_start..=i]);
            self.chunks.push_back(Chunk {
                size: self.chunk_size,
                digest: self.chunk_digest.finalize_reset().to_vec(),
            });
            chunk_start = i + 1;
            self.reset();
        }

        if chunk_start < data.len() {
            self.chunk_digest.update(&data[chunk_start..]);
        }
    }

    pub fn finalize_reset(&mut self) -> Stats {
        let total_size = self.total_size;
        let chunk_size = self.chunk_size;
        let digest = self.chunk_digest.finalize_reset();

        self.total_size = 0;
        self.reset();

        if chunk_size != 0 {
            self.chunks.push_back(Chunk {
                size: chunk_size,
                digest: digest.to_vec(),
            })
        }

        Stats {
            size: total_size,
            sha512: self.digest.finalize_reset().to_vec(),
        }
    }

    fn reset(&mut self) {
        self.hash = 0;
        self.chunk_size = 0;
        self.zip_header_offset = 0;
        self.window.fill(0);
    }
}

impl Iterator for Chunker {
    type Item = Chunk;

    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.pop_front()
    }
}

#[cfg(test)]
mod tests {
    extern crate base64;

    use super::*;

    #[test]
    #[cfg(feature = "window_size")]
    fn it_computes_rolling_hash() {
        let mut chunker = Chunker::new(ChunkerOptions {
            window_size: 16,
            min_chunk: 0,
            max_chunk: 1024 * 1024,

            // Make sure we never chunk for this test
            avg_chunk: 1024 * 1024,
            detect_zip_boundary: false,
        });

        for i in 0..1024u64 {
            chunker.update(&[(i & 0xff) as u8]);
        }
        let rolling_hash = chunker.hash;

        chunker.reset();
        for i in (1024 - 16)..1024u64 {
            chunker.update(&[(i & 0xff) as u8]);
        }
        let non_rolling_hash = chunker.hash;
        assert_eq!(rolling_hash, non_rolling_hash);

        assert_eq!(rolling_hash, 1976718474515856107);
    }

    #[test]
    fn it_computes_chunks() {
        let mut chunker = Chunker::new(ChunkerOptions::default());

        let size: usize = 256 * 1024;

        for _i in 0..size {
            chunker.update(&[0x33, 0x31, 0x85]);
        }

        let stats = chunker.finalize_reset();
        assert_eq!(stats.size, size * 3);

        assert_eq!(chunker.count(), 24);
    }

    #[test]
    fn it_doesnt_chunk_early_after_skipping() {
        let mut chunker = Chunker::new(ChunkerOptions::default());

        chunker.update(&[0xff; 8 * 1024 + 8]);

        let stats = chunker.finalize_reset();
        assert_eq!(stats.size, 8 * 1024 + 8);

        assert_eq!(chunker.count(), 1);
    }
}
