use std::{borrow::Borrow, collections::HashSet};

/// An alphabet that represents the set of valid symbols of a text.
///
/// In this library, symbols have two different representations, IO and dense.
///
/// The IO representation is typically the ASCII code of the symbol, such as `b'A'`, `b'b'`, etc..
/// Texts, queries and symbols that are inputs to FM-Index functions have to be supplied in their IO
/// representation.
///
/// The dense representation is used internally by the FM-Index. Symbols are represented by the numbers `0, ... , k-1` in memory,
/// where `k` is the number of symbols in the alphabet.
///
/// The dense representation of the alphabet currently always includes a special sentinel/text-delimiter symbol,
/// which does not have an IO representation. This is why the maximum number of symbols with IO representation
/// of alphabets is 255. This might change in the future.
///
/// Many commonly used alphabets are provided by this library and can be found [here](self). Examples of how to define a custom alphabet
/// can be found [here](https://github.com/feldroop/genedex/blob/master/examples/custom_alphabet.rs).
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[savefile_doc_hidden]
#[derive(Clone, PartialEq, Eq)]
pub struct Alphabet {
    io_to_dense_representation_table: Vec<u8>,
    dense_to_io_representation_table: Vec<u8>,
    num_io_symbols_not_searcheable: usize,
}

impl Alphabet {
    /// Construct an alphabet from symbols in IO representation.
    ///
    /// The last `num_io_symbols_not_searcheable` symbols from `symbols` are assumed to never be
    /// part of queries to the FM-Index. The sentinel is never searchable, but not included in this
    /// number.
    ///
    /// ```
    /// use genedex::Alphabet;
    /// let digits = Alphabet::from_io_symbols(b"0123456789", 0);
    /// assert_eq!(digits.num_dense_symbols(), 11);
    /// assert_eq!(digits.num_searchable_dense_symbols(), 10);
    /// ```
    pub fn from_io_symbols<S>(
        symbols: impl IntoIterator<Item = S>,
        num_io_symbols_not_searcheable: usize,
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

        Alphabet::new(
            io_to_dense_representation_table,
            dense_to_io_representation_table,
            num_io_symbols_not_searcheable,
        )
    }

    /// Construct an alphabet from ambiguous symbols in IO representation.
    ///
    /// This function allows defining an alphabet in which multiple symbols in IO representation map
    /// to the same symbol in dense representation. This is useful for expressing case-insensitivity, for example.
    ///
    /// The last `num_io_symbols_not_searcheable` symbols from `symbols` are assumed to never be
    /// part of queries to the FM-Index. The sentinel is never searchable, but not included in this
    /// number.
    ///
    /// ```
    /// use genedex::Alphabet;
    /// let roman = Alphabet::from_ambiguous_io_symbols(
    ///     [
    ///         b"Aa", b"Bb", b"Cc", b"Dd", b"Ee", b"Ff", b"Gg", b"Hh", b"Ii", b"Jj", b"Kk", b"Ll",
    ///         b"Mm", b"Nn", b"Oo", b"Pp", b"Qq", b"Rr", b"Ss", b"Tt", b"Uu", b"Vv", b"Ww", b"Xx",
    ///         b"Yy", b"Zz",
    ///     ],
    ///      0,
    /// );
    /// assert_eq!(roman.num_dense_symbols(), 27);
    /// assert_eq!(roman.num_searchable_dense_symbols(), 26);
    /// ```
    pub fn from_ambiguous_io_symbols<S, I>(
        symbols: impl IntoIterator<Item = I>,
        num_io_symbols_not_searcheable: usize,
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
            num_io_symbols_not_searcheable,
        )
    }

    fn new(
        io_to_dense_representation_table: Vec<u8>,
        dense_to_io_representation_table: Vec<u8>,
        num_io_symbols_not_searcheable: usize,
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
            num_io_symbols_not_searcheable + 2 <= size,
            "Invalid alphabet. there must be at least one searchable symbol."
        );

        Self {
            io_to_dense_representation_table,
            dense_to_io_representation_table,
            num_io_symbols_not_searcheable,
        }
    }

    /// Panics if `symbol` is not a valid symbol in IO representation of this alphabet.
    pub fn io_to_dense_representation(&self, symbol: u8) -> u8 {
        self.try_io_to_dense_representation(symbol)
            .expect("symbol in io representation should be valid")
    }

    pub fn try_io_to_dense_representation(&self, symbol: u8) -> Option<u8> {
        let symbol = self.io_to_dense_representation_table[symbol as usize];

        if symbol == 0 { None } else { Some(symbol) }
    }

    /// Panics if `symbol` is not a valid symbol in dense representation of this alphabet.
    pub fn dense_to_io_representation(&self, symbol: u8) -> u8 {
        self.try_dense_to_io_representation(symbol)
            .expect("symbol in dense representation should be valid")
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

    pub fn iter_io_symbols(&self) -> impl Iterator<Item = u8> {
        self.io_to_dense_representation_table
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(io_symbol, dense_symbol)| {
                if dense_symbol != 0 {
                    Some(io_symbol as u8)
                } else {
                    None
                }
            })
    }

    pub fn num_dense_symbols(&self) -> usize {
        self.dense_to_io_representation_table.len() + 1
    }

    pub fn num_searchable_dense_symbols(&self) -> usize {
        self.num_dense_symbols() - self.num_io_symbols_not_searcheable - 1
    }

    /// Currently always `true`. Might change in the future.
    pub fn contains_sentinel_in_dense_representation(&self) -> bool {
        true
    }
}

/// Includes only the four bases of DNA A, C, G and T (case-insensitive).
pub fn ascii_dna() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols([b"Aa", b"Cc", b"Gg", b"Tt"], 0)
}

/// Includes the four bases of DNA A, C, G and T, and the N character (case-insensitive). The N character is not allowed to be searched.
pub fn ascii_dna_with_n() -> Alphabet {
    Alphabet::from_ambiguous_io_symbols([b"Aa", b"Cc", b"Gg", b"Tt", b"Nn"], 1)
}

/// Includes all values of the IUPAC standard (or .fasta format) for DNA bases, except for gaps (case-insensitive).
///
/// All symbols are allowed to be searched, but the "degenerate" symbols are not resolved to match their base symbols.
/// For example, M means "A or C", but an M in the searched query does not match at an A or C of the indexed texts.
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

/// Includes only values that correspond to single amino acids in the IUPAC standard (case-insensitive).
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
/// This alphabet therefore contains all letters of the basic latin alphabet, and the symbol `*`.
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

/// Includes all u8 values until including the `max_symbol` value.
pub fn u8_until(max_symbol: u8) -> Alphabet {
    Alphabet::from_io_symbols(0..=max_symbol, 0)
}

/// Includes all 95 printable symbols of the ASCII code (case-sensitive).
pub fn ascii_printable() -> Alphabet {
    Alphabet::from_io_symbols(b" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~", 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundabout(alphabet: Alphabet) {
        let mut num_roundabouts = 0;
        for io_symbol in alphabet.iter_io_symbols() {
            // not always equal due to ambiguous chars
            if io_symbol
                == alphabet
                    .dense_to_io_representation(alphabet.io_to_dense_representation(io_symbol))
            {
                num_roundabouts += 1
            }
        }
        assert_eq!(num_roundabouts, alphabet.num_dense_symbols() - 1);
    }

    #[test]
    fn custom_digits_alphabet() {
        let digits = Alphabet::from_io_symbols(b"0123456789", 0);
        assert_eq!(digits.num_dense_symbols(), 11);
        assert_eq!(digits.num_searchable_dense_symbols(), 10);
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
        assert_eq!(roman.num_dense_symbols(), 27);
        assert_eq!(roman.num_searchable_dense_symbols(), 26);
    }

    #[test]
    fn construct_alphabets() {
        let dna = ascii_dna();
        assert_eq!(dna.num_dense_symbols(), 5);
        assert_eq!(dna.num_searchable_dense_symbols(), 4);
        roundabout(dna);

        let dna_n = ascii_dna_with_n();
        assert_eq!(dna_n.num_dense_symbols(), 6);
        assert_eq!(dna_n.num_searchable_dense_symbols(), 4);
        roundabout(dna_n);

        let dna_iupac = ascii_dna_iupac();
        assert_eq!(dna_iupac.num_dense_symbols(), 16);
        assert_eq!(dna_iupac.num_searchable_dense_symbols(), 15);
        roundabout(dna_iupac);

        let dna_iupac_as_n = ascii_dna_iupac_as_dna_with_n();
        assert_eq!(dna_iupac_as_n.num_dense_symbols(), 6);
        assert_eq!(dna_iupac_as_n.num_searchable_dense_symbols(), 4);
        roundabout(dna_iupac_as_n);

        let aa = ascii_amino_acid();
        assert_eq!(aa.num_dense_symbols(), 23);
        assert_eq!(aa.num_searchable_dense_symbols(), 22);
        roundabout(aa);

        let aa_iupac = ascii_amino_acid_iupac();
        assert_eq!(aa_iupac.num_dense_symbols(), 28);
        assert_eq!(aa_iupac.num_searchable_dense_symbols(), 27);
        roundabout(aa_iupac);

        let printable = ascii_printable();
        assert_eq!(printable.num_dense_symbols(), 96);
        assert_eq!(printable.num_searchable_dense_symbols(), 95);
        roundabout(printable);

        for max_symbol in 1..=254 {
            let alph = u8_until(max_symbol);
            assert_eq!(alph.num_dense_symbols(), max_symbol as usize + 2);
            assert_eq!(alph.num_searchable_dense_symbols(), max_symbol as usize + 1);
            roundabout(alph);
        }
    }
}
