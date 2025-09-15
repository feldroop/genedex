use genedex::{FmIndexConfig, alphabet};

fn main() {
    // This example shows how to use the FM-Index in the most basic way.

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
}
