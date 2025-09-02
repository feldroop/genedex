use std::marker::PhantomData;

use crate::alphabet::Alphabet;

type OccurrenceColumn<T> = Vec<T>;

// parallel construction?
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub(crate) struct NaiveOccurrenceTable<A> {
    data: Vec<OccurrenceColumn<usize>>,
    _alphabet_maker: PhantomData<A>,
}

impl<A: Alphabet> NaiveOccurrenceTable<A> {
    pub(crate) fn construct(bwt: &[u8]) -> Self {
        let mut data = Vec::new();

        for rank in 1..=A::size() {
            data.push(create_occurrence_column(rank as u8, bwt));
        }

        Self {
            data,
            _alphabet_maker: PhantomData,
        }
    }

    // rank should not be zero
    // occurrences of the character in bwt[0, idx)
    pub(crate) fn occurrences(&self, rank: u8, idx: usize) -> usize {
        self.data[(rank - 1) as usize][idx]
    }

    pub(crate) fn bwt_rank_at(&self, idx: usize) -> u8 {
        for (i, column) in self.data.iter().enumerate() {
            if column[idx] < column[idx + 1] {
                return (i + 1) as u8;
            }
        }

        unreachable!()
    }
}

fn create_occurrence_column(target_rank: u8, bwt: &[u8]) -> Vec<usize> {
    let mut column = Vec::with_capacity(bwt.len() + 1);

    let mut count = 0;
    column.push(count);

    for &r in bwt {
        if r == target_rank {
            count += 1;
        }

        column.push(count);
    }

    column
}
