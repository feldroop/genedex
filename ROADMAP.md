# Possible Future Extensions and Improvements (roughly in order of priority):

- integrate alternative rank impl into codebase and benchmark, docs for it
- NO further encoding experiments
- try a little bit about doing multiple rank operations at the same time (to effectively parallelize memory loads)

### Index for single texts

- more flexible alphabet API
    - allow alphabet with sentinel included in io representation
    - allow alphabet without sentinel (only usable for single text indexing)
- optimized version for single text without sentinel
- optimized construction directly from (fasta) file reader

### Nice to have, higher priority

- compress text and/or bwt at some point during construction (half/half buffer)
- space optimization for rarely occurring symbols (such as the sentinel and N in the human Genome)
    - maybe leverage the fact that such characters often occur in runs
- paired blocks for improved memory usage when using larger alphabets
- suffix array, lookup table compression using unconventional int widths (e.g. 33 bit)

### Very useful functionality, is the goal to eventually support

- bidirectional FM-Index
- searches with errors and "degenerate" chars in IUPAC fasta definition (using search schemes)

### Nice to have, but low priority

- gate rayon/OpenMP usage behind feature flag
- optional functionality for text recovery
- text sampled suffix array (optionally with text ids and other annotations),

### Large topics, might never happen

- optimizations for highly repetitive texts such as run length encoding
- FMD-Index
- word-based FM-Indices
