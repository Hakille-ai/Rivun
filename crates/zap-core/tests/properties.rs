use proptest::prelude::*;
use zap_core::{HEADER_LEN, ZapFrame, ZapHeader};

proptest! {
    #[test]
    fn header_parser_never_panics(input in proptest::collection::vec(any::<u8>(), 0..160)) {
        let _ = ZapHeader::parse(&input);
    }

    #[test]
    fn frame_decoder_never_panics(input in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = ZapFrame::decode(&input);
    }

    #[test]
    fn truncated_headers_are_rejected(input in proptest::collection::vec(any::<u8>(), 0..HEADER_LEN)) {
        prop_assert!(ZapHeader::parse(&input).is_err());
    }
}
