pub const ASCII_DNA_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = [255; 256];

    table[b'A' as usize] = 0;
    table[b'a' as usize] = 0;

    table[b'C' as usize] = 1;
    table[b'c' as usize] = 1;

    table[b'G' as usize] = 2;
    table[b'g' as usize] = 2;

    table[b'T' as usize] = 3;
    table[b't' as usize] = 3;

    table
};

pub const ASCII_DNA_N_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = ASCII_DNA_TRANSLATION_TABLE;
    table[b'N' as usize] = 4;
    table[b'n' as usize] = 4;

    table
};

pub const ASCII_DNA_WITH_SENTINEL_TRANSLATION_TABLE: [u8; 256] = {
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

pub const ASCII_DNA_N_WITH_SENTINEL_TRANSLATION_TABLE: [u8; 256] = {
    let mut table = ASCII_DNA_WITH_SENTINEL_TRANSLATION_TABLE;
    table[b'N' as usize] = 5;
    table[b'n' as usize] = 5;

    table
};

pub fn transfrom_into_ranks_inplace(
    text: &mut [u8],
    translation_table: &[u8; 256],
) -> Result<(), usize> {
    for (i, c) in text.iter_mut().enumerate() {
        *c = translation_table[*c as usize];

        if *c == 255 {
            return Err(i);
        }
    }

    Ok(())
}

pub fn iter_ranks<'a>(
    text: &[u8],
    translation_table: &'a [u8; 256],
) -> impl ExactSizeIterator<Item = &'a u8> + DoubleEndedIterator {
    text.iter().map(|&c| {
        let rank = &translation_table[c as usize];

        if *rank == 255 {
            panic!("encountered invalid character in iter_ranks");
        } else {
            rank
        }
    })
}
