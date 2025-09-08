use genedex::{
    FmIndexI32, FmIndexI64, FmIndexU32, Hit,
    alphabet::{AsciiDna, AsciiDnaWithN},
};
use proptest::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
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

    let expected_results = HashSet::from_iter([
        Hit {
            text_id: 0,
            position: 6,
        },
        Hit {
            text_id: 0,
            position: 7,
        },
    ]);

    assert_eq!(results, expected_results);
    assert_eq!(results_u32_compressed, expected_results);
}

#[test]
fn text_front_search() {
    let index = create_index();
    let index_u32_compressed = create_index_u32_compressed();

    let results: HashSet<_> = index.locate(FRONT_QUERY).collect();
    let results_u32_compressed: HashSet<_> = index_u32_compressed.locate(FRONT_QUERY).collect();

    let expected_results = HashSet::from_iter([
        Hit {
            text_id: 0,
            position: 0,
        },
        Hit {
            text_id: 0,
            position: 1,
        },
        Hit {
            text_id: 0,
            position: 2,
        },
    ]);

    assert_eq!(results, expected_results);
    assert_eq!(results_u32_compressed, expected_results);
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

    let expected_results_basic_query = HashSet::from_iter([
        Hit {
            text_id: 0,
            position: 6,
        },
        Hit {
            text_id: 0,
            position: 7,
        },
    ]);

    let results_basic_query: HashSet<_> = index.locate(BASIC_QUERY).collect();
    assert_eq!(results_basic_query, expected_results_basic_query);

    let expected_results_multi_query = HashSet::from_iter([
        Hit {
            text_id: 0,
            position: 8,
        },
        Hit {
            text_id: 1,
            position: 2,
        },
        Hit {
            text_id: 1,
            position: 6,
        },
        Hit {
            text_id: 1,
            position: 10,
        },
    ]);

    let results_multi_query: HashSet<_> = index.locate(MULTI_QUERY).collect();
    assert_eq!(results_multi_query, expected_results_multi_query);
}

struct QuerySampler<'t, 'r> {
    texts: &'t [Vec<u8>],
    rng: &'r mut ChaCha8Rng,
    max_extent: usize,
}

impl<'t, 'r> Iterator for QuerySampler<'t, 'r> {
    type Item = (Hit, &'t [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.texts.is_empty() {
            return None;
        }
        let text_id = self.rng.random_range(0..self.texts.len());
        let text = &self.texts[text_id];

        if text.is_empty() {
            return None;
        }

        let position = self.rng.random_range(0..text.len());
        let extent_range = 1..std::cmp::min(self.max_extent, text.len() - position + 1);
        let extent = self.rng.random_range(extent_range);

        Some((
            Hit { text_id, position },
            &text[position..position + extent],
        ))
    }
}

struct RandomQueryGenerator<'r> {
    max_len: usize,
    rng: &'r mut ChaCha8Rng,
}

impl<'r> Iterator for RandomQueryGenerator<'r> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.rng.random_range(1..self.max_len);
        let mut query = vec![0; len];
        for q in query.iter_mut() {
            *q = b"ACGTN"[self.rng.random_range(0..5)];
        }

        Some(query)
    }
}

fn naive_search(texts: &[Vec<u8>], query: &[u8]) -> HashSet<Hit> {
    let mut hits = HashSet::new();

    for (text_id, text) in texts.iter().enumerate() {
        for (position, window) in text.windows(query.len()).enumerate() {
            if window == query {
                hits.insert(Hit { text_id, position });
            }
        }
    }

    hits
}

proptest! {
    #![proptest_config(ProptestConfig::with_failure_persistence(prop::test_runner::FileFailurePersistence::WithSource("proptest-regressions")))]

    #[test]
    fn correctness_random_texts(
        texts in prop::collection::vec(
            prop::collection::vec((0usize..5).prop_map(|i| b"ACGTN"[i]), 0..1500),
            1..5
        ),
        suffix_array_sampling_rate in 1usize..=64,
        num_threads in 1u16..4,
        seed in any::<u64>()
    ) {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads as usize)
            .build()
            .unwrap();

        let mut rng = ChaCha8Rng::seed_from_u64(seed);

        let existing_queries: Vec<_> = QuerySampler{texts: &texts, max_extent: 200, rng: &mut rng }.take(20).collect();
        let random_queries: Vec<_> = RandomQueryGenerator{max_len: 20, rng: &mut rng}.take(100).collect();

        let random_queries_naive_hits: Vec<_> = random_queries.iter().map(|q| naive_search(&texts, q)).collect();

        pool.install(|| {
            let index_i32 = FmIndexI32::<AsciiDnaWithN>::new(&texts, num_threads, suffix_array_sampling_rate);
            let index_u32 = FmIndexU32::<AsciiDnaWithN>::new_u32_compressed(&texts, num_threads, suffix_array_sampling_rate);
            let index_i64 = FmIndexI64::<AsciiDnaWithN>::new(&texts, num_threads, suffix_array_sampling_rate);

            for (hit, query) in existing_queries {
                let results_i32: HashSet<_> = index_i32.locate(query).collect();
                let results_u32: HashSet<_> = index_u32.locate(query).collect();
                let results_i64: HashSet<_> = index_i64.locate(query).collect();

                assert!(results_i32.contains(&hit));
                assert!(results_u32.contains(&hit));
                assert!(results_i64.contains(&hit));
            }

            for (query, naive_results) in random_queries.iter().zip(random_queries_naive_hits) {
                let results_i32: HashSet<_> = index_i32.locate(query).collect();
                let results_u32: HashSet<_> = index_u32.locate(query).collect();
                let results_i64: HashSet<_> = index_i64.locate(query).collect();

                assert_eq!(results_i32, naive_results);
                assert_eq!(results_u32, naive_results);
                assert_eq!(results_i64, naive_results);
            }
        });
    }
}
