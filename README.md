# ⚡genedex: A Small and Fast FM-Index for Rust⚡

[![Build Status](https://img.shields.io/github/actions/workflow/status/feldroop/genedex/rust.yml?style=flat-square&logo=github&label=CI)](https://github.com/feldroop/genedex/actions)
[![Crates.io](https://img.shields.io/crates/v/genedex.svg?style=flat-square&logo=rust)](https://crates.io/crates/genedex)
[![Documentation](https://img.shields.io/docsrs/genedex?style=flat-square&logo=rust)](https://docs.rs/genedex)

The [FM-Index] is a full-text index data structure that allows efficiently counting and retrieving all occurrenes of short sequences in very large texts. It is widely used in sequence analysis and bioinformatics.

The implementation of this library is based on an encoding for the text with rank support data structure (a.k.a. occurrence table)
by Simon Gene Gottlieb (publication pending). This encoding attemps to provide a good trade-off between
memory usage and running time of queries. Further benefits of `genedex` include:

- Fast, parallel index construction by leveraging the [`libsais-rs`] crate
- Support for indexing a set of texts, like chromosomes of a genomes
- Flexible cursor API
- Thoroughly tested using [`proptest`]

⚠️ **Warning:** this library is in an early stage. The API is still subject to changes. Any kind of feedback and suggestions via the issue tracker is highly appreciated! ⚠️

## Usage

For detailed information about how to use `genedex`, please refer to the [documentation]. The following is an example of the most basic functionality:

```rust
use genedex::{FmIndexConfig, alphabet};

let dna_n_alphabet = alphabet::ascii_dna_with_n();
let texts = [b"aACGT", b"acGtn"];

let index = FmIndexConfig::<i32>::new().construct_index(texts, dna_n_alphabet);

let query = b"GT";
assert_eq!(index.count(query), 2);

for hit in index.locate(query) {
    println!(
        "Found query in text {} at position {}.",
        hit.text_id, hit.position
    );
}
``` 

## Comparison to Other Crates and Benchmarks

Work in progress. Can be found [here](https://github.com/feldroop/rust-fmindex-benchmark)

## Possible Future Extensions and Improvements (roughly in order of priority):

- improved build memory usage: 
    - configurable, slower low memory mode
    - u32 saca (maybe sais-drum)
    - BWT view optimization
    - suffix array, lookup table compression using unconventional int widths (e.g. 33 bit)
    - maybe compress compress text and/or bwt at some point during construction 
- more flexible alphabet API
    - allow alphabet with sentinel inclded in io representation
    - allow alphabet without sentinel (only usable for single text indexing)
- optimized version for single text without sentinel
- optimized construction directly from (fasta) file reader
- space optimization for rarely occurring symbols (such as the sentinel and N in the human Genome)
    - maybe leverage the fact that such characters often occur in runs
- gate rayon/OpenMP usage behind feature flag
- bidirectional FM-Index
- paired blocks for improved memory usage when using larger alphabets
- optional functionality for text recovery
- text sampled suffix array (optionally with text ids and other annotations),
- FMD-Index
- optimizations for highly repetitive texts such as run length encoding
- searches with errors and "degenerate" chars in IUPAC fasta definition (using search schemes)
- word-based FM-Indices

[FM-Index]: https://doi.org/10.1109/SFCS.2000.892127
[`libsais-rs`]: https://github.com/feldroop/libsais-rs
[`proptest`]: https://github.com/proptest-rs/proptest
[documentation]: https://docs.rs/genedex
