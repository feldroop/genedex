use libsais::OutputElement;

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

#[derive(Debug, Clone, Copy)]
pub struct Alphabet {
    pub(crate) u8_to_rank_translation_table: &'static [u8; 256],
    pub(crate) alphabet_size: usize,
}

pub static ASCII_DNA: Alphabet = Alphabet {
    u8_to_rank_translation_table: &ASCII_DNA_TRANSLATION_TABLE,
    alphabet_size: 4,
};

pub static ASCII_DNA_N: Alphabet = Alphabet {
    u8_to_rank_translation_table: &ASCII_DNA_N_TRANSLATION_TABLE,
    alphabet_size: 5,
};

pub static ASCII_DNA_IUPAC: Alphabet = Alphabet {
    u8_to_rank_translation_table: &ASCII_DNA_IUPAC_TRANSLATION_TABLE,
    alphabet_size: 16,
};

pub static ASCII_DNA_IUPAC_AS_DNA: Alphabet = Alphabet {
    u8_to_rank_translation_table: &ASCII_DNA_IUPAC_AS_DNA_TRANSLATION_TABLE,
    alphabet_size: 4,
};

pub static ASCII_DNA_IUPAC_AS_DNA_N: Alphabet = Alphabet {
    u8_to_rank_translation_table: &ASCII_DNA_IUPAC_AS_DNA_N_TRANSLATION_TABLE,
    alphabet_size: 5,
};

pub(crate) fn transfrom_into_ranks_inplace<O: OutputElement>(
    text: &mut [u8],
    translation_table: &[u8; 256],
    frequency_table: &mut [O],
) -> Result<(), usize> {
    for (i, c) in text.iter_mut().enumerate() {
        *c = translation_table[*c as usize];

        if *c == 255 {
            return Err(i);
        }

        frequency_table[*c as usize] = frequency_table[*c as usize] + O::one();
    }

    Ok(())
}

type TextAndMetadata<O> = (Vec<u8>, Vec<O>, Vec<usize>);

pub(crate) fn create_concatenated_rank_text<'a, O: OutputElement>(
    texts: impl IntoIterator<Item = &'a [u8]>,
    translation_table: &[u8; 256],
) -> Result<TextAndMetadata<O>, (usize, usize)> {
    let texts: Vec<_> = texts.into_iter().collect();
    let needed_capacity = texts.iter().map(|t| t.len()).sum::<usize>() + texts.len();

    let mut concatenated_text = Vec::with_capacity(needed_capacity);

    let mut frequency_table = vec![O::zero(); 256];

    let mut sentinel_indices = Vec::with_capacity(texts.len());

    for (i, text) in texts.into_iter().enumerate() {
        concatenated_text.extend_from_slice(text);
        let offset = concatenated_text.len() - text.len();

        transfrom_into_ranks_inplace(
            &mut concatenated_text[offset..],
            translation_table,
            &mut frequency_table,
        )
        .map_err(|text_idx| (i, text_idx))?;

        sentinel_indices.push(concatenated_text.len());
        concatenated_text.push(0);
        frequency_table[0] = frequency_table[0] + O::one();
    }

    Ok((concatenated_text, frequency_table, sentinel_indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_text() {
        let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];
        let (text, frequency_table, sentinel_indices) =
            create_concatenated_rank_text::<i32>(texts, &ASCII_DNA_TRANSLATION_TABLE).unwrap();

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
