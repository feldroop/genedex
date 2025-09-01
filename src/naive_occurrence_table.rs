type OccurrenceColumn<T> = Vec<T>;

#[derive(Debug)]
pub(crate) struct NaiveOccurrenceTable {
    data: Vec<OccurrenceColumn<usize>>,
}

impl NaiveOccurrenceTable {
    pub(crate) fn construct(alphabet_size: usize, bwt: &[u8]) -> Self {
        let mut data = Vec::new();

        for char in 0..alphabet_size {
            data.push(create_occurrence_column(char as u8, bwt));
        }

        Self { data }
    }

    pub(crate) fn occurrences(&self, character: u8, index: usize) -> usize {
        self.data[character as usize][index]
    }

    pub(crate) fn bwt_char_at(&self, index: usize) -> u8 {
        for (i, column) in self.data.iter().enumerate() {
            if column[index] < column[index + 1] {
                return i as u8;
            }
        }

        unreachable!()
    }
}

// occurrences of the character in bwt[0, index)
fn create_occurrence_column(target_char: u8, bwt: &[u8]) -> Vec<usize> {
    let mut column = Vec::with_capacity(bwt.len() + 1);

    let mut count = 0;
    column.push(count);

    for &c in bwt {
        if c == target_char {
            count += 1;
        }

        column.push(count);
    }

    column
}
