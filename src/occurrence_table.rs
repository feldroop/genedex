use std::marker::PhantomData;

use crate::alphabet::Alphabet;

type OccurrenceColumn<T> = Vec<T>;

// parallel construction?
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub(crate) struct StringRank<A> {
    data: Vec<OccurrenceColumn<usize>>,
    _alphabet_maker: PhantomData<A>,
}

impl<A: Alphabet> StringRank<A> {
    pub(crate) fn construct(bwt: &[u8]) -> Self {
        let mut data = Vec::new();

        for symbol in 0..A::size() {
            data.push(create_occurrence_column(symbol as u8, bwt));
        }

        Self {
            data,
            _alphabet_maker: PhantomData,
        }
    }

    // rank should not be zero
    // occurrences of the character in bwt[0, idx)
    pub(crate) fn rank(&self, symbol: u8, idx: usize) -> usize {
        self.data[symbol as usize][idx]
    }

    pub(crate) fn symbol_at(&self, idx: usize) -> u8 {
        for (symbol, column) in self.data.iter().enumerate() {
            if column[idx] < column[idx + 1] {
                return symbol as u8;
            }
        }

        unreachable!()
    }

    pub(crate) fn len(&self) -> usize {
        self.data.first().unwrap().len() - 1
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
