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
        ("x86_64-unknown-linux-gnu", "sha256:770c4e120ce6977a9a6405ad09a26fc2e9a58f26a58eea4d2219a9df97616428"),
    ),
    "rust-svg-basic": (
        ("x86_64-unknown-linux-gnu", "sha256:07072ffc30a5df01638d868e64b301c53aa5b0453ee5b2897a66cd30536ac96b"),
    ),
    "cli-analysis": (
        ("aarch64-apple-darwin", "sha256:9066e7ca12db7c2383bff0cfcca880b74986c0f773ea6c36d9bbcf4e347f7749"),
        ("x86_64-apple-darwin", "sha256:9066e7ca12db7c2383bff0cfcca880b74986c0f773ea6c36d9bbcf4e347f7749"),
        ("x86_64-pc-windows-msvc", "sha256:64e6d4b3abfe5c675c3b129895e824d8112b73cc0893c5601a9c1be1d1aa7099"),
        ("x86_64-unknown-linux-gnu", "sha256:9066e7ca12db7c2383bff0cfcca880b74986c0f773ea6c36d9bbcf4e347f7749"),
    ),
    "rust-export-jpeg": (
        ("x86_64-unknown-linux-gnu", "sha256:0b61d81d4517023814bd944bf96794e4c8afc518c51ed2c7411faf84ba89f2fe"),
    ),
    "rust-export-png": (
        ("x86_64-unknown-linux-gnu", "sha256:a7b8aa5a4110be079bcc5d20819d1cf4125ef8e8374e9d4642eb76ff0e8af962"),
    ),
    "rust-export-pdf": (
        ("x86_64-unknown-linux-gnu", "sha256:723ee0ba4d20a3aa9955519ac47c6727c0efc600ec649f7d1e48f65be38fdd7c"),
    ),
    "android-native": (
        ("aarch64-linux-android", "sha256:f841efb991a9746b7977d4dd9d931c1568d8e8eca674ab1c119663da896814c3"),
        ("x86_64-linux-android", "sha256:f841efb991a9746b7977d4dd9d931c1568d8e8eca674ab1c119663da896814c3"),
    ),
    "apple-uniffi-native": (
        ("aarch64-apple-darwin", "sha256:5e928a4e9b9d3ca328af59de510f56f64b1d54692e45345d0fd32e646c87c41a"),
        ("aarch64-apple-ios", "sha256:2f28ccf25835e3bb541b8b7dfed6fb822b159dd53ba2f80733389a4c8de417e6"),
        ("aarch64-apple-ios-sim", "sha256:2f28ccf25835e3bb541b8b7dfed6fb822b159dd53ba2f80733389a4c8de417e6"),
        ("x86_64-apple-darwin", "sha256:5e928a4e9b9d3ca328af59de510f56f64b1d54692e45345d0fd32e646c87c41a"),
        ("x86_64-apple-ios", "sha256:2f28ccf25835e3bb541b8b7dfed6fb822b159dd53ba2f80733389a4c8de417e6"),
    ),
    "c-abi-native": (
        ("x86_64-unknown-linux-gnu", "sha256:ace9a83b2845e7132be2e205e0dee6d0d1a327696ce096fe6812634ab689b775"),
    ),
    "cli-release": (
        ("aarch64-apple-darwin", "sha256:98b93d0dbe6ad17a3f2410db7fab1e7a842d08729412d19c70f8ab51bc36218f"),
        ("x86_64-apple-darwin", "sha256:03183e37875754d7042dec42d03de404a66b469cab832f601bd4a682686834e9"),
        ("x86_64-pc-windows-msvc", "sha256:57452d6d387b0286e514cb7a96f711c6627d1d14974f21be0105566422cc7c96"),
        ("x86_64-unknown-linux-gnu", "sha256:1a54169e4ba32dfeb4b8de8e77baa08268d18f1723c1b72db404c4ebf932b955"),
    ),
    "flutter-android-native": (
        ("aarch64-linux-android", "sha256:552d8170b4d4b33b79ee3d01ab7879d3483dd6f56c2bc813367766a233c5294d"),
        ("x86_64-linux-android", "sha256:552d8170b4d4b33b79ee3d01ab7879d3483dd6f56c2bc813367766a233c5294d"),
    ),
    "flutter-desktop-native": (
        ("aarch64-apple-darwin", "sha256:552d8170b4d4b33b79ee3d01ab7879d3483dd6f56c2bc813367766a233c5294d"),
        ("aarch64-unknown-linux-gnu", "sha256:ace9a83b2845e7132be2e205e0dee6d0d1a327696ce096fe6812634ab689b775"),
        ("x86_64-apple-darwin", "sha256:552d8170b4d4b33b79ee3d01ab7879d3483dd6f56c2bc813367766a233c5294d"),
        ("x86_64-pc-windows-gnu", "sha256:aa97a4770c832a33645e9ead8f4ba0f6453a3e468687eaf26c96e354aa74a328"),
        ("x86_64-unknown-linux-gnu", "sha256:ace9a83b2845e7132be2e205e0dee6d0d1a327696ce096fe6812634ab689b775"),
    ),
    "flutter-ios-native": (
        ("aarch64-apple-ios", "sha256:ace9a83b2845e7132be2e205e0dee6d0d1a327696ce096fe6812634ab689b775"),
        ("aarch64-apple-ios-sim", "sha256:ace9a83b2845e7132be2e205e0dee6d0d1a327696ce096fe6812634ab689b775"),
        ("x86_64-apple-ios", "sha256:ace9a83b2845e7132be2e205e0dee6d0d1a327696ce096fe6812634ab689b775"),
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
        ("aarch64-apple-darwin", "sha256:5e928a4e9b9d3ca328af59de510f56f64b1d54692e45345d0fd32e646c87c41a"),
        ("x86_64-pc-windows-msvc", "sha256:8960e85b08e2ee79dfe2587c6400776c7746b951295b3ec643b3f9e48f875da4"),
        ("x86_64-unknown-linux-gnu", "sha256:5d0f3556c99b130c44e3753f9cff02d81cfc92ba9ca11517f90d3777c91afdde"),
    ),
    "rust-all": (
        ("x86_64-unknown-linux-gnu", "sha256:bacd48500284f347287c8698a31790e341d40a17e4b7b775a1fbf49070872732"),
    ),
    "rust-analysis": (
        ("x86_64-unknown-linux-gnu", "sha256:249a0125e84aa3d4fca2777d6572505b0f6ed03a06610037feac5abc23c44095"),
    ),
    "rust-ascii": (
        ("x86_64-unknown-linux-gnu", "sha256:9c4b5ccc17973b08325e553836a354fb62357183eeb384cc3b34708a098f5cf4"),
    ),
    "rust-bindings-core-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:fa39a3819c66fd8d75808d9d9a1e5507c7b124bbd92807baf74b7fb8d13bb1ff"),
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
        ("x86_64-unknown-linux-gnu", "sha256:2532b7a5af2d27bb54acbc937fad3af0178692a203d2ea1110d77095ec8d3297"),
    ),
    "rust-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:edb1825caa712da4f53fee4a619b98c0d1d5e6f30bb54a3022e3615ed63c270b"),
    ),
    "rust-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:2d0b569edc94a9346d8a0624771a0868f5d8391652adfe98e0c8ba89db22a839"),
    ),
    "rust-render-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:ab074b39fa6164769cd97d7e0bf8ebbc077aedb631907fab65aab8d4fb43a4e2"),
    ),
    "rustdoc-static-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:e48cc86dafeabc1781d4481b263a1d92f56a014c9e43adaee2c74e6915f0d675"),
    ),
    "typst-wasm": (
        ("wasm32-unknown-unknown", "sha256:4818c5dd604bba0b32b05579f613c7098977483febd2cb66fd1b913a7fa0887e"),
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
        ("wasm32-unknown-unknown", "sha256:585c7087f5b6d1f0183d7bd9f163b4d3c838f168efc5ed429e2614174ceed35c"),
    ),
    "web-render": (
        ("wasm32-unknown-unknown", "sha256:a6891b5296fb6bcf686b2fb981ec552f2a9b264f4736f8a983fe41b17d136613"),
    ),
}
