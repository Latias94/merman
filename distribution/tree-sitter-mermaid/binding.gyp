{
  "targets": [
    {
      "target_name": "tree_sitter_mermaid_binding",
      "dependencies": [
        "<!(node -p \"require('node-addon-api').targets\"):node_addon_api_except",
        "tree_sitter_mermaid_parser"
      ],
      "include_dirs": [
        "src"
      ],
      "sources": [
        "bindings/node/binding.cc"
      ]
    },
    {
      "target_name": "tree_sitter_mermaid_parser",
      "type": "static_library",
      "include_dirs": [
        "src"
      ],
      "sources": [
        "src/parser.c",
        "src/scanner.c"
      ],
      "conditions": [
        ["OS!='win'", {
          "cflags_c": [
            "-std=c11"
          ]
        }, { # OS == "win"
          "msvs_settings": {
            "VCCLCompilerTool": {
              # Node's common.gypi adds C++20 to every MSVC target. Remove it
              # from this C-only library before selecting the C11 standard.
              "AdditionalOptions!": [
                "-std:c++20"
              ],
              "LanguageStandard_C": "stdc11",
              "AdditionalOptions": [
                "/utf-8"
              ]
            }
          }
        }],
        ["OS=='mac'", {
          "xcode_settings": {
            "GCC_C_LANGUAGE_STANDARD": "c11",
            "MACOSX_DEPLOYMENT_TARGET": "10.7"
          }
        }]
      ]
    }
  ]
}
