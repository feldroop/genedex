use genedex::{FmIndexI32, FmIndexU32, alphabet::AsciiDna};
use std::collections::HashSet;

fn create_index() -> FmIndexI32<AsciiDna> {
    let text = b"cccaaagggttt".as_slice();

    FmIndexI32::new([text], 1, 3)
}

fn create_index_u32_compressed() -> FmIndexU32<AsciiDna> {
    let text = b"cccaaagggttt".as_slice();

    FmIndexU32::new_u32_compressed([text], 1, 3)
}

static BASIC_QUERY: &[u8] = b"gg";
static FRONT_QUERY: &[u8] = b"c";
static WRAPPING_QUERY: &[u8] = b"ta";
static MULTI_QUERY: &[u8] = b"gt";

#[test]
fn basic_search() {
    let index = create_index();
    let index_u32_compressed = create_index_u32_compressed();

    let results: HashSet<_> = index.locate(BASIC_QUERY).collect();
    let results_u32_compressed: HashSet<_> = index_u32_compressed.locate(BASIC_QUERY).collect();

    assert_eq!(results, HashSet::from_iter([(0, 6), (0, 7)]));
    assert_eq!(results_u32_compressed, HashSet::from_iter([(0, 6), (0, 7)]));
}

#[test]
fn text_front_search() {
    let index = create_index();
    let index_u32_compressed = create_index_u32_compressed();

    let results: HashSet<_> = index.locate(FRONT_QUERY).collect();
    let results_u32_compressed: HashSet<_> = index_u32_compressed.locate(FRONT_QUERY).collect();

    assert_eq!(results, HashSet::from_iter([(0, 0), (0, 1), (0, 2)]));
    assert_eq!(
        results_u32_compressed,
        HashSet::from_iter([(0, 0), (0, 1), (0, 2)])
    );
}

#[test]
fn search_no_wrapping() {
    let index = create_index();
    let index_u32_compressed = create_index_u32_compressed();

    let results: HashSet<_> = index.locate(WRAPPING_QUERY).collect();
    let results_u32_compressed: HashSet<_> = index_u32_compressed.locate(WRAPPING_QUERY).collect();

    assert!(results.is_empty());
    assert!(results_u32_compressed.is_empty());
}

#[test]
fn search_multitext() {
    let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];

    let index = FmIndexU32::<AsciiDna>::new_u32_compressed(texts, 1, 3);

    let results_basic_query: HashSet<_> = index.locate(BASIC_QUERY).collect();
    assert_eq!(results_basic_query, HashSet::from_iter([(0, 6), (0, 7)]));

    let results_multi_query: HashSet<_> = index.locate(MULTI_QUERY).collect();
    assert_eq!(
        results_multi_query,
        HashSet::from_iter([(0, 8), (1, 2), (1, 6), (1, 10)])
    );
}
