use std::collections::HashSet;

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

/// For now, only alphabets with a sentinel b'\0' are allowed
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[derive(Debug, Clone)]
pub struct Alphabet {
    io_to_dense_representation_table: Vec<u8>,
    dense_to_io_representation_table: Vec<u8>,
    num_symbols_not_searcheable: usize,
}

impl Alphabet {
    /// num_symbols_not_searcheable does NOT include sentinel
    pub fn new(
        io_to_dense_representation_table: &[u8; 256],
        dense_to_io_representation_table: &[u8],
        num_symbols_not_searcheable: usize,
    ) -> Self {
        let io_to_dense_representation_table = io_to_dense_representation_table.to_vec();
        let dense_to_io_representation_table = dense_to_io_representation_table.to_vec();

        let size = dense_to_io_representation_table.len() + 1;

        assert!(
            size > 1,
            "Alphabet size must be at least 2 (including sentinel)"
        );

        assert!(
            size <= 255,
            "Alphabet size can be at most 255 (including sentinel)"
        );

        assert!(
            io_to_dense_representation_table
                .iter()
                .find(|&&s| s == 0)
                .is_none(),
            "No symbol in io representation is allowed to have 0 as dense representation."
        );

        let dense_encoded_symbols_without_sentinel: HashSet<_> = io_to_dense_representation_table
            .iter()
            .filter(|&&s| (s as usize) < size)
            .collect();

        let num_dense_symbols = dense_encoded_symbols_without_sentinel.len() + 1;

        assert!(
            num_dense_symbols == size,
            "The alphabet translation tables are invalid. Alphabet size is {}, but there are {} symbols in the dense encoding (both wihtout sentinel).",
            size - 1,
            num_dense_symbols - 1
        );

        assert!(
            num_symbols_not_searcheable + 2 <= size,
            "Invalid alphabet. there must be at least one searchable symbol."
        );

        Self {
            io_to_dense_representation_table,
            dense_to_io_representation_table,
            num_symbols_not_searcheable,
        }
    }

    pub fn io_to_dense_representation(&self, symbol: u8) -> u8 {
        self.try_io_to_dense_representation(symbol)
            .expect("symbol in io representation is valid")
    }

    pub fn try_io_to_dense_representation(&self, symbol: u8) -> Option<u8> {
        let symbol = self.io_to_dense_representation_table[symbol as usize];

        if (symbol as usize) < self.size() {
            Some(symbol)
        } else {
            None
        }
    }

    pub fn dense_to_io_representation(&self, symbol: u8) -> u8 {
        self.try_dense_to_io_representation(symbol)
            .expect("symbol in dense representation is valid")
    }

    pub fn try_dense_to_io_representation(&self, symbol: u8) -> Option<u8> {
        if symbol == 0 {
            None
        } else {
            self.dense_to_io_representation_table
                .get(symbol as usize - 1)
                .copied()
        }
    }

    pub fn size(&self) -> usize {
        self.dense_to_io_representation_table.len() + 1
    }

    pub fn num_searchable_symbols(&self) -> usize {
        self.size() - self.num_symbols_not_searcheable - 1
    }

    pub fn contains_sentinel(&self) -> bool {
        true
    }
}

pub fn ascii_dna() -> Alphabet {
    Alphabet::new(&ASCII_DNA_TRANSLATION_TABLE, b"ACGT", 0)
}

pub fn ascii_dna_with_n() -> Alphabet {
    Alphabet::new(&ASCII_DNA_N_TRANSLATION_TABLE, b"ACGTN", 1)
}

pub fn ascii_dna_iupac() -> Alphabet {
    Alphabet::new(&ASCII_DNA_IUPAC_TRANSLATION_TABLE, b"ACGTNRYKMSWBDHV", 0)
}

pub fn ascii_dna_iupac_as_dna_with_n() -> Alphabet {
    Alphabet::new(&ASCII_DNA_IUPAC_AS_DNA_N_TRANSLATION_TABLE, b"ACGTN", 1)
}

pub fn u8_until(max_symbol: u8) -> Alphabet {
    let io_to_dense_representation_table: Vec<_> =
        (0u8..=255).map(|s| s.saturating_add(1)).collect();
    let dense_to_io_representation_table: Vec<_> = (0..=max_symbol).collect();

    Alphabet::new(
        io_to_dense_representation_table
            .as_slice()
            .try_into()
            .unwrap(),
        &dense_to_io_representation_table,
        0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_alphabets() {
        let _ = ascii_dna();
        let _ = ascii_dna_with_n();
        let _ = ascii_dna_iupac();
        let _ = ascii_dna_iupac_as_dna_with_n();

        for max_symbol in 1..=254 {
            let _ = u8_until(max_symbol);
        }
    }
}
