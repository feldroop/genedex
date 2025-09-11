# genedex: A Small and Fast FM-Index for Rust

Coming soon!

## Possible future extensions:

- API/structure:
    - better API for construction
    - better API for alphabets, the current one is not fleshed out
    - more alphabets + better test coverage for different alphabets
    - flexible cursor API
    - gate rayon/OpenMP usage behind feature flag (enabled by default)
- Optimization ideas for existing features:
    - space optimization for rarely occurring symbols (such as the sentinel and N in the human Genome),
    - improved build memory usage (maybe a configurable, slower low memory mode): 
        - add u32 saca
        - BWT only as view optimization 
        - suffix array compression using unconventional int widths (e.g. 33 bit)
    - paired blocks for less memory usage when using large alphabets (e.g. all possible u8 values)
- Novel Features (implementation + API):
    - bidirectional FM-Index
    - searches with errors and "degenerate" chars as in IUPAC fasta definition (using search schemes)
    - optimized version for single text without sentinel
    - text sampled suffix array (maybe with text ids and other annotations),
    - optimized construction directly from (fasta) file reader
