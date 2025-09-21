use proptest::prelude::*;

use genedex::{IndexStorage, text_with_rank_support::*};

type OccurrenceColumn<T> = Vec<T>;

#[derive(Debug)]
struct NaiveTextWithRankSupport {
    data: Vec<OccurrenceColumn<usize>>,
}

impl NaiveTextWithRankSupport {
    pub fn construct(text: &[u8], alphabet_size: usize) -> Self {
        let mut data = Vec::new();

        for symbol in 0..alphabet_size {
            data.push(create_occurrence_column(symbol as u8, text));
        }

        Self { data }
    }

    // occurrences of the character in bwt[0, idx)
    pub fn rank(&self, symbol: u8, idx: usize) -> usize {
        self.data[symbol as usize][idx]
    }
}

fn create_occurrence_column(target_symbol: u8, bwt: &[u8]) -> Vec<usize> {
    let mut column = Vec::with_capacity(bwt.len() + 1);

    let mut count = 0;
    column.push(count);

    for &r in bwt {
        if r == target_symbol {
            count += 1;
        }

        column.push(count);
    }

    column
}

fn test_against_naive<I: IndexStorage, R: TextWithRankSupport<I>>(
    text: &[u8],
    alphabet_size: usize,
) {
    let text_rank: R = TextWithRankSupport::construct(text, alphabet_size);
    let naive_text_rank = NaiveTextWithRankSupport::construct(text, alphabet_size);

    assert_eq!(text_rank.text_len(), text.len());

    for (i, symbol) in text.iter().copied().enumerate() {
        assert_eq!(text_rank.symbol_at(i), symbol);
    }

    for symbol in 0..alphabet_size as u8 {
        for idx in 0..=text.len() {
            assert_eq!(
                text_rank.rank(symbol, idx),
                naive_text_rank.rank(symbol, idx),
                "symbol: {symbol}, idx: {idx}"
            );
        }
    }
}

fn test_different_block_sizes_against_naive(text: &[u8], alphabet_size: usize) {
    test_against_naive::<i32, CondensedTextWithRankSupport<i32, Block64>>(text, alphabet_size);
    test_against_naive::<u32, CondensedTextWithRankSupport<u32, Block512>>(text, alphabet_size);
    test_against_naive::<i64, FlatTextWithRankSupport<i64, Block64>>(text, alphabet_size);
    test_against_naive::<i32, FlatTextWithRankSupport<i32, Block512>>(text, alphabet_size);
}

#[test]
fn empty() {
    let alphabet_size = 2;
    let text = [];

    test_different_block_sizes_against_naive(&text, alphabet_size);
}

#[test]
fn superblock_size_text() {
    let superblock_size = u16::MAX as usize + 1;
    let alphabet_size = 3;
    let text: Vec<_> = [0u8, 1, 2, 2, 1, 0, 0, 0, 1, 2]
        .iter()
        .cycle()
        .copied()
        .take(superblock_size)
        .collect();

    test_different_block_sizes_against_naive(&text, alphabet_size);
}

// the key property of this test is that the text has the length 512
#[test]
fn failing_proptest0() {
    let alphabet_size = 27;
    let text = [
        2, 2, 25, 0, 15, 19, 23, 7, 18, 13, 1, 20, 16, 14, 19, 15, 3, 4, 13, 17, 12, 22, 21, 8, 5,
        11, 13, 25, 2, 21, 16, 22, 23, 19, 3, 13, 23, 19, 18, 20, 13, 23, 2, 2, 17, 6, 9, 5, 19,
        26, 4, 18, 20, 17, 18, 1, 20, 26, 13, 3, 15, 17, 7, 2, 26, 12, 11, 25, 18, 25, 17, 24, 8,
        14, 15, 3, 14, 9, 11, 26, 12, 18, 21, 8, 7, 22, 7, 9, 10, 2, 14, 9, 4, 21, 13, 4, 4, 7, 0,
        24, 4, 4, 7, 10, 2, 3, 11, 12, 16, 9, 5, 6, 10, 25, 21, 6, 16, 3, 23, 5, 4, 15, 14, 1, 12,
        15, 3, 24, 2, 25, 9, 1, 18, 21, 15, 13, 1, 22, 6, 10, 15, 14, 16, 16, 13, 24, 5, 2, 21, 16,
        6, 19, 22, 6, 24, 23, 26, 26, 19, 13, 26, 0, 23, 6, 24, 13, 2, 20, 10, 15, 13, 22, 25, 3,
        11, 14, 5, 0, 13, 15, 12, 22, 7, 14, 14, 23, 20, 14, 21, 12, 10, 15, 19, 23, 2, 16, 14, 13,
        8, 0, 18, 3, 23, 10, 6, 2, 19, 9, 11, 19, 9, 22, 1, 11, 5, 12, 21, 19, 26, 18, 15, 25, 12,
        18, 10, 22, 13, 5, 22, 23, 5, 19, 6, 19, 19, 7, 8, 2, 26, 18, 1, 21, 20, 15, 4, 24, 16, 5,
        5, 4, 15, 3, 23, 21, 23, 3, 6, 15, 23, 6, 7, 1, 25, 0, 22, 10, 3, 10, 7, 15, 26, 1, 22, 9,
        11, 22, 1, 8, 19, 10, 25, 3, 2, 14, 19, 23, 22, 15, 11, 5, 0, 21, 5, 6, 25, 0, 21, 26, 21,
        5, 11, 8, 9, 10, 8, 20, 5, 0, 2, 15, 12, 24, 6, 6, 16, 16, 21, 4, 5, 12, 4, 12, 5, 23, 22,
        25, 12, 12, 5, 7, 16, 13, 3, 19, 26, 17, 15, 0, 10, 4, 3, 3, 19, 11, 5, 20, 24, 1, 8, 6,
        26, 25, 12, 15, 25, 0, 7, 25, 1, 12, 2, 26, 25, 2, 2, 4, 18, 10, 0, 9, 21, 10, 22, 1, 0,
        22, 11, 7, 4, 4, 9, 14, 10, 19, 22, 23, 18, 18, 9, 5, 25, 3, 9, 10, 13, 3, 16, 12, 5, 7,
        14, 17, 24, 21, 14, 0, 13, 26, 21, 26, 25, 4, 26, 2, 23, 14, 10, 26, 3, 26, 21, 2, 24, 19,
        17, 11, 26, 9, 11, 11, 17, 14, 9, 2, 21, 8, 26, 22, 7, 11, 19, 7, 17, 17, 16, 11, 17, 22,
        20, 4, 14, 6, 17, 5, 18, 8, 17, 13, 4, 3, 18, 7, 17, 26, 9, 14, 22, 13, 23, 25, 12, 3, 7,
        8, 17, 12, 14, 10, 8, 17, 26, 22, 12, 20, 13, 25, 23, 9, 20, 7, 6, 11, 15, 26, 15, 1, 21,
        12, 0, 9, 0, 9, 19, 10, 19, 26, 26, 21, 7, 18, 6, 14,
    ];

    test_different_block_sizes_against_naive(&text, alphabet_size);
}

prop_compose! {
    fn text_over_alphabet()(max_symbol in 1u8..=255)
        (text in prop::collection::vec(0..=max_symbol, 0..1000), max_symbol in Just(max_symbol)) -> (Vec<u8>, usize) {
        (text, max_symbol as usize + 1)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_failure_persistence(prop::test_runner::FileFailurePersistence::WithSource("proptest-regressions")))]

    #[test]
    fn correctness_random_texts((text, alphabet_size) in text_over_alphabet()) {
        test_different_block_sizes_against_naive(&text, alphabet_size);
    }
}
