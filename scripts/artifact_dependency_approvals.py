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
        ("x86_64-unknown-linux-gnu", "sha256:ac38cac2f1878909ed9284cf7abe9bcc7f92beee9671007d3d87ab76fc35b9a2"),
    ),
    "rust-svg-basic": (
        ("x86_64-unknown-linux-gnu", "sha256:898213c12cfb87f02b3168493fc4d2f3a2db07d3cb5f023dfb78cfbf1f3dc505"),
    ),
    "cli-analysis": (
        ("aarch64-apple-darwin", "sha256:2eb9b3e8fbcbdddd48ee67af25ca49dd206fc2c88a84c517946bd735249e3c35"),
        ("x86_64-apple-darwin", "sha256:2eb9b3e8fbcbdddd48ee67af25ca49dd206fc2c88a84c517946bd735249e3c35"),
        ("x86_64-pc-windows-msvc", "sha256:b4a7bcc4799e701d0afc8bc0bf28d3e37dc912a1bff368fc14dcd5d60169160d"),
        ("x86_64-unknown-linux-gnu", "sha256:2eb9b3e8fbcbdddd48ee67af25ca49dd206fc2c88a84c517946bd735249e3c35"),
    ),
    "rust-export-jpeg": (
        ("x86_64-unknown-linux-gnu", "sha256:34212d9ba9845d536753421e3b2aa1a33e91788d58c7a92ff81aafbbcb559190"),
    ),
    "rust-export-png": (
        ("x86_64-unknown-linux-gnu", "sha256:7d6cf4a267c2bb2473e2cea3cd6c65ad75ff2c2f8e20736c5cccb260b9bd8d68"),
    ),
    "rust-export-pdf": (
        ("x86_64-unknown-linux-gnu", "sha256:b3bac931ad06b25bd02f02df807a5f1c5d19a1647857cb2c9ea4ed4ce89c08bd"),
    ),
    "android-native": (
        ("aarch64-linux-android", "sha256:745c250a9e884b022e5b05495795ba6d2930bbdff382ef29ab69dcd6748df7e1"),
        ("x86_64-linux-android", "sha256:745c250a9e884b022e5b05495795ba6d2930bbdff382ef29ab69dcd6748df7e1"),
    ),
    "apple-uniffi-native": (
        ("aarch64-apple-darwin", "sha256:bdaaa83ce3f4c52f440914a2da0c92f12b6e703345875e27a26c6e7ceda205ed"),
        ("aarch64-apple-ios", "sha256:be57d627db057712680d0214b42311a77d340d7ef39d5753acd795dbeef90054"),
        ("aarch64-apple-ios-sim", "sha256:be57d627db057712680d0214b42311a77d340d7ef39d5753acd795dbeef90054"),
        ("x86_64-apple-darwin", "sha256:bdaaa83ce3f4c52f440914a2da0c92f12b6e703345875e27a26c6e7ceda205ed"),
        ("x86_64-apple-ios", "sha256:be57d627db057712680d0214b42311a77d340d7ef39d5753acd795dbeef90054"),
    ),
    "c-abi-native": (
        ("x86_64-unknown-linux-gnu", "sha256:981190b346d31517f14b04361cc790fb4c5379ceeaeaf19c6b52dbfe3592348f"),
    ),
    "cli-release": (
        ("aarch64-apple-darwin", "sha256:4fd343ef2dbf76fbffb381f15aa62f9bfccee856a8eca4dbe7ce1a441a6c5c74"),
        ("x86_64-apple-darwin", "sha256:4fd343ef2dbf76fbffb381f15aa62f9bfccee856a8eca4dbe7ce1a441a6c5c74"),
        ("x86_64-pc-windows-msvc", "sha256:cbfa7fe696d1d758a31478316a4b82ad4dd095ec75e796be1cec3dfaf2946c51"),
        ("x86_64-unknown-linux-gnu", "sha256:6e2e702ba0d363b35403fba710783280782d4fabdfb52e599be4794f8ec90a86"),
    ),
    "flutter-android-native": (
        ("aarch64-linux-android", "sha256:538bdc983a5d6ca069ffaa452e0ae2ba704efa519041160de47f4ef5e7377d67"),
        ("x86_64-linux-android", "sha256:538bdc983a5d6ca069ffaa452e0ae2ba704efa519041160de47f4ef5e7377d67"),
    ),
    "flutter-desktop-native": (
        ("aarch64-apple-darwin", "sha256:538bdc983a5d6ca069ffaa452e0ae2ba704efa519041160de47f4ef5e7377d67"),
        ("aarch64-unknown-linux-gnu", "sha256:981190b346d31517f14b04361cc790fb4c5379ceeaeaf19c6b52dbfe3592348f"),
        ("x86_64-apple-darwin", "sha256:538bdc983a5d6ca069ffaa452e0ae2ba704efa519041160de47f4ef5e7377d67"),
        ("x86_64-pc-windows-gnu", "sha256:7fef9a5bb9d84b4968ad77c9416c9bc68c05da2953b4ef643b4226cd9f6b45f6"),
        ("x86_64-unknown-linux-gnu", "sha256:981190b346d31517f14b04361cc790fb4c5379ceeaeaf19c6b52dbfe3592348f"),
    ),
    "flutter-ios-native": (
        ("aarch64-apple-ios", "sha256:981190b346d31517f14b04361cc790fb4c5379ceeaeaf19c6b52dbfe3592348f"),
        ("aarch64-apple-ios-sim", "sha256:981190b346d31517f14b04361cc790fb4c5379ceeaeaf19c6b52dbfe3592348f"),
        ("x86_64-apple-ios", "sha256:981190b346d31517f14b04361cc790fb4c5379ceeaeaf19c6b52dbfe3592348f"),
    ),
    "lsp-library": (
        ("x86_64-unknown-linux-gnu", "sha256:51e3157571580cb7fad910503beb1a0c523abc508cd777de5e5974a6bdd46383"),
    ),
    "lsp-stdio-release": (
        ("aarch64-apple-darwin", "sha256:8b97358dad20973a6d4c5b269b77cba6f4a230eff61641fb379338028c077426"),
        ("x86_64-apple-darwin", "sha256:8b97358dad20973a6d4c5b269b77cba6f4a230eff61641fb379338028c077426"),
        ("x86_64-pc-windows-msvc", "sha256:cb475db8d0b0c9758e12478c2e7e81d00baf54a201146b060c1e0766eb6b722b"),
        ("x86_64-unknown-linux-gnu", "sha256:8b97358dad20973a6d4c5b269b77cba6f4a230eff61641fb379338028c077426"),
    ),
    "python-uniffi-native": (
        ("aarch64-apple-darwin", "sha256:bdaaa83ce3f4c52f440914a2da0c92f12b6e703345875e27a26c6e7ceda205ed"),
        ("x86_64-pc-windows-msvc", "sha256:a1456957adf72737320044119d82007534bd6e052aca53b91e749cb14f1fe4bc"),
        ("x86_64-unknown-linux-gnu", "sha256:bb7eb42b4c692eca865291eb33a0ca12b07ed94abbf9a5792cf958eda00772e2"),
    ),
    "rust-all": (
        ("x86_64-unknown-linux-gnu", "sha256:840cb7611ea69c09c159e73a31a9f9d11ac534e88002243617bc85afc0635c54"),
    ),
    "rust-analysis": (
        ("x86_64-unknown-linux-gnu", "sha256:4f1a147f761d0d2884767490e2436dab57904523a317aa18299cf352cc773e2f"),
    ),
    "rust-ascii": (
        ("x86_64-unknown-linux-gnu", "sha256:301ee045ce2cffe9dd29b275ec28fcd22ac9bf0782ede6893d1eab7b3396b3e3"),
    ),
    "rust-bindings-core-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:a5771f17174ac3c2a74a8464ad314b82fa9436cb2efa6ca58e9666d2bb2768f7"),
    ),
    "rust-core": (
        ("x86_64-unknown-linux-gnu", "sha256:ed4bbe71cca6bb1955c7143665152c8db71f00b41b22d3bb69e1a159b7fc3e27"),
    ),
    "rust-editor-core": (
        ("x86_64-unknown-linux-gnu", "sha256:8a7dbaab0d7d6ecfd82357e269bb3953916a521d31aa82b23a1618cd40eddbba"),
    ),
    "rust-editor-facade": (
        ("x86_64-unknown-linux-gnu", "sha256:14f5509d2dffcf7c7f2b11e8ac86f313788af7befabaaf2edb63dee15a24d1bf"),
    ),
    "rust-export-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:fbf865e8cf4ec0771b99d2cd8b585edc164ebadeb5537ae5b4ea82807372fc64"),
    ),
    "rust-native-sdk": (
        ("x86_64-unknown-linux-gnu", "sha256:7ea95cbe1aa5b97274045fee60b87a0416e2975881598183be59d748a1825810"),
    ),
    "rust-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:7d5e7a1e4533ccc3ccf7f8eb4a45d0032ec6e3f587b33bf3e367b490b82484d3"),
    ),
    "rust-render-native-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:76ecb9fd46caf33f7546df61f7981717f92e6b4d3795686e114300f7ad60f92d"),
    ),
    "rustdoc-static-svg": (
        ("x86_64-unknown-linux-gnu", "sha256:a74c6a7961cf855a7904ec753f38628ffa8b908f820a1252b7de7d4b5bcffe14"),
    ),
    "typst-wasm": (
        ("wasm32-unknown-unknown", "sha256:5a2ccdfe3568d99efa3bb0f55463483f0e533918d342fc2f1aa11e55423573a8"),
    ),
    "web-analysis": (
        ("wasm32-unknown-unknown", "sha256:20bd93e07a4a94de6857949b8e32382b6cafcaf0631bdec0e05fe90795a23182"),
    ),
    "web-ascii": (
        ("wasm32-unknown-unknown", "sha256:662f054ff4510b3c245d48329783c4f0751c69eb3a191a9962f1ba9de4355a7f"),
    ),
    "web-editor": (
        ("wasm32-unknown-unknown", "sha256:7fe1c1fbe25147d03903fbfad094f18c0d8d22e74196cfd308be752de3bca7cd"),
    ),
    "web-full": (
        ("wasm32-unknown-unknown", "sha256:f6d5c71fe9c49ff2567535d29c5f5f1613787cd60d35cb7b40bfe8506a2c96c0"),
    ),
    "web-render": (
        ("wasm32-unknown-unknown", "sha256:fd3037f88adc8fd105fffff15238d858e9c5f07f7e9b6f76a7f4b2c9b26f88d7"),
    ),
}
