//! 加固测试：`checksum`（src/model/checksum.rs 变异幸存体杀灭）。
//! 靶点（v117 复测轮 checksum.rs 幸存 3）：
//! - `56:76` 下标钳位 `len() - 1` 的 `-` → `+` / `-` → `/`：越界算法下标必须
//!   钳到表尾 Adler-32（idx ≥ 7 不 panic 且按 8 位十六进制口径校验）；
//! - `41:9` `Display for ChecksumError` 整体替换为 `Ok(Default::default())`：
//!   两种错误变体的用户文案必须逐字保真（`to_string` 非空且与定稿一致）。
//!
//! 挂载：checksum.rs 内嵌 tests 引用 `crate::model::testenv`，
//! 经根 `model` 挂载壳解析（tests/hardening/model/mod.rs 收编）。

#![allow(missing_docs)]
use crate::model::checksum::{validate_value, ChecksumError};

/// 靶 `56:76`（`-`→`+` 与 `-`→`/`）：越界下标钳位。
/// 原版 `min(len-1=6)` 把 idx=7 钳到 6（Adler-32，期望 8 位）；
/// mutant `len+1=8` / `len/1=7` 使 idx=7/42 越界 panic。
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

/// 靶 `41:9`：`Display` 整体替换为 `Ok(Default::default())`（`to_string` 变空）。
/// 两种变体文案逐字保真：NotHex 定稿文案；BadLength 含 expected/got 数值插值。
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
