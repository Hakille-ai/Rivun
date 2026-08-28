use proptest::prelude::*;
use rivun_core::{HEADER_LEN, RivunFrame, RivunHeader};

proptest! {
    #[test]
    fn header_parser_never_panics(input in proptest::collection::vec(any::<u8>(), 0..160)) {
        let _ = RivunHeader::parse(&input);
    }

    #[test]
    fn frame_decoder_never_panics(input in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _ = RivunFrame::decode(&input);
    }

    #[test]
    fn truncated_headers_are_rejected(input in proptest::collection::vec(any::<u8>(), 0..HEADER_LEN)) {
        prop_assert!(RivunHeader::parse(&input).is_err());
    }
}
