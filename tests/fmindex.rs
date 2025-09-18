use genedex::{FmIndex, FmIndexConfig, Hit, IndexStorage, PerformancePriority, alphabet};
use proptest::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;

fn create_index<I: IndexStorage>() -> FmIndex<I> {
    let text = b"cccaaagggttt".as_slice();
    FmIndexConfig::<I>::new()
        .lookup_table_depth(0)
        .suffix_array_sampling_rate(3)
        .construct_index([text], alphabet::ascii_dna())
}

static BASIC_QUERY: &[u8] = b"gg";
static FRONT_QUERY: &[u8] = b"c";
static WRAPPING_QUERY: &[u8] = b"ta";
static MULTI_QUERY: &[u8] = b"gt";

#[test]
fn basic_search() {
    let index = create_index::<i32>();
    let index_u32_compressed = create_index::<u32>();

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
    let index = create_index::<i32>();
    let index_u32_compressed = create_index::<u32>();

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
    let index = create_index::<i32>();
    let index_u32_compressed = create_index::<u32>();

    let results: HashSet<_> = index.locate(WRAPPING_QUERY).collect();
    let results_u32_compressed: HashSet<_> = index_u32_compressed.locate(WRAPPING_QUERY).collect();

    assert!(results.is_empty());
    assert!(results_u32_compressed.is_empty());
}

#[test]
fn search_multitext() {
    let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];

    let index = FmIndexConfig::<u32>::new()
        .lookup_table_depth(4)
        .suffix_array_sampling_rate(3)
        .construct_index(texts, alphabet::ascii_dna());

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

#[test]
fn u8_alphabet() {
    let texts = &[
        [0, 4, 3, 2, 1, 5, 8, 6, 7, 8].as_slice(),
        &[5, 7, 3, 4, 2, 1, 5, 8],
        &[],
    ];

    let index = FmIndexConfig::<u32>::new()
        .lookup_table_depth(4)
        .suffix_array_sampling_rate(3)
        .construct_index(texts, alphabet::u8_until(8));

    let expected_results = HashSet::from_iter([
        Hit {
            text_id: 0,
            position: 4,
        },
        Hit {
            text_id: 1,
            position: 5,
        },
    ]);

    let results: HashSet<_> = index.locate(&[1, 5, 8]).collect();
    assert_eq!(results, expected_results);
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
        let extent_range = 0..std::cmp::min(self.max_extent, text.len() - position + 1);
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
        let len = self.rng.random_range(0..self.max_len);
        let mut query = vec![0; len];
        for q in query.iter_mut() {
            *q = b"ACGT"[self.rng.random_range(0..4)];
        }

        Some(query)
    }
}

fn naive_search(texts: &[Vec<u8>], query: &[u8]) -> HashSet<Hit> {
    let mut hits = HashSet::new();

    for (text_id, text) in texts.iter().enumerate() {
        if query.len() == 0 {
            for position in 0..=text.len() {
                hits.insert(Hit { text_id, position });
            }

            continue;
        }

        for (position, window) in text.windows(query.len()).enumerate() {
            if window == query {
                hits.insert(Hit { text_id, position });
            }
        }
    }

    hits
}

fn run_queries<I: IndexStorage>(
    index: &FmIndex<I>,
    existing_queries: &[(Hit, &[u8])],
    random_queries: &[Vec<u8>],
    random_queries_naive_hits: &[HashSet<Hit>],
) {
    for (hit, query) in existing_queries {
        let results: HashSet<_> = index.locate(query).collect();
        assert!(results.contains(&hit));
    }

    for (query, naive_results) in random_queries.iter().zip(random_queries_naive_hits) {
        let results: HashSet<_> = index.locate(query).collect();

        assert_eq!(&results, naive_results);
    }
}

#[test]
fn proptest_fail() {
    let texts = vec![vec![71, 65]];
    // [3, 1, 0]

    let mut rng = ChaCha8Rng::seed_from_u64(0);

    let existing_queries: Vec<_> = QuerySampler {
        texts: &texts,
        max_extent: 200,
        rng: &mut rng,
    }
    .take(20)
    .collect();
    let random_queries: Vec<_> = RandomQueryGenerator {
        max_len: 20,
        rng: &mut rng,
    }
    .take(100)
    .collect();

    let random_queries_naive_hits: Vec<_> = random_queries
        .iter()
        .map(|q| naive_search(&texts, q))
        .collect();

    let index_i32 = FmIndexConfig::<i32>::new()
        .lookup_table_depth(0)
        .suffix_array_sampling_rate(1)
        .construct_index(&texts, alphabet::ascii_dna());

    run_queries(
        &index_i32,
        &existing_queries,
        &random_queries,
        &random_queries_naive_hits,
    );
}

proptest! {
    //#![proptest_config(ProptestConfig::with_failure_persistence(prop::test_runner::FileFailurePersistence::WithSource("proptest-regressions")))]

    #[test]
    fn correctness_random_texts(
        texts in prop::collection::vec(
            prop::collection::vec((0usize..4).prop_map(|i| b"ACGT"[i]), 0..1500),
            1..5
        ),
        suffix_array_sampling_rate in 1usize..=64,
        num_threads in 1u16..4,
        lookup_table_depth in 0usize..6,
        performance_priority in (0usize..3).prop_map(|i| [PerformancePriority::Balanced, PerformancePriority::HighSpeed, PerformancePriority::LowMemory][i]),
        seed in any::<u64>(),
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
            let index_i32 = FmIndexConfig::<i32>::new()
                .lookup_table_depth(lookup_table_depth)
                .suffix_array_sampling_rate(suffix_array_sampling_rate).construction_performance_priority(performance_priority)
                .construct_index(&texts, alphabet::ascii_dna());
            let index_u32 = FmIndexConfig::<u32>::new()
                .lookup_table_depth(lookup_table_depth)
                .suffix_array_sampling_rate(suffix_array_sampling_rate).construction_performance_priority(performance_priority)
                .construct_index(&texts, alphabet::ascii_dna_with_n());
            let index_i64 = FmIndexConfig::<i64>::new()
                .lookup_table_depth(lookup_table_depth)
                .suffix_array_sampling_rate(suffix_array_sampling_rate).construction_performance_priority(performance_priority)
                .construct_index(&texts, alphabet::ascii_dna_iupac_as_dna_with_n());

            run_queries(&index_i32, &existing_queries,&random_queries, &random_queries_naive_hits);
            run_queries(&index_u32, &existing_queries,&random_queries, &random_queries_naive_hits);
            run_queries(&index_i64, &existing_queries,&random_queries, &random_queries_naive_hits);
        });
    }
}
