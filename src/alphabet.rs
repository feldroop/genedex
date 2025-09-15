use std::{borrow::Borrow, collections::HashSet};

/// For now, only alphabets with a sentinel (encoded as the zero byte) are allowed
/// num_symbols_not_searcheable does NOT include sentinel
/// max size 255
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[derive(Debug, Clone)]
pub struct Alphabet {
    io_to_dense_representation_table: Vec<u8>,
    dense_to_io_representation_table: Vec<u8>,
    num_symbols_not_searcheable: usize,
}

impl Alphabet {
    pub fn from_io_symbols<S>(
        symbols: impl IntoIterator<Item = S>,
        num_symbols_not_searcheable: usize,
    ) -> Self
    where
        S: Borrow<u8>,
    {
        let dense_to_io_representation_table: Vec<_> =
            symbols.into_iter().map(|s| *s.borrow()).collect();
        let symbols_set: HashSet<_> = dense_to_io_representation_table.iter().copied().collect();

        assert!(
            dense_to_io_representation_table.len() == symbols_set.len(),
            "Symbols of the alphabet must be unique."
        );

        assert!(
            dense_to_io_representation_table.len() <= 255,
            "Alphabet size can be at most 255 (to leave space for the sentinel)."
        );

        let mut io_to_dense_representation_table = vec![0; 256];

        for (i, &symbol) in dense_to_io_representation_table.iter().enumerate() {
            io_to_dense_representation_table[symbol as usize] = (i + 1) as u8;
        }

        Self {
            io_to_dense_representation_table,
            dense_to_io_representation_table,
            num_symbols_not_searcheable,
        }
    }

    /// First symbol per groups of ambiguous symbols is canonical IO representation
    pub fn from_ambiguous_io_symbols<S, I>(
        symbols: impl IntoIterator<Item = I>,
        num_symbols_not_searcheable: usize,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Borrow<u8>,
    {
        let symbol_groups: Vec<Vec<_>> = symbols
            .into_iter()
            .map(|group| group.into_iter().map(|s| *s.borrow()).collect())
            .collect();

        let dense_to_io_representation_table: Vec<_> = symbol_groups
            .iter()
            .map(|group| {
                assert!(
                    !group.is_empty(),
                    "Every group of symbols must contain at least one symbol"
                );
                group[0]
            })
            .collect();

        let symbols_set: HashSet<_> = symbol_groups.iter().flatten().copied().collect();
        let num_symbols_total: usize = symbol_groups.iter().map(|group| group.len()).sum();

        assert!(
            num_symbols_total == symbols_set.len(),
            "Symbols of the alphabet must be unique."
        );

        assert!(
            dense_to_io_representation_table.len() <= 255,
            "Alphabet size can be at most 255 (to leave space for the sentinel)."
        );

        let mut io_to_dense_representation_table = vec![0; 256];

        for (i, group) in symbol_groups.iter().enumerate() {
            for &symbol in group.iter() {
                io_to_dense_representation_table[symbol as usize] = (i + 1) as u8;
            }
        }

        Alphabet::new(
            io_to_dense_representation_table,
            dense_to_io_representation_table,
            num_symbols_not_searcheable,
        )
    }

    fn new(
        io_to_dense_representation_table: Vec<u8>,
        dense_to_io_representation_table: Vec<u8>,
        num_symbols_not_searcheable: usize,
    ) -> Self {
        let size = dense_to_io_representation_table.len() + 1;

        assert!(
            size > 1,
            "Alphabet size must be at least 2 (including sentinel)"
        );

        assert!(
            size <= 256,
            "Alphabet size can be at most 256 (including sentinel)"
        );

        let dense_encoded_symbols_without_sentinel: HashSet<_> = io_to_dense_representation_table
            .iter()
            .filter(|&&s| s != 0)
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

        if symbol == 0 { None } else { Some(symbol) }
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

/// Includes only the four bases of DNA A,C,G and T (case-insensitive).
pub fn ascii_dna() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols([b"Aa", b"Cc", b"Gg", b"Tt"], 0)
}
/// Includes the four bases of DNA A,C,G and T, and the N character (case-insensitive). The N character is not allowed to be searched.
pub fn ascii_dna_with_n() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols([b"Aa", b"Cc", b"Gg", b"Tt", b"Nn"], 1)
}

/// Includes all values of the IUPAC standard (or .fasta format) for DNA bases, except for gaps (case-insensitive).
pub fn ascii_dna_iupac() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols(
        [
            b"Aa", b"Cc", b"Gg", b"Tt", b"Nn", b"Rr", b"Yy", b"Kk", b"Mm", b"Ss", b"Ww", b"Bb",
            b"Dd", b"Hh", b"Vv",
        ],
        0,
    )
}

/// Functionally equivalent to the DNA with N alphabet, but allows other IUPAC DNA
/// characters as input, which are converted to N (case-insensitive). The N character is not allowed to be searched.
pub fn ascii_dna_iupac_as_dna_with_n() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols(
        [
            b"Aa".as_slice(),
            b"Cc",
            b"Gg",
            b"Tt",
            b"NnRrYyKkMmSsWwBbDdHhVv",
        ],
        1,
    )
}

/// Includes only values that correspond to single amino acids (case-insensitive).
pub fn ascii_amino_acid() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols(
        [
            b"Aa", b"Cc", b"Dd", b"Ee", b"Ff", b"Gg", b"Hh", b"Ii", b"Kk", b"Ll", b"Mm", b"Nn",
            b"Oo", b"Pp", b"Qq", b"Rr", b"Ss", b"Tt", b"Uu", b"Vv", b"Ww", b"Yy",
        ],
        0,
    )
}

/// Includes all values of the IUPAC standard (or .fasta format) for amino acids, except for gaps (case-insensitive).
pub fn ascii_amino_acid_iupac() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols(
        [
            b"Aa".as_slice(),
            b"Bb",
            b"Cc",
            b"Dd",
            b"Ee",
            b"Ff",
            b"Gg",
            b"Hh",
            b"Ii",
            b"Jj",
            b"Kk",
            b"Ll",
            b"Mm",
            b"Nn",
            b"Oo",
            b"Pp",
            b"Qq",
            b"Rr",
            b"Ss",
            b"Tt",
            b"Uu",
            b"Vv",
            b"Ww",
            b"Xx",
            b"Yy",
            b"Zz",
            b"*",
        ],
        0,
    )
}

/// Includes all u8 values until the `max_symbol` value.
pub fn u8_until(max_symbol: u8) -> Alphabet {
    Alphabet::from_io_symbols(0..=max_symbol, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_digits_alphabet() {
        let digits = Alphabet::from_io_symbols(b"0123456789", 0);
        assert_eq!(digits.size(), 11);
        assert_eq!(digits.num_searchable_symbols(), 10);
    }

    #[test]
    fn custom_iso_basic_latin_alphabet() {
        let roman = Alphabet::from_ambiguous_io_symbols(
            [
                b"Aa", b"Bb", b"Cc", b"Dd", b"Ee", b"Ff", b"Gg", b"Hh", b"Ii", b"Jj", b"Kk", b"Ll",
                b"Mm", b"Nn", b"Oo", b"Pp", b"Qq", b"Rr", b"Ss", b"Tt", b"Uu", b"Vv", b"Ww", b"Xx",
                b"Yy", b"Zz",
            ],
            0,
        );
        assert_eq!(roman.size(), 27);
        assert_eq!(roman.num_searchable_symbols(), 26);
    }

    #[test]
    fn construct_alphabets() {
        let dna = ascii_dna();
        assert_eq!(dna.size(), 5);
        assert_eq!(dna.num_searchable_symbols(), 4);

        let dna_n = ascii_dna_with_n();
        assert_eq!(dna_n.size(), 6);
        assert_eq!(dna_n.num_searchable_symbols(), 4);

        let dna_iupac = ascii_dna_iupac();
        assert_eq!(dna_iupac.size(), 16);
        assert_eq!(dna_iupac.num_searchable_symbols(), 15);

        let dna_iupac_as_n = ascii_dna_iupac_as_dna_with_n();
        assert_eq!(dna_iupac_as_n.size(), 6);
        assert_eq!(dna_iupac_as_n.num_searchable_symbols(), 4);

        let aa = ascii_amino_acid();
        assert_eq!(aa.size(), 23);
        assert_eq!(aa.num_searchable_symbols(), 22);

        let aa_iupac = ascii_amino_acid_iupac();
        assert_eq!(aa_iupac.size(), 28);
        assert_eq!(aa_iupac.num_searchable_symbols(), 27);

        for max_symbol in 1..=254 {
            let alph = u8_until(max_symbol);
            assert_eq!(alph.size(), max_symbol as usize + 2);
            assert_eq!(alph.num_searchable_symbols(), max_symbol as usize + 1);
        }
    }
}
