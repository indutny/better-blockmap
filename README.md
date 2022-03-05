# better-blockmap
[![Latest version](https://img.shields.io/crates/v/better-blockmap.svg)](https://crates.io/crates/better-blockmap)
![License](https://img.shields.io/crates/l/better-blockmap.svg)

Blockmap file generator for electron-builder.

## Installation

```sh
cargo install better-blockmap
```

## Running

```sh
$ better-blockmap --help
better-blockmap 0.1.0
Fedor Indutny <fedor@indutny.com>
Generate better blockmap files for electron-builder

USAGE:
    better-blockmap [OPTIONS] --input <INPUT>

OPTIONS:
    -c, --compression <COMPRESSION>    Compression [default: gzip] [possible values: gzip, deflate]
    -h, --help                         Print help information
    -i, --input <INPUT>                Input binary file
    -o, --output <OUTPUT>              Output blockmap file
    -V, --version                      Print version information
    -z, --detect-zip-boundary          Use zip file boundaries for splitting chunks
```
