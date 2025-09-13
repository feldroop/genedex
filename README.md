# genedex: A Small and Fast FM-Index for Rust

Coming soon!

## Possible future extensions:

- API/structure:
    - better API for construction
    - better API for alphabets (alphabet should not be type anymore)
    - more alphabets + better test coverage for different alphabets
    - add text recovery features to public API
    - gate rayon/OpenMP usage behind feature flag (enabled by default)
- Optimization ideas for existing features:
    - space optimization for rarely occurring symbols (such as the sentinel and N in the human Genome), maybe leverage the fact that such characters often occur in runs
    - improved build memory usage (maybe a configurable, slower low memory mode): 
        - add u32 saca
        - BWT view optimization
        - suffix array, lookup table compression using unconventional int widths (e.g. 33 bit)
    - paired blocks for less memory usage when using larger alphabets (such as all possible u8 values except 0)
- Novel features (implementation + API):
    - bidirectional FM-Index
    - FMD-Index
    - searches with errors and "degenerate" chars as in IUPAC fasta definition (using search schemes)
    - optimized version for single text without sentinel
    - text sampled suffix array (maybe with text ids and other annotations),
    - optimized construction directly from (fasta) file reader
    - optimizations for hingly repetitive texts such as run length encoding
