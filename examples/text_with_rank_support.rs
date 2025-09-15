use genedex::TextWithRankSupport;

fn main() {
    // This example shows how to directly use the TextWithRankSupport data structure that powers the FM-Index
    // of this library. The data structure assumes that the input is a single text that is already transformed into
    // the dense representation of symbols.

    let text = vec![0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];
    let text_with_rank_support = TextWithRankSupport::<i32>::construct(&text, 4);
    drop(text);

    let idx = 4;
    let symbol = 1;

    // characters from the text can easily be recovered
    assert_eq!(text_with_rank_support.symbol_at(idx), symbol);

    // In this library, the rank of a symbol is defined as the number of occurrences of
    // the symbol in the part of the input text before the given index. This part can be
    // denoted as [0, idx) or text[..idx]
    assert_eq!(text_with_rank_support.rank(symbol, idx), 1);
}
