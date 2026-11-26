#![allow(missing_docs)]
use crate::model::checksum::{validate_value, ChecksumError};

#[test]
fn out_of_range_algo_idx_clamps_to_adler() {
    assert_eq!(validate_value(7, "024d0127").unwrap(), "024d0127");
    assert_eq!(
        validate_value(42, "abcd"),
        Err(ChecksumError::BadLength {
            expected: 8,
            got: 4
        })
    );
}

#[test]
fn display_messages_verbatim() {
    assert_eq!(
        ChecksumError::NotHex.to_string(),
        "校验码只能是十六进制字符（0-9a-f）"
    );
    assert_eq!(
        ChecksumError::BadLength {
            expected: 64,
            got: 4
        }
        .to_string(),
        "校验码需为 64 位十六进制（当前 4 位）"
    );
}
