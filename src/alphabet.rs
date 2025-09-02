use libsais::OutputElement;
use num_traits::NumCast;
use rayon::prelude::*;

pub trait Alphabet {
    const TO_RANK_TRANSLATION_TABLE: [u8; 256];
    fn size() -> usize;
}

const ASCII_DNA_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = [255; 256];

    table[b'A' as usize] = 1;
    table[b'a' as usize] = 1;

    table[b'C' as usize] = 2;
    table[b'c' as usize] = 2;

    table[b'G' as usize] = 3;
    table[b'g' as usize] = 3;

    table[b'T' as usize] = 4;
    table[b't' as usize] = 4;

    table
};

const ASCII_DNA_N_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = ASCII_DNA_TRANSLATION_TABLE;
    table[b'N' as usize] = 5;
    table[b'n' as usize] = 5;

    table
};

const ASCII_DNA_IUPAC_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = ASCII_DNA_N_TRANSLATION_TABLE;
    table[b'R' as usize] = 6;
    table[b'r' as usize] = 6;

    table[b'Y' as usize] = 7;
    table[b'y' as usize] = 7;

    table[b'K' as usize] = 8;
    table[b'k' as usize] = 8;

    table[b'M' as usize] = 9;
    table[b'm' as usize] = 9;

    table[b'S' as usize] = 10;
    table[b's' as usize] = 10;

    table[b'W' as usize] = 11;
    table[b'w' as usize] = 11;

    table[b'B' as usize] = 12;
    table[b'b' as usize] = 12;

    table[b'D' as usize] = 13;
    table[b'd' as usize] = 13;

    table[b'H' as usize] = 14;
    table[b'h' as usize] = 14;

    table[b'V' as usize] = 15;
    table[b'v' as usize] = 15;

    table
};

const ASCII_DNA_IUPAC_AS_DNA_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = ASCII_DNA_TRANSLATION_TABLE;
    table[b'R' as usize] = 1;
    table[b'r' as usize] = 1;

    table[b'Y' as usize] = 2;
    table[b'y' as usize] = 2;

    table[b'K' as usize] = 3;
    table[b'k' as usize] = 3;

    table[b'M' as usize] = 1;
    table[b'm' as usize] = 1;

    table[b'S' as usize] = 2;
    table[b's' as usize] = 2;

    table[b'W' as usize] = 1;
    table[b'w' as usize] = 1;

    table[b'B' as usize] = 2;
    table[b'b' as usize] = 2;

    table[b'D' as usize] = 1;
    table[b'd' as usize] = 1;

    table[b'H' as usize] = 1;
    table[b'h' as usize] = 1;

    table[b'V' as usize] = 1;
    table[b'v' as usize] = 1;

    table
};

const ASCII_DNA_IUPAC_AS_DNA_N_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = ASCII_DNA_N_TRANSLATION_TABLE;
    table[b'R' as usize] = 5;
    table[b'r' as usize] = 5;

    table[b'Y' as usize] = 5;
    table[b'y' as usize] = 5;

    table[b'K' as usize] = 5;
    table[b'k' as usize] = 5;

    table[b'M' as usize] = 5;
    table[b'm' as usize] = 5;

    table[b'S' as usize] = 5;
    table[b's' as usize] = 5;

    table[b'W' as usize] = 5;
    table[b'w' as usize] = 5;

    table[b'B' as usize] = 5;
    table[b'b' as usize] = 5;

    table[b'D' as usize] = 5;
    table[b'd' as usize] = 5;

    table[b'H' as usize] = 5;
    table[b'h' as usize] = 5;

    table[b'V' as usize] = 5;
    table[b'v' as usize] = 5;

    table
};

pub struct AsciiDna {}

impl Alphabet for AsciiDna {
    const TO_RANK_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_TRANSLATION_TABLE;

    fn size() -> usize {
        4
    }
}

pub struct AsciiDnaWithN {}

impl Alphabet for AsciiDnaWithN {
    const TO_RANK_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_N_TRANSLATION_TABLE;

    fn size() -> usize {
        5
    }
}

pub struct AsciiDnaIupac {}

impl Alphabet for AsciiDnaIupac {
    const TO_RANK_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_IUPAC_TRANSLATION_TABLE;

    fn size() -> usize {
        16
    }
}

pub struct AsciiDnaIupacAsDna {}

impl Alphabet for AsciiDnaIupacAsDna {
    const TO_RANK_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_IUPAC_AS_DNA_TRANSLATION_TABLE;

    fn size() -> usize {
        4
    }
}

pub struct AsciiDnaIupacAsDnaWithN {}

impl Alphabet for AsciiDnaIupacAsDnaWithN {
    const TO_RANK_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_IUPAC_AS_DNA_N_TRANSLATION_TABLE;

    fn size() -> usize {
        5
    }
}

pub(crate) fn create_concatenated_rank_text<'a, S: OutputElement + Sync + Send>(
    texts: impl IntoIterator<Item = &'a [u8]>,
    translation_table: &[u8; 256],
) -> (Vec<u8>, Vec<S>, Vec<usize>) {
    let texts: Vec<_> = texts.into_iter().collect();
    let num_texts = texts.len();

    let needed_capacity = texts.iter().map(|t| t.len()).sum::<usize>() + num_texts;

    let sentinel_indices: Vec<_> = texts
        .iter()
        .scan(0, |state, t| {
            let temp = *state + t.len();
            *state += t.len() + 1;
            Some(temp)
        })
        .collect();

    let mut concatenated_text = vec![0; needed_capacity];
    let mut concatenated_text_splits = Vec::with_capacity(num_texts);
    let mut remaining_slice = concatenated_text.as_mut_slice();

    for t in texts.iter() {
        let (this, remaining) = remaining_slice.split_at_mut(t.len() + 1);
        concatenated_text_splits.push(this);
        remaining_slice = remaining;
    }

    let mut frequency_table = texts
        .into_par_iter()
        .zip(concatenated_text_splits)
        .map(|(text, concatenated_text_split)| {
            let mut frequency_table = vec![S::zero(); 256];

            for (source, target) in text.iter().zip(concatenated_text_split) {
                *target = translation_table[*source as usize];
                frequency_table[*target as usize] = frequency_table[*target as usize] + S::one();
            }

            frequency_table
        })
        .reduce_with(merge_frequency_tables)
        .expect("There should be at least one texts");

    frequency_table[0] = <S as NumCast>::from(num_texts).unwrap();

    (concatenated_text, frequency_table, sentinel_indices)
}

fn merge_frequency_tables<S: OutputElement>(mut f1: Vec<S>, f2: Vec<S>) -> Vec<S> {
    for (x1, x2) in f1.iter_mut().zip(f2) {
        *x1 = *x1 + x2;
    }

    f1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_text() {
        let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];
        let (text, frequency_table, sentinel_indices) =
            create_concatenated_rank_text::<i32>(texts, &ASCII_DNA_TRANSLATION_TABLE);

        assert_eq!(
            text,
            [
                2, 2, 2, 1, 1, 1, 3, 3, 3, 4, 4, 4, 0, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 0
            ]
        );

        assert_eq!(&sentinel_indices, &[12, 25]);

        let mut expected_frequency_table = vec![0; 256];
        expected_frequency_table[0] = 2;
        expected_frequency_table[1] = 6;
        expected_frequency_table[2] = 6;
        expected_frequency_table[3] = 6;
        expected_frequency_table[4] = 6;

        assert_eq!(expected_frequency_table, frequency_table);
    }
}
