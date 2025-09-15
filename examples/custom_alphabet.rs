use genedex::Alphabet;

fn main() {
    // This example shows how to create a custom alphabet, either with or without ambiguous symbols.
    // The size of the alphabets is 1 larger than the number of symbols, because a special text-delimiter/sentinel
    // symbol is always implicitly included.

    let digits = Alphabet::from_io_symbols(b"0123456789", 0);
    assert_eq!(digits.size(), 11);
    assert_eq!(digits.num_searchable_symbols(), 10);

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
