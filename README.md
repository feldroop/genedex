# genedex: A Small and Fast FM-Index for Rust

Coming soon!

## Possible future extensions and improvements (roughly in order of priority):

- improved build memory usage: 
    - configurable, slower low memory mode
    - u32 saca (maybe sais-drum)
    - BWT view optimization
    - suffix array, lookup table compression using unconventional int widths (e.g. 33 bit)
    - maybe compress compress text and/or bwt at some point during construction 
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
