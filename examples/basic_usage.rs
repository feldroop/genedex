use genedex::{FmIndexConfig, PerformancePriority, alphabet};

fn main() {
    // This example shows how to use the FM-Index in a basic way.

    let dna_n_alphabet = alphabet::ascii_dna_with_n();
    let texts = [b"aACGT", b"acGtn"];

    let index = FmIndexConfig::<i32>::new()
        .suffix_array_sampling_rate(2)
        .lookup_table_depth(0)
        .construction_performance_priority(PerformancePriority::Balanced)
        .construct_index(texts, dna_n_alphabet);

    let query = b"GT";
    assert_eq!(index.count(query), 2);

    for hit in index.locate(query) {
        println!(
            "Found query in text {} at position {}.",
            hit.text_id, hit.position
        );
    }

    // for many queries, the locate_many function can be used for convenience and to improve running time
    let many_queries = [b"AC".as_slice(), b"CG", b"GT", b"GTN"];

    for (query_id, hits) in index.locate_many(many_queries).enumerate() {
        for hit in hits {
            println!(
                "Found query {query_id} in text {} at position {}.",
                hit.text_id, hit.position
            );
        }
    }
}
