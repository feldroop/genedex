# Possible Future Extensions and Improvements:

I'm not sure how much I will be able to work on this in the future, so nothing is guaranteed. Also, I believe in YAGNI and won't implement a lot of this unless I hear from anyone who wants to actually use `genedex`. I you want to use the library, but are missing a specific feature, I'd be happy to hear from you and will give the missing feature a high priority.

### Optimizations for existing features

- space optimization for rarely occurring symbols (such as the sentinel and N in the human Genome)
    - maybe leverage the fact that such characters (namely N) often occur in runs
    - the sentinel can not be searched. the current sampled suffix array implementation has special handling for it.
        so it technically doesn't have to be stored in the text with rank support. If N also gets special handling,
        the condensed text with rank support will get smaller and maybe faster. 
        A text sampled suffix array could be an option, or a "sparse" text with rank support substructure.
- paired blocks for improved memory usage when using larger alphabets
- in the search, `lookup_tables::compute_lookup_idx_static_len` still seems to be one of the bottlenecks. this
    should be investigated further, maybe it can be optimized or it's a measuring error.
- the batching of search queries could be improved. Currenty, it is not efficent if the queries have very different lengths
    or if many of them quickly get an empty interval, while others need ot be searched to the very end.
- the batched rank function could also be optimized using const currying and other techniques
- a faster `u32`-SACA to make the low memory mode less painful (`sais-drum` is a start, but optimizing it would be a lot of work)
- suffix array, lookup table compression using unconventional int widths (e.g. 33 bit)

### Smaller new features

- more flexible alphabet API
    - allow alphabet with sentinel included in io representation
    - allow alphabet without sentinel (only usable for single text indexing)
- optimized version for single text without sentinel
- functionality to directly retrieve maximal exact matches (MEMs/SMEMs)
- more documentation tests
- gate rayon/OpenMP usage behind feature flag
- API to use batched search with cursors
- type-erase index storage type and choose automatically for text size (does that work with savefile?)
- bidirectional FM-Index
- optimizations for highly repetitive texts such as run length encoding (r-index)
- optional functionality for text recovery
- text sampled suffix array (with text ids and optionally other annotations)
- optimized functions for reading directly from input files: both for texts to build the index and queries to search.
    the latter might be more important, because for simple searches, the search can be faster than reading the 
    queries from disk.

### Larger new features, might never happen

- searches with errors and "degenerate" chars in IUPAC fasta definition (using search schemes, needs bidirectional FM-Index)
- FMD-Index
- word-based FM-Indices
- ropeBWT/dynamic FM-Index
