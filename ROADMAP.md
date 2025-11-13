# Possible Future Extensions and Improvements:

I'm not sure how much I will be able to work on this in the future, so nothing is guaranteed. I won't implement a lot of this unless I hear from anyone who wants to actually use `genedex`. If you want to use the library, but are missing a specific feature, I'd be happy to hear from you and will give the missing feature a high priority.

### Next Milestones (High Priority)

- functionality to efficiently compute maximal exact matches (MEMs/SMEMs), FMD-Index. See https://github.com/feldroop/genedex/issues/1

### Optimizations for Existing Features

- paired blocks for improved memory usage when using larger alphabets
- in the search, `lookup_tables::compute_lookup_idx_static_len` still seems to be one of the bottlenecks. this
    should be investigated further, maybe it can be optimized or it's a measuring error.
- the batching of search queries could be improved. Currenty, it is not efficent if the queries have very different lengths
    or if many of them quickly get an empty interval, while others need ot be searched to the very end.
- the batched rank function could also be optimized using const currying and other techniques
- a faster `u32`-SACA to make the low memory mode less painful (`sais-drum` is a start, but optimizing it would be a lot of work)
- suffix array, lookup table compression using unconventional int widths (e.g. 33 bit)
- investigate large space usage of u8_until alphabet (https://github.com/RagnarGrootKoerkamp/quadrank)
### Small New Features/Tweaks

- gate rayon/OpenMP usage behind feature flag
- API to use batched search with cursors
- type-erase index storage type and choose automatically for text size
- optimized functions for reading directly from input files: both for texts to build the index and queries to search.
    the latter might be more important, because for simple searches, the search can be faster than reading the 
    queries from disk.
- more documentation tests

### Large New Features

- bidirectional FM-Index
- searches with errors and "degenerate" chars in IUPAC fasta definition (using search schemes, needs bidirectional FM-Index)
- optimizations for highly repetitive texts such as run length encoding (r-index). This would be simpler, but much less useful than a ropeBWT-based FM-Index
- ropeBWT/dynamic FM-Index
- word-based FM-Index
