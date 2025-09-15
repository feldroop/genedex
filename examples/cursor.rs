use genedex::{FmIndexConfig, alphabet};

fn main() {
    // This eample showcases the flexible cursor API for the FM-Index of this library.

    let dna_n_alphabet = alphabet::ascii_dna_with_n();
    let texts = [b"AaACGT".as_slice(), b"AacGtn", b"GTGTGT"];

    let index = FmIndexConfig::<i32>::new().construct_index(texts, dna_n_alphabet);

    let query = b"GT";

    // We obtain a cursor that points to the index. The cursor maintains a currently searched query.
    // Symbols can iteratively be added to the front of this query.
    let mut cursor = index.cursor_for_query(query);

    // There are too many occurrences for our taste.
    assert_eq!(cursor.count(), 5);

    // So we extend the currently searched query by a symbol.
    cursor.extend_query_front(b'C');

    // That's better!
    assert_eq!(cursor.count(), 2);

    for hit in cursor.locate() {
        println!(
            "Found query in text {} at position {}.",
            hit.text_id, hit.position
        );
    }
}
