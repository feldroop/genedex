# ⚡genedex: A Small and Fast FM-Index for Rust⚡

[![Build Status](https://img.shields.io/github/actions/workflow/status/feldroop/genedex/rust.yml?style=flat-square&logo=github&label=CI)](https://github.com/feldroop/genedex/actions)
[![Crates.io](https://img.shields.io/crates/v/genedex.svg?style=flat-square&logo=rust)](https://crates.io/crates/genedex)
[![Documentation](https://img.shields.io/docsrs/genedex?style=flat-square&logo=rust)](https://docs.rs/genedex)

The [FM-Index] is a full-text index data structure that allows efficiently counting and retrieving all occurrenes of short sequences in very large texts. It is widely used in sequence analysis and bioinformatics.

The implementation of this library is based on an encoding for the text with rank support data structure (a.k.a. occurrence table)
by Simon Gene Gottlieb (publication pending), who also was a great help while developing this library. This encoding attemps to provide a good trade-off between
memory usage and running time of queries. Further benefits of `genedex` include:

- Fast, parallel and memory efficient index construction by leveraging [`libsais-rs`] and [`rayon`].
- Configurable, very low memory mode for index construction.
- Support for indexing a set of texts, like chromosomes of a genome.
- A flexible cursor API.
- Fast reading and writing the FM-Index from/to files, using [`savefile`].
- Thoroughly tested using [`proptest`].

⚠️ **Warning:** this library is in an early stage. The API is still subject to changes. Currently, only a basic FM-Index is implemented. For upcoming features, take a look at the [roadmap]. Any kind of feedback and suggestions via the issue tracker is highly appreciated! ⚠️

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

[FM-Index]: https://doi.org/10.1109/SFCS.2000.892127
[`libsais-rs`]: https://github.com/feldroop/libsais-rs
[`rayon`]: https://github.com/rayon-rs/rayon
[`savefile`]: https://github.com/avl/savefile
[`proptest`]: https://github.com/proptest-rs/proptest
[roadmap]: ./ROADMAP.md
[documentation]: https://docs.rs/genedex
