# genedex: A Small and Fast FM-Index for Rust

Coming soon!

## Possible future additions:
    - API:
        - other alphabets + better test coverage for different alphabets + improved API for alphabets
        - flexible cursor API
    - Optimization ideas for existing features:
        - space optimization for rarely occurring symbols (such as the sentinel and N in the human Genome),
        - optimized version for single text without sentinel
        - improve build memory usage: 
            - add u32 saca
            - BWT only as view optimization 
            - suffix array compression using unconventional int widths (e.g. 33 bit)
        - paired blocks for less memory usage when using large alphabets (e.g. u8)
    - Novel Features (implementation + API):
        - bidirectional FM-Index
        - searches with errors and semantically correct handlign of degenerate IUPAC fasta chars (using search schemes)
        - text sampled suffix array (maybe with text ids and other annotations),
        - optimized construction directly from (fasta) file reader
