// There might be values at the end of the alphabet that are never searched (like N)
// sentinel 0 is NOT allowed to be a defined value for the alphabet
pub trait Alphabet {
    const DENSE_ENCODING_TRANSLATION_TABLE: [u8; 256];
    const SIZE: usize;
    const NUM_SYMBOL_NOT_SEARCHED: usize;
}

pub(crate) const ASCII_DNA_TRANSLATION_TABLE: [u8; 256] = {
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

pub(crate) const ASCII_DNA_N_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = ASCII_DNA_TRANSLATION_TABLE;
    table[b'N' as usize] = 5;
    table[b'n' as usize] = 5;

    table
};

pub(crate) const ASCII_DNA_IUPAC_TRANSLATION_TABLE: [u8; 256] = {
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

pub(crate) const ASCII_DNA_IUPAC_AS_DNA_TRANSLATION_TABLE: [u8; 256] = {
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

pub(crate) const ASCII_DNA_IUPAC_AS_DNA_N_TRANSLATION_TABLE: [u8; 256] = {
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

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
pub struct AsciiDna {}

impl Alphabet for AsciiDna {
    const DENSE_ENCODING_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_TRANSLATION_TABLE;
    const SIZE: usize = 5;
    const NUM_SYMBOL_NOT_SEARCHED: usize = 0;
}

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
pub struct AsciiDnaWithN {}

impl Alphabet for AsciiDnaWithN {
    const DENSE_ENCODING_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_N_TRANSLATION_TABLE;
    const SIZE: usize = 6;
    const NUM_SYMBOL_NOT_SEARCHED: usize = 1;
}

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
pub struct AsciiDnaIupac {}

impl Alphabet for AsciiDnaIupac {
    const DENSE_ENCODING_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_IUPAC_TRANSLATION_TABLE;
    const SIZE: usize = 16;
    const NUM_SYMBOL_NOT_SEARCHED: usize = 0;
}

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
pub struct AsciiDnaIupacAsDna {}

impl Alphabet for AsciiDnaIupacAsDna {
    const DENSE_ENCODING_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_IUPAC_AS_DNA_TRANSLATION_TABLE;
    const SIZE: usize = 5;
    const NUM_SYMBOL_NOT_SEARCHED: usize = 0;
}

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
pub struct AsciiDnaIupacAsDnaWithN {}

impl Alphabet for AsciiDnaIupacAsDnaWithN {
    const DENSE_ENCODING_TRANSLATION_TABLE: [u8; 256] = ASCII_DNA_IUPAC_AS_DNA_N_TRANSLATION_TABLE;
    const SIZE: usize = 6;
    const NUM_SYMBOL_NOT_SEARCHED: usize = 1;
}
