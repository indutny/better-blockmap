extern crate blake2;
extern crate rug;

use blake2::{Blake2b, Digest};
use rug::Integer;
use std::default::Default;

const POLYNOMIAL: u64 = 0xbfe6b8a5bf378d83;
static ZIP_HEADER: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];

fn reduce(value: Integer, modulo: &Integer) -> u64 {
    let modulo_bits = modulo.significant_bits();
    let value_bits = value.significant_bits();
    if value_bits < modulo_bits {
        return value.to_u64_wrapping();
    }

    let delta = value_bits - modulo_bits;

    let mut result = value;
    for i in (0..=delta).rev() {
        if result.get_bit(modulo_bits + i - 1) {
            result ^= modulo.clone() << i;
        }
    }

    result.to_u64_wrapping()
}

pub struct Table {
    shift: [u64; 256],
    drop: [u64; 256],
}

impl Table {
    pub fn new(window_size: usize) -> Self {
        let mut res = Self {
            shift: [0; 256],
            drop: [0; 256],
        };

        let modulo = Integer::from(POLYNOMIAL);
        let degree = modulo.significant_bits();

        for i in 0..256 {
            res.shift[i] =
                reduce(Integer::from(i) << (degree - 1), &modulo) ^ (i << (degree - 1)) as u64;
            res.drop[i] = reduce(Integer::from(i) << (window_size * 8), &modulo);
        }
        res
    }
}

#[derive(Debug)]
pub struct ChunkerOptions {
    pub window_size: usize,
    pub min_chunk: usize,
    pub avg_chunk: usize,
    pub max_chunk: usize,
    pub zip_boundary: bool,
}

impl Default for ChunkerOptions {
    fn default() -> Self {
        Self {
            window_size: 64,
            min_chunk: 8 * 1024,
            avg_chunk: 16 * 1024,
            max_chunk: 32 * 1024,
            zip_boundary: false,
        }
    }
}

#[derive(Debug)]
pub struct Chunk {
    pub size: usize,
    pub digest: Vec<u8>,
}

pub struct Chunker {
    table: Table,
    options: ChunkerOptions,
    hash: u64,
    hash_mask: u64,
    window: Vec<u8>,
    window_offset: usize,
    chunk_size: usize,
    degree: u32,
    digest: Blake2b<blake2::digest::consts::U18>,
    zip_header_offset: usize,
}

impl Chunker {
    pub fn new(options: ChunkerOptions) -> Self {
        let window = vec![0; options.window_size];
        assert!(Integer::from(options.avg_chunk).is_power_of_two());

        let hash_mask = options.avg_chunk - 1;

        let modulo = Integer::from(POLYNOMIAL);
        let degree = modulo.significant_bits();

        Self {
            table: Table::new(options.window_size),
            options,
            hash: 0,
            hash_mask: hash_mask as u64,
            window,
            window_offset: 0,
            chunk_size: 0,
            degree,
            digest: Blake2b::new(),
            zip_header_offset: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) -> Vec<Chunk> {
        let window_size = self.options.window_size;
        let mut result = Vec::new();

        let mut chunk_start = 0;

        for i in 0..data.len() {
            if self.options.zip_boundary && self.zip_header_offset < ZIP_HEADER.len() {
                let b = data[i];
                if ZIP_HEADER[self.zip_header_offset] == b {
                    self.zip_header_offset += 1;
                } else {
                    self.zip_header_offset = 0;
                }
            }

            let seen_zip_header = self.zip_header_offset == ZIP_HEADER.len();

            // Skip until we are 8 bytes behind minimum chunk size
            if self.chunk_size + 8 <= self.options.min_chunk && !seen_zip_header {
                self.chunk_size += 1;
                continue;
            }

            let b = data[i];
            let dropped_byte = self.window[self.window_offset] as usize;
            let shifted_byte = self.hash >> (self.degree - 8 - 1);
            self.window[self.window_offset] = b;
            self.window_offset = (self.window_offset + 1) % window_size;

            self.hash <<= 8;
            self.hash ^= b as u64;
            self.hash ^= self.table.drop[dropped_byte];
            self.hash ^= self.table.shift[shifted_byte as usize];

            self.chunk_size += 1;

            if !seen_zip_header
                && self.chunk_size < self.options.max_chunk
                && (self.hash & self.hash_mask) != self.hash_mask
            {
                continue;
            }

            self.digest.update(&data[chunk_start..=i]);
            result.push(Chunk {
                size: self.chunk_size,
                digest: self.digest.finalize_reset().to_vec(),
            });
            chunk_start = i + 1;
            self.reset();
        }

        if chunk_start < data.len() {
            self.digest.update(&data[chunk_start..])
        }

        result
    }

    pub fn flush(&mut self) -> Option<Chunk> {
        let chunk_size = self.chunk_size;
        let digest = self.digest.finalize_reset();

        self.reset();
        if chunk_size == 0 {
            None
        } else {
            Some(Chunk {
                size: chunk_size,
                digest: digest.to_vec(),
            })
        }
    }

    fn reset(&mut self) {
        self.hash = 0;
        self.chunk_size = 0;
        self.zip_header_offset = 0;
        self.window.fill(0);
    }
}

#[cfg(test)]
mod tests {
    extern crate base64;

    use super::*;

    fn assert_is_field_value(hash: u64) {
        let modulo = Integer::from(POLYNOMIAL);
        assert_eq!(hash, reduce(Integer::from(hash), &modulo));
    }

    #[test]
    fn it_computes_correct_table() {
        let table = Table::new(64);

        assert_eq!(table.shift[0], 0);
        assert_eq!(table.drop[0], 0);

        assert_eq!(table.shift[8], 4548086706303466141);
        assert_eq!(table.drop[8], 4180238687019168624);
    }

    #[test]
    fn it_computes_rolling_hash() {
        let mut chunker = Chunker::new(ChunkerOptions {
            window_size: 16,
            min_chunk: 0,
            max_chunk: 1024 * 1024,

            // Make sure we never chunk for this test
            avg_chunk: 1024 * 1024,
            zip_boundary: false,
        });

        for i in 0..1024u64 {
            chunker.update(&[(i & 0xff) as u8]);
        }
        assert_is_field_value(chunker.hash);
        let rolling_hash = chunker.hash;

        chunker.reset();
        for i in (1024 - 16)..1024u64 {
            chunker.update(&[(i & 0xff) as u8]);
        }
        assert_is_field_value(chunker.hash);
        let non_rolling_hash = chunker.hash;
        assert_eq!(rolling_hash, non_rolling_hash);

        assert_eq!(rolling_hash, 1976718474515856107);
    }

    #[test]
    fn it_computes_chunks() {
        let mut chunker = Chunker::new(ChunkerOptions::default());

        let size: u64 = 256 * 1024;

        let mut chunks = Vec::new();
        for _i in 0..size {
            chunks.append(&mut chunker.update(&[0x33, 0x31, 0x85]));
        }

        let last = chunker.flush();
        if let Some(x) = last {
            chunks.push(x);
        }

        assert_eq!(chunks.len(), 24);
    }
}
