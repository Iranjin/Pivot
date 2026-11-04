pub const PIVOT_LICENSE: &str = include_str!("../LICENSE");
pub const THIRD_PARTY_NOTICES: &str = include_str!("../THIRD_PARTY_LICENSES.md");

pub const ADDITIONAL_LICENSES: &[(&str, &str)] = &[
    (
        "Apache License 2.0",
        include_str!("../licenses/Apache-2.0.txt"),
    ),
    (
        "BSD 1-Clause License (assert_no_alloc)",
        include_str!("../licenses/BSD-1-Clause.txt"),
    ),
    (
        "SIL Open Font License 1.1 (Noto Emoji)",
        include_str!("../licenses/OFL-1.1.txt"),
    ),
    (
        "Ubuntu Font License 1.0",
        include_str!("../licenses/Ubuntu-font-1.0.txt"),
    ),
    (
        "Unicode License 3.0",
        include_str!("../licenses/Unicode-3.0.txt"),
    ),
    ("Hack Font License", include_str!("../licenses/Hack.txt")),
    (
        "Emoji Icon Font MIT License",
        include_str!("../licenses/emoji-icon-font-MIT.txt"),
    ),
];
