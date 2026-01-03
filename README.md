# ⚡genedex: A Small and Fast FM-Index for Rust⚡

[![Build Status](https://img.shields.io/github/actions/workflow/status/feldroop/genedex/rust.yml?style=flat-square&logo=github&label=CI)](https://github.com/feldroop/genedex/actions)
[![Crates.io](https://img.shields.io/crates/v/genedex.svg?style=flat-square&logo=rust)](https://crates.io/crates/genedex)
[![Documentation](https://img.shields.io/docsrs/genedex?style=flat-square&logo=rust)](https://docs.rs/genedex)

⚠️ **Warning:** this library is in an early stage. The API is still subject to changes. Currently, only a basic FM-Index is implemented. For upcoming features, take a look at the [roadmap]. Any kind of feedback and suggestions via the issue tracker is highly appreciated! ⚠️

The [FM-Index] is a full-text index data structure that allows efficiently counting and retrieving the positions of all occurrenes of (typically short) sequences in very large texts. It is widely used in sequence analysis and bioinformatics.
The benefits of `genedex` include:

- Multiple optimized implementations with different running time/memory usage trade-offs.
- Fast, parallel and memory efficient index construction by leveraging [`libsais-rs`] and [`rayon`].
- Support for indexing a set of texts, like chromosomes of a genome.
- A flexible cursor API.
- Fast reading and writing the FM-Index from/to files, using [`savefile`].
- Thoroughly tested using [`proptest`].
- Experimental, optimized functions for searching multiple queries at once. This is not multithreading. It batches searches on a single thread to leverage SIMD and saturate (multichannel) RAM bandwidth.

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

## References

- The default (_condensed_) implementation is based on:

    > Gottlieb, S.G., Reinert, K.: _Engineering rank queries on bit vectors and strings_ (2025) DOI: [10.1186/s13015-025-00291-9](https://doi.org/10.1186/s13015-025-00291-9)

## Comparison to Other Crates and Benchmarks

A thorough comparison of all available Rust implementations of the FM-Index can be found [here](https://github.com/feldroop/rust-fmindex-benchmark). The main benchmark results are shown below.

Running time and peak memory usage of constructing the FM-Index for the human reference genome hg38 using different implementations and configurations:

<img src="https://raw.githubusercontent.com/feldroop/rust-fmindex-benchmark/refs/heads/main/plots/img/Construction-Hg38.svg" />

Running time and index memory usage of searching 377 MB of queries of length 50 in this index using the `locate` function:

<img src="https://raw.githubusercontent.com/feldroop/rust-fmindex-benchmark/refs/heads/main/plots/img/Locate-Hg38.svg" />

## Acknowledgements

- The implementation of this library is based on an encoding for the text with rank support data structure (a.k.a. occurrence table)
by Simon Gene Gottlieb, who also was a great help while developing the library.
- Ragnar Groot Koerkamp, who develops the library [`quadrank`], was very helpful and supplied bug reports, suggestions and improvements to `genedex`.

[FM-Index]: https://doi.org/10.1109/SFCS.2000.892127
[`libsais-rs`]: https://github.com/feldroop/libsais-rs
[`rayon`]: https://github.com/rayon-rs/rayon
[`savefile`]: https://github.com/avl/savefile
[`proptest`]: https://github.com/proptest-rs/proptest
[roadmap]: ./ROADMAP.md
[documentation]: https://docs.rs/genedex
[`quadrank`]: https://github.com/RagnarGrootKoerkamp/quadrank
