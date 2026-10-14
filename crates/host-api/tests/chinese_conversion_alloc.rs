//! 预热后的共享繁简转换只申请输出缓冲，不为每个查找端点申请堆内存。
#![allow(unsafe_code)]

#[path = "support/allocations.rs"]
mod allocation_counter;

use allocation_counter::{allocation_requests, allocations};
use msime_client_core::chinese_conversion::{
    simplified_to_traditional, traditional_to_simplified_characters,
};

#[test]
fn chinese_conversion_does_not_allocate_for_each_prefix() {
    assert!(allocations(|| drop(std::hint::black_box(Vec::<u8>::with_capacity(8)))) > 0);
    // 字典和逆表首次构建不属于稳态转换预算。
    drop(simplified_to_traditional("汉语输入法"));
    drop(traditional_to_simplified_characters("漢語輸入法"));
    let ascii = "synthetic-001 ".repeat(256);
    let cases = [
        ("", "", ""),
        ("synthetic-001", "synthetic-001", "synthetic-001"),
        ("汉语输入法", "漢語輸入法", "汉语输入法"),
        ("发展头发", "發展頭髮", "发展头发"),
        ("兀", "兀", "兀"),
        ("a漢😀𠀀かな", "a漢😀𠀀かな", "a汉😀𠀀かな"),
        ("⿰发发", "⿰发发", "⿰发发"),
        (ascii.as_str(), ascii.as_str(), ascii.as_str()),
    ];
    let mut measurements = Vec::new();
    for (input, traditional, simplified) in cases {
        for (direction, convert, expected) in [
            (
                "s2t",
                simplified_to_traditional as fn(&str) -> String,
                traditional,
            ),
            (
                "t2s-char",
                traditional_to_simplified_characters as fn(&str) -> String,
                simplified,
            ),
        ] {
            let mut output = String::new();
            let measured = allocation_requests(|| output = convert(std::hint::black_box(input)));
            assert_eq!(output, expected, "direction={direction}");
            eprintln!(
                "direction={direction} characters={} allocations={} requested_bytes={}",
                input.chars().count(),
                measured.0,
                measured.1
            );
            measurements.push((direction, input.chars().count(), measured));
        }
    }
    // 所选文字不会使输出扩容；规范化和最终输出各预留一次，空串无需申请。
    assert!(
        measurements
            .iter()
            .all(|(_, characters, measured)| measured.0 <= if *characters == 0 { 0 } else { 2 }),
        "字符数增长不应增加临时端点申请: {measurements:?}"
    );
}
