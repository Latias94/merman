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
        ("x86_64-unknown-linux-gnu", "sha256:f5a5a602a37f6e22229239f89f8f6233470ac2ae24a2a2fcf52ed9b8ce4d4750"),
    ),
    "rust-svg-basic": (
        ("x86_64-unknown-linux-gnu", "sha256:b53b13b6b3b0b75db19064d820121d0a26afa305f9fa63990b9383a48a086cff"),
    ),
    "cli-analysis": (
        ("aarch64-apple-darwin", "sha256:9066e7ca12db7c2383bff0cfcca880b74986c0f773ea6c36d9bbcf4e347f7749"),
        ("x86_64-apple-darwin", "sha256:9066e7ca12db7c2383bff0cfcca880b74986c0f773ea6c36d9bbcf4e347f7749"),
        ("x86_64-pc-windows-msvc", "sha256:12df6e0378f3052390bf53f72d81cfe14e6cf7ba1f5b8774be8946beee24adda"),
        ("x86_64-unknown-linux-gnu", "sha256:9066e7ca12db7c2383bff0cfcca880b74986c0f773ea6c36d9bbcf4e347f7749"),
    ),
    "rust-export-jpeg": (
        ("x86_64-unknown-linux-gnu", "sha256:e32d5377030beed91f8fe9c182ee4959522bd0637e9c2d4f0bf4d314e13161e1"),
    ),
    "rust-export-png": (
        ("x86_64-unknown-linux-gnu", "sha256:43ac5c3daa3c5ba941f5508873e6a70ed5358af5860d7db78322ada6e1bd1797"),
    ),
    "rust-export-pdf": (
        ("x86_64-unknown-linux-gnu", "sha256:4538a0286f67097450bd2960fe703aa560cb6c6f2df9c54a782aed5bc18beab3"),
    ),
    "android-native": (
        ("aarch64-linux-android", "sha256:74106a2254ad6abe66f5b8cc89ca31ae280863c82e12d6ef24c7557ac1a6dcb2"),
        ("x86_64-linux-android", "sha256:74106a2254ad6abe66f5b8cc89ca31ae280863c82e12d6ef24c7557ac1a6dcb2"),
    ),
    "apple-uniffi-native": (
        ("aarch64-apple-darwin", "sha256:84b49c5453bdb2a8df95f009b45e67e88b75b19b23268bb9e196dee4c9bf5d31"),
        ("aarch64-apple-ios", "sha256:162f024d24f11dc60170ee3e6a0dc92298aa2c98bf973e12b98000275841c455"),
        ("aarch64-apple-ios-sim", "sha256:162f024d24f11dc60170ee3e6a0dc92298aa2c98bf973e12b98000275841c455"),
        ("x86_64-apple-darwin", "sha256:84b49c5453bdb2a8df95f009b45e67e88b75b19b23268bb9e196dee4c9bf5d31"),
        ("x86_64-apple-ios", "sha256:162f024d24f11dc60170ee3e6a0dc92298aa2c98bf973e12b98000275841c455"),
    ),
    "c-abi-native": (
        ("x86_64-unknown-linux-gnu", "sha256:b0b8f4579d05a63ae969ccab4ec74ddc4b671a4e4fa4a48c920946bff68c859f"),
    ),
    "cli-release": (
        ("aarch64-apple-darwin", "sha256:7c7ae6d9c16b143172db59790fdcaf0932f9abaff1e8209b0e8ee22ca45fdc08"),
        ("x86_64-apple-darwin", "sha256:b3aa8603345462fa6140de8f2d9e8fa760152a48910bcdc6a2ce5cb5a7f0340b"),
        ("x86_64-pc-windows-msvc", "sha256:d459a0ebc0de59f8d3a1bbd5dcd3c7d737bb2658fac385ab055c31da6fe9a8de"),
        ("x86_64-unknown-linux-gnu", "sha256:fc09ffdfa240bd14f7ddb172781d2b7bfea4c020bd1b51ff989dbfa20dff6db8"),
    ),
    "flutter-android-native": (
        ("aarch64-linux-android", "sha256:49a04afab7932c59112876d61ab53cb62c8639a7a19f3644b7739d91c5205b9b"),
        ("x86_64-linux-android", "sha256:49a04afab7932c59112876d61ab53cb62c8639a7a19f3644b7739d91c5205b9b"),
    ),
    "flutter-desktop-native": (
        ("aarch64-apple-darwin", "sha256:49a04afab7932c59112876d61ab53cb62c8639a7a19f3644b7739d91c5205b9b"),
        ("aarch64-unknown-linux-gnu", "sha256:b0b8f4579d05a63ae969ccab4ec74ddc4b671a4e4fa4a48c920946bff68c859f"),
        ("x86_64-apple-darwin", "sha256:49a04afab7932c59112876d61ab53cb62c8639a7a19f3644b7739d91c5205b9b"),
        ("x86_64-pc-windows-gnu", "sha256:ab0e861a6683355ad5fcda31a8e0302db3eaf6bf58e092fc14ba5cc73bde26f6"),
        ("x86_64-unknown-linux-gnu", "sha256:b0b8f4579d05a63ae969ccab4ec74ddc4b671a4e4fa4a48c920946bff68c859f"),
    ),
    "flutter-ios-native": (
        ("aarch64-apple-ios", "sha256:b0b8f4579d05a63ae969ccab4ec74ddc4b671a4e4fa4a48c920946bff68c859f"),
        ("aarch64-apple-ios-sim", "sha256:b0b8f4579d05a63ae969ccab4ec74ddc4b671a4e4fa4a48c920946bff68c859f"),
        ("x86_64-apple-ios", "sha256:b0b8f4579d05a63ae969ccab4ec74ddc4b671a4e4fa4a48c920946bff68c859f"),
    ),
    "lsp-library": (
        ("x86_64-unknown-linux-gnu", "sha256:0f1cb8bfd1957738a9828570cd23b8e83370fe48ef6881b3548ad97a22f06890"),
    ),
    "lsp-stdio-release": (
        ("aarch64-apple-darwin", "sha256:7f75c706c825967d6787a68b2bbd065fda8180a2685ad85ebdf4fcf9f0a85a9a"),
        ("x86_64-apple-darwin", "sha256:7f75c706c825967d6787a68b2bbd065fda8180a2685ad85ebdf4fcf9f0a85a9a"),
        ("x86_64-pc-windows-msvc", "sha256:81d3f548f05edd1bc9e000e406ede9df0cd5f249958183bd61045a1bae1b03f4"),
        ("x86_64-unknown-linux-gnu", "sha256:7f75c706c825967d6787a68b2bbd065fda8180a2685ad85ebdf4fcf9f0a85a9a"),
    ),
    "python-uniffi-native": (
        ("aarch64-apple-darwin", "sha256:84b49c5453bdb2a8df95f009b45e67e88b75b19b23268bb9e196dee4c9bf5d31"),
        ("x86_64-pc-windows-msvc", "sha256:33d98255dd380a64e375717b0346e6ca7d2e491593bddcccd63a129127a3b68c"),
        ("x86_64-unknown-linux-gnu", "sha256:5d1f39820f7727275f9561462c3eda4208654cff23fe36a035eb81ca3bdcede3"),
    ),
    "rust-all": (
        ("x86_64-unknown-linux-gnu", "sha256:cde9cca4425c07132ca9277b384ddfbdb9a66a2e87092adf1f439179383ce9f8"),
    ),
    "rust-analysis": (
        ("x86_64-unknown-linux-gnu", "sha256:249a0125e84aa3d4fca2777d6572505b0f6ed03a06610037feac5abc23c44095"),
    ),
    "rust-ascii": (
        ("x86_64-unknown-linux-gnu", "sha256:9c4b5ccc17973b08325e553836a354fb62357183eeb384cc3b34708a098f5cf4"),
    ),
    "rust-bindings-core-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:73a4deea6c0e4dfc0c15a39169bbdd00eedabe32190e168b04cf75034dab95e2"),
    ),
    "rust-core": (
        ("x86_64-unknown-linux-gnu", "sha256:56db275bcefb63d7e6bbcd634584396df60a869125dc20401ca24fda9c4f9dac"),
    ),
    "rust-editor-core": (
        ("x86_64-unknown-linux-gnu", "sha256:4f115f1c0f573abef20da88c0688156f38b59acd6160bb5bfafb8ee45d1d77f2"),
    ),
    "rust-editor-facade": (
        ("x86_64-unknown-linux-gnu", "sha256:3dab48cf7c6b5bbfb5af789dd9f4c0aed2ce1152af0dd14822da023f10db97c5"),
    ),
    "rust-export-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:41fcb2af501cd68e0fb39dbc9c44147c2e1e32be80a01e54b775d22555a0a03e"),
    ),
    "rust-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:767f5d23048ca35522e812be682d264567b19ad0c8838d350eeed10fc0eb191f"),
    ),
    "rust-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:1d1ff50170d4af83772fcf7c6da0191676f6b8f771cac9bd5e08975dd7e0e70b"),
    ),
    "rust-render-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:048a5fbdff70712fea8ea3ba475cfef0a0a448fb9150e8cd7404a970de6b5c6f"),
    ),
    "rustdoc-static-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:5a7165a057183e78f673b15efedb5f25ab88044294946b815e9ceeaabdd86c0f"),
    ),
    "typst-wasm": (
        ("wasm32-unknown-unknown", "sha256:152fb771aafb93d14024220c27ec77845fb7363ff175f575d834bea14bbeedf1"),
    ),
    "web-analysis": (
        ("wasm32-unknown-unknown", "sha256:296ab2487e983b95b516dec8b5cc325afa1d8f7471c7d221bb7d8d2a11dc67f6"),
    ),
    "web-ascii": (
        ("wasm32-unknown-unknown", "sha256:5d4f9b4df09dc682e40a0deb3a35f4a7505758965f1b57b9ada0cf16e84e02c6"),
    ),
    "web-editor": (
        ("wasm32-unknown-unknown", "sha256:203cc97728c0b1a11574e137e8833d290bfb6c0e827aacdbc17238ba779e8ff9"),
    ),
    "web-full": (
        ("wasm32-unknown-unknown", "sha256:751137bbef5c51973aeb79b19cfcdbdd153ff643c1e7bf08ef6df87edc3459f6"),
    ),
    "web-render": (
        ("wasm32-unknown-unknown", "sha256:27794188a9d7664ca1ce6e31d1336e48ebb345f0d6008fff6aad30115db713e3"),
    ),
}
