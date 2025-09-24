pub(crate) trait SliceCompression {
    fn get(idx: usize, slice: &[u8]) -> u8;

    fn set(idx: usize, slice: &mut [u8], value: u8);

    fn transform_chunk_size(chunk_size: usize) -> usize;

    fn transformed_slice_len(slice: &[u8]) -> usize;

    fn iter(slice: &[u8]) -> impl Iterator<Item = u8>;

    fn iter_zero_indices(slice: &[u8]) -> impl Iterator<Item = usize>;
}

pub(crate) struct NoSliceCompression {}

impl SliceCompression for NoSliceCompression {
    fn get(idx: usize, slice: &[u8]) -> u8 {
        slice[idx]
    }

    fn set(idx: usize, slice: &mut [u8], value: u8) {
        slice[idx] = value;
    }

    fn transform_chunk_size(chunk_size: usize) -> usize {
        chunk_size
    }

    fn transformed_slice_len(slice: &[u8]) -> usize {
        slice.len()
    }

    fn iter(slice: &[u8]) -> impl Iterator<Item = u8> {
        slice.iter().copied()
    }

    fn iter_zero_indices(slice: &[u8]) -> impl Iterator<Item = usize> {
        memchr::memchr_iter(0, slice)
    }
}

pub(crate) struct HalfBytesCompression {}

impl SliceCompression for HalfBytesCompression {
    fn get(idx: usize, slice: &[u8]) -> u8 {
        let byte = slice[idx / 2];

        if idx.is_multiple_of(2) {
            unpack_from_left_half_of_byte(byte)
        } else {
            unpack_from_right_half_of_byte(byte)
        }
    }

    fn set(idx: usize, slice: &mut [u8], value: u8) {
        let byte = &mut slice[idx / 2];

        if idx.is_multiple_of(2) {
            pack_into_left_half_of_byte(byte, value);
        } else {
            pack_into_right_half_of_byte(byte, value);
        }
    }

    fn transform_chunk_size(chunk_size: usize) -> usize {
        chunk_size / 2
    }

    fn transformed_slice_len(slice: &[u8]) -> usize {
        slice.len() * 2
    }

    fn iter(slice: &[u8]) -> impl Iterator<Item = u8> {
        slice.iter().flat_map(|&byte| {
            [
                unpack_from_left_half_of_byte(byte),
                unpack_from_right_half_of_byte(byte),
            ]
        })
    }

    fn iter_zero_indices(slice: &[u8]) -> impl Iterator<Item = usize> {
        Self::iter(slice)
            .enumerate()
            .filter_map(|(idx, byte)| if byte == 0 { Some(idx) } else { None })
    }
}

fn pack_into_left_half_of_byte(byte: &mut u8, value: u8) {
    *byte = (value << 4) | (*byte & 0b00001111);
}

fn pack_into_right_half_of_byte(byte: &mut u8, value: u8) {
    *byte = (*byte & 0b11110000) | (value & 0b00001111);
}

fn unpack_from_left_half_of_byte(byte: u8) -> u8 {
    byte >> 4
}

fn unpack_from_right_half_of_byte(byte: u8) -> u8 {
    byte & 0b00001111
}

pub(crate) fn half_byte_compress_text(text: &mut [u8]) {
    for i in 0..text.len() / 2 {
        let mut byte = 0;

        let left = text[i * 2];
        let right = text[i * 2 + 1];

        pack_into_left_half_of_byte(&mut byte, left);
        pack_into_right_half_of_byte(&mut byte, right);

        text[i] = byte;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_half_byte_compress_text() {
        let text = [
            0b00001101, 0b00000000, 0b00000110, 0b00000011, 0b00001111, 0b00000101,
        ];

        let mut text_copy = text;

        half_byte_compress_text(&mut text_copy);
        let compressed_text = &text_copy[..3];
        let expected_compressed_text = [0b11010000, 0b01100011, 0b11110101];

        assert_eq!(compressed_text, expected_compressed_text);

        for (idx, &expected_symbol) in text.iter().enumerate() {
            assert_eq!(
                expected_symbol,
                HalfBytesCompression::get(idx, compressed_text)
            );
        }

        let mut second_compressed_text = [0u8; 3];

        for (idx, &expected_symbol) in text.iter().enumerate() {
            HalfBytesCompression::set(idx, second_compressed_text.as_mut_slice(), expected_symbol);
        }
        assert_eq!(second_compressed_text, compressed_text);

        let collected: Vec<_> = HalfBytesCompression::iter(compressed_text).collect();
        assert_eq!(collected, text);

        let zero_indices: Vec<_> =
            HalfBytesCompression::iter_zero_indices(compressed_text).collect();
        assert_eq!(zero_indices, [1]);
    }
}
