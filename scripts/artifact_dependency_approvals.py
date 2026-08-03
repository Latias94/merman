"""Approved runtime dependency fingerprints for exact artifact profiles.

Each profile maps to its ordered evidence targets. Host profiles use the explicit
Linux reference target; target-set profiles list every descriptor-owned target.
"""

from __future__ import annotations

from collections.abc import Mapping
from typing import TypeAlias


HOST_CLOSURE_REFERENCE_TARGET = "x86_64-unknown-linux-gnu"
TargetFingerprint: TypeAlias = tuple[str, str]
ApprovalCatalog: TypeAlias = Mapping[str, tuple[TargetFingerprint, ...]]


ARTIFACT_DEPENDENCY_APPROVALS: ApprovalCatalog = {
    "rust-static-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:a22602b2f9f453464581f7ce688bac2ac30f64b38f26ba2db145e6fafaa5200e"),
    ),
    "rust-svg-basic": (
        ("x86_64-unknown-linux-gnu", "sha256:24b31b4d1cac71537e1679d86d080dc184bf357c73a36bd61c5049bec54166bc"),
    ),
    "cli-analysis": (
        ("aarch64-apple-darwin", "sha256:f93696009cc1483745b01fc1486e8de18d79cae08fc4ac1b97b96637f2977050"),
        ("x86_64-apple-darwin", "sha256:f93696009cc1483745b01fc1486e8de18d79cae08fc4ac1b97b96637f2977050"),
        ("x86_64-pc-windows-msvc", "sha256:8ee0852f1c3823319169bf67ebecf93ed7e82ec0409a0ce7b1d37b35ad137420"),
        ("x86_64-unknown-linux-gnu", "sha256:f93696009cc1483745b01fc1486e8de18d79cae08fc4ac1b97b96637f2977050"),
    ),
    "rust-export-jpeg": (
        ("x86_64-unknown-linux-gnu", "sha256:699ea589ae008b497d0b53940065148eaeef212c073de74621f24db7ef46affa"),
    ),
    "rust-export-png": (
        ("x86_64-unknown-linux-gnu", "sha256:e69d33e18b93c5b98cfef7bca75aa13f45ada5e00d063a8c39a034629e5877d1"),
    ),
    "rust-export-pdf": (
        ("x86_64-unknown-linux-gnu", "sha256:6937a477b7e62f1b20de077b747d1dcfdd072c3a2b4e23bea6236aa4f72ae733"),
    ),
    "android-native": (
        ("aarch64-linux-android", "sha256:293db7e2b4c14ee728bf8d3efbf6eff0312c73b4bbd5ef9c8f74f50927e5b060"),
        ("x86_64-linux-android", "sha256:293db7e2b4c14ee728bf8d3efbf6eff0312c73b4bbd5ef9c8f74f50927e5b060"),
    ),
    "apple-uniffi-native": (
        ("aarch64-apple-darwin", "sha256:f50c4de31fb00ad282b25f6b56921b67aa716e569230bcee37ed629f1577381a"),
        ("aarch64-apple-ios", "sha256:00b20e51a1cfeeafad57e017b6f8923aef0fb802bc73995210548c91fdbf60ab"),
        ("aarch64-apple-ios-sim", "sha256:00b20e51a1cfeeafad57e017b6f8923aef0fb802bc73995210548c91fdbf60ab"),
        ("x86_64-apple-darwin", "sha256:f50c4de31fb00ad282b25f6b56921b67aa716e569230bcee37ed629f1577381a"),
        ("x86_64-apple-ios", "sha256:00b20e51a1cfeeafad57e017b6f8923aef0fb802bc73995210548c91fdbf60ab"),
    ),
    "c-abi-native": (
        ("x86_64-unknown-linux-gnu", "sha256:ee4da624355363958a45392ac7efaf068e1eb45374a336e3a49c5dfc08cc0656"),
    ),
    "cli-release": (
        ("aarch64-apple-darwin", "sha256:d1c08c6b517d6698e7ff8d808383e8ad51ee686fdeec27c60bbdb6da9ef7c306"),
        ("x86_64-apple-darwin", "sha256:d1c08c6b517d6698e7ff8d808383e8ad51ee686fdeec27c60bbdb6da9ef7c306"),
        ("x86_64-pc-windows-msvc", "sha256:349df3da190f9702f1b460cec762bd5e08dc50758a62798651c670fca907b37a"),
        ("x86_64-unknown-linux-gnu", "sha256:57811cb5feb726684017426419b6652b021a884b6b5cbbbbf27da268cd2f8848"),
    ),
    "flutter-android-native": (
        ("aarch64-linux-android", "sha256:fb669adc03f82e87522ee8d072dbb97d54e3e7511d9fefc4db7f326e36b6ddb0"),
        ("x86_64-linux-android", "sha256:fb669adc03f82e87522ee8d072dbb97d54e3e7511d9fefc4db7f326e36b6ddb0"),
    ),
    "flutter-desktop-native": (
        ("aarch64-apple-darwin", "sha256:fb669adc03f82e87522ee8d072dbb97d54e3e7511d9fefc4db7f326e36b6ddb0"),
        ("aarch64-unknown-linux-gnu", "sha256:ee4da624355363958a45392ac7efaf068e1eb45374a336e3a49c5dfc08cc0656"),
        ("x86_64-apple-darwin", "sha256:fb669adc03f82e87522ee8d072dbb97d54e3e7511d9fefc4db7f326e36b6ddb0"),
        ("x86_64-pc-windows-gnu", "sha256:5c3e934fb3765a40685a576697074a76d71f1880cc180dec22e7c6e8e544e1ca"),
        ("x86_64-unknown-linux-gnu", "sha256:ee4da624355363958a45392ac7efaf068e1eb45374a336e3a49c5dfc08cc0656"),
    ),
    "flutter-ios-native": (
        ("aarch64-apple-ios", "sha256:ee4da624355363958a45392ac7efaf068e1eb45374a336e3a49c5dfc08cc0656"),
        ("aarch64-apple-ios-sim", "sha256:ee4da624355363958a45392ac7efaf068e1eb45374a336e3a49c5dfc08cc0656"),
        ("x86_64-apple-ios", "sha256:ee4da624355363958a45392ac7efaf068e1eb45374a336e3a49c5dfc08cc0656"),
    ),
    "lsp-library": (
        ("x86_64-unknown-linux-gnu", "sha256:b163eae03b3c4f0da68eae1147069ba3ffd2e2a8f497c285f57df4dfda07245c"),
    ),
    "lsp-stdio-release": (
        ("aarch64-apple-darwin", "sha256:72229c3d60b4e156682b2f6906b625b648caaa2fa927ec9ce4a6a6cf2b866b33"),
        ("x86_64-apple-darwin", "sha256:72229c3d60b4e156682b2f6906b625b648caaa2fa927ec9ce4a6a6cf2b866b33"),
        ("x86_64-pc-windows-msvc", "sha256:adc3ce664961858a3d436a2eaa1e66fca691d733ee695289af00d8224f05a52c"),
        ("x86_64-unknown-linux-gnu", "sha256:72229c3d60b4e156682b2f6906b625b648caaa2fa927ec9ce4a6a6cf2b866b33"),
    ),
    "python-uniffi-native": (
        ("aarch64-apple-darwin", "sha256:f50c4de31fb00ad282b25f6b56921b67aa716e569230bcee37ed629f1577381a"),
        ("x86_64-pc-windows-msvc", "sha256:6a9b859395a6b72821d43ee05b19a99922f2b806db97e4eccfd539cf1f3d406b"),
        ("x86_64-unknown-linux-gnu", "sha256:edde976310d3deddf9f40b62ab1bad2e36404d5f841351a27f16b145b83c68f5"),
    ),
    "rust-all": (
        ("x86_64-unknown-linux-gnu", "sha256:2eabab1f9ff9884330bf471e32db57e4a3f076ac2083f61f89701686c53208cc"),
    ),
    "rust-analysis": (
        ("x86_64-unknown-linux-gnu", "sha256:19a2696403ffd792c4edff306c1925d8d408dc4c09842c0b5b5c593b6e5000ed"),
    ),
    "rust-ascii": (
        ("x86_64-unknown-linux-gnu", "sha256:6b2e894de674b33f9caaa9244b6e9632ef1f344cc894e33fc5a7dcd571ed0b44"),
    ),
    "rust-bindings-core-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:4b10493b24f9d4f0fa2d8810e5340790e1da034fe6fd104a3ee7e166bb856298"),
    ),
    "rust-core": (
        ("x86_64-unknown-linux-gnu", "sha256:23a0ca4053049ff0f4c0de3cee5dd01cb14586faad41a58899e412cb6e977b84"),
    ),
    "rust-editor-core": (
        ("x86_64-unknown-linux-gnu", "sha256:0489a4e99d5272d1c63e2d65e51949a82a8d51fe189dc53cfbf8db544378a543"),
    ),
    "rust-editor-facade": (
        ("x86_64-unknown-linux-gnu", "sha256:d8f4c10fef2253e1a321a4b96fb7d72b728fc461e8348efa3f4a6e864ffe5d93"),
    ),
    "rust-export-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:a9842873f83f2b64c23ede3828cccb3599be7eb3cd5a002abaa3955aafd8fc6f"),
    ),
    "rust-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:08d6695a7092ef9e27cafd99e55610c9bae2ebe5607bad60b1c54f49d94787b9"),
    ),
    "rust-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:ee5847b2de051fddd6e0831bda40a41de456da1e137fe0872b9b403ff7d591c9"),
    ),
    "rust-render-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:58d82031954c86f3ab92c33c87171edbdef3ca4ea438a893f81e28cdcdfdf98e"),
    ),
    "rustdoc-static-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:0d97b23ada1d65665900a987e5f15ad40b7d987e3ca14dee9f58c344ba152b50"),
    ),
    "typst-wasm": (
        ("wasm32-unknown-unknown", "sha256:c2524c2cfd0cda13a406ace9e72e2ea641940fb417725e91a9ad6b4d1fd766c4"),
    ),
    "web-analysis": (
        ("wasm32-unknown-unknown", "sha256:0a4e24e09c0feabb79c42504dcbae4bdc70f1d08e66ab0ab3f80d724968e9b76"),
    ),
    "web-ascii": (
        ("wasm32-unknown-unknown", "sha256:056278a5cce62e911e198f77dd5cd1165c379416185da89036fdda87723a97c7"),
    ),
    "web-editor": (
        ("wasm32-unknown-unknown", "sha256:49ff47a2bfc0b4ac865f7bd0a61983b536afc1918a7d304e6ba1eb5c60789b11"),
    ),
    "web-full": (
        ("wasm32-unknown-unknown", "sha256:a11b38ef3e2f209b2fbc91d190a8c8cfbb37ed900c8ea73f39e73cd2fe0edf2a"),
    ),
    "web-render": (
        ("wasm32-unknown-unknown", "sha256:cbb45bf671f9d92e02db77cdb8cd251d8b2991e0d347a05de90c46732d34d834"),
    ),
}
