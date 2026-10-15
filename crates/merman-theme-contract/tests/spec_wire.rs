use merman_theme_contract::{
    CanonicalJsonErrorKind, DiagramThemeSpecWireV1, SpecifiedWireV1, ThemeEffectEntryWireV1,
    ThemeEffectPrimitiveWireV1, ThemeRuleSetWireV1, resolve_authoring_version,
};

const ALL_SECTIONS_JSON: &str = r##"
{
  "mermaid": {
    "theme": "base",
    "dark_mode": true,
    "variables": {
      "primaryColor": "#2563eb",
      "fontSize": 16,
      "useMaxWidth": false
    }
  },
  "typography": {
    "default": {
      "font_stack": ["Inter", "sans-serif"],
      "font_size_px": 14,
      "font_weight": 500,
      "font_style": "italic",
      "line_height": 1.5,
      "letter_spacing_px": 0.25,
      "word_spacing_px": 0.5,
      "transform": "uppercase",
      "decoration": "underline",
      "text_align": "center",
      "white_space": "pre-wrap",
      "wrap": "break-word"
    },
    "families": {
      "flowchart": { "font_size_px": 15, "line_height": { "px": 20 } }
    }
  },
  "styles": [
    {
      "kind": "rule",
      "target": "node",
      "family": "flowchart",
      "variant": "primary",
      "ordinal": { "cycle": { "period": 3, "offset": 1 } },
      "style": {
        "fill": {
          "kind": "radial-gradient",
          "center_x": { "percent": 50 },
          "center_y": { "percent": 40 },
          "radius": { "px": 24 },
          "stops": [
            { "offset": 0, "color": "#ffffff" },
            { "offset": 1, "color": "#2563eb" }
          ],
          "repetition": { "kind": "tiled", "width_px": 48, "height_px": 32 }
        },
        "opacity": 0.5,
        "fill_opacity": 0.75,
        "stroke": {
          "paint": "#0f172a",
          "width": 2,
          "dasharray": [4, 2],
          "linecap": "round",
          "linejoin": "bevel",
          "opacity": 0.75
        },
        "radius": 6,
        "padding": { "top": 1, "right": 2, "bottom": 3, "left": 4 },
        "typography": {
          "font_stack": ["Inter"],
          "font_size_px": 13,
          "font_weight": 600,
          "font_style": "normal",
          "line_height": { "px": 18 },
          "letter_spacing_px": 0.25,
          "word_spacing_px": 0.5,
          "transform": "none",
          "decoration": "none",
          "text_align": "start",
          "white_space": "normal",
          "wrap": "normal"
        },
        "effect": "node-shadow"
      }
    },
    {
      "kind": "ordinal-palette",
      "target": "chart-series",
      "colors": ["#2563eb", "#16a34a"]
    }
  ],
  "canvas": {
    "base": {
      "kind": "linear-gradient",
      "angle_degrees": 135,
      "stops": [
        { "offset": 0, "color": "#0f172a" },
        { "offset": 1, "color": "#1e293b" }
      ],
      "repetition": { "kind": "repeating", "period_px": 16 }
    },
    "layers": [
      {
        "paint": {
          "kind": "pattern",
          "pattern": "grid",
          "cell_width": 12,
          "cell_height": 12,
          "foreground": "#ffffff22",
          "background": "#00000000",
          "angle_degrees": 45
        },
        "opacity": 0.5,
        "blend_mode": "multiply",
        "offset_x": 2,
        "offset_y": 3
      }
    ],
    "bleed": { "top": 1, "right": 2, "bottom": 3, "left": 4 }
  },
  "effects": [
    {
      "kind": "graph",
      "id": "node-shadow",
      "primitives": [
        {
          "kind": "drop-shadow",
          "input": "source-graphic",
          "offset_x": 2,
          "offset_y": 3,
          "blur_radius": 4,
          "spread": 1,
          "color": "#00000080"
        },
        { "kind": "gaussian-blur", "input": "previous", "std_deviation": 1.5 },
        {
          "kind": "color-matrix",
          "input": "previous",
          "values": [1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0]
        },
        {
          "kind": "turbulence",
          "input": "previous",
          "base_frequency_x": 0.125,
          "base_frequency_y": 0.25,
          "octaves": 2,
          "seed": 7
        },
        {
          "kind": "displacement",
          "input": "previous",
          "map_input": "source-graphic",
          "scale": 8
        }
      ]
    },
    { "kind": "binding", "target": "node", "effect_id": "node-shadow" }
  ],
  "requirements": {
    "capabilities": ["semantic-rules", "svg-filter"],
    "text_capabilities": ["catalog-binding"]
  },
  "assets": {
    "fonts": [
      { "id": "inter-regular", "format": "woff2", "data_base64": "Zg==" }
    ],
    "aliases": [{ "alias": "Inter UI", "target": "Inter" }],
    "generic_families": [{ "generic": "sans-serif", "target": "Inter" }],
    "available_sources": ["embedded", "system"],
    "embedding": "full-font"
  }
}
"##;

const ALL_SECTIONS_CANONICAL: &str = r##"{"assets":{"aliases":[{"alias":"Inter UI","target":"Inter"}],"available_sources":["embedded","system"],"embedding":"full-font","fonts":[{"data_base64":"Zg==","format":"woff2","id":"inter-regular"}],"generic_families":[{"generic":"sans-serif","target":"Inter"}]},"canvas":{"base":{"angle_degrees":135,"kind":"linear-gradient","repetition":{"kind":"repeating","period_px":16},"stops":[{"color":"#0f172a","offset":0},{"color":"#1e293b","offset":1}]},"bleed":{"bottom":3,"left":4,"right":2,"top":1},"layers":[{"blend_mode":"multiply","offset_x":2,"offset_y":3,"opacity":0.5,"paint":{"angle_degrees":45,"background":"#00000000","cell_height":12,"cell_width":12,"foreground":"#ffffff22","kind":"pattern","pattern":"grid"}}]},"effects":[{"id":"node-shadow","kind":"graph","primitives":[{"blur_radius":4,"color":"#00000080","input":"source-graphic","kind":"drop-shadow","offset_x":2,"offset_y":3,"spread":1},{"input":"previous","kind":"gaussian-blur","std_deviation":1.5},{"input":"previous","kind":"color-matrix","values":[1,0,0,0,0,0,1,0,0,0,0,0,1,0,0,0,0,0,1,0]},{"base_frequency_x":0.125,"base_frequency_y":0.25,"input":"previous","kind":"turbulence","octaves":2,"seed":7},{"input":"previous","kind":"displacement","map_input":"source-graphic","scale":8}]},{"effect_id":"node-shadow","kind":"binding","target":"node"}],"mermaid":{"dark_mode":true,"theme":"base","variables":{"fontSize":16,"primaryColor":"#2563eb","useMaxWidth":false}},"requirements":{"capabilities":["semantic-rules","svg-filter"],"text_capabilities":["catalog-binding"]},"styles":[{"family":"flowchart","kind":"rule","ordinal":{"cycle":{"offset":1,"period":3}},"style":{"effect":"node-shadow","fill":{"center_x":{"percent":50},"center_y":{"percent":40},"kind":"radial-gradient","radius":{"px":24},"repetition":{"height_px":32,"kind":"tiled","width_px":48},"stops":[{"color":"#ffffff","offset":0},{"color":"#2563eb","offset":1}]},"fill_opacity":0.75,"opacity":0.5,"padding":{"bottom":3,"left":4,"right":2,"top":1},"radius":6,"stroke":{"dasharray":[4,2],"linecap":"round","linejoin":"bevel","opacity":0.75,"paint":"#0f172a","width":2},"typography":{"decoration":"none","font_size_px":13,"font_stack":["Inter"],"font_style":"normal","font_weight":600,"letter_spacing_px":0.25,"line_height":{"px":18},"text_align":"start","transform":"none","white_space":"normal","word_spacing_px":0.5,"wrap":"normal"}},"target":"node","variant":"primary"},{"colors":["#2563eb","#16a34a"],"kind":"ordinal-palette","target":"chart-series"}],"typography":{"default":{"decoration":"underline","font_size_px":14,"font_stack":["Inter","sans-serif"],"font_style":"italic","font_weight":500,"letter_spacing_px":0.25,"line_height":1.5,"text_align":"center","transform":"uppercase","white_space":"pre-wrap","word_spacing_px":0.5,"wrap":"break-word"},"families":{"flowchart":{"font_size_px":15,"line_height":{"px":20}}}}}"##;

#[test]
fn complete_spec_wire_round_trips_every_section_and_has_known_canonical_bytes() {
    let spec: DiagramThemeSpecWireV1 =
        serde_json::from_str(ALL_SECTIONS_JSON).expect("closed complete spec should decode");

    let replay: DiagramThemeSpecWireV1 =
        serde_json::from_slice(&serde_json::to_vec(&spec).expect("complete spec should encode"))
            .expect("encoded complete spec should decode");
    assert_eq!(replay, spec);
    assert_eq!(
        spec.canonical_json_bytes()
            .expect("complete spec should canonicalize"),
        ALL_SECTIONS_CANONICAL.as_bytes()
    );
}

#[test]
fn complete_spec_wire_rejects_unknown_fields_and_non_clearable_nulls() {
    for invalid in [
        r#"{"unexpected":true}"#,
        r##"{"canvas":{"layers":[{"paint":"#fff","unexpected":true}]}}"##,
        r#"{"canvas":null}"#,
        r#"{"typography":{"default":{"font_stack":null}}}"#,
        r#"{"styles":[{"kind":"rule","target":"node","style":{"stroke":null}}]}"#,
        r#"{"styles":[{"kind":"rule","target":"node","style":{"typography":null}}]}"#,
    ] {
        assert!(
            serde_json::from_str::<DiagramThemeSpecWireV1>(invalid).is_err(),
            "closed complete-spec wire must reject {invalid}"
        );
    }
}

#[test]
fn complete_spec_wire_preserves_omission_empty_collections_and_style_clear() {
    let omitted: DiagramThemeSpecWireV1 =
        serde_json::from_str("{}").expect("omitted optional sections should decode");
    let empty: DiagramThemeSpecWireV1 = serde_json::from_str(r#"{"styles":[],"effects":[]}"#)
        .expect("explicit empty collections should decode");
    let clear: DiagramThemeSpecWireV1 = serde_json::from_str(
        r#"{"styles":[{"kind":"rule","target":"node","style":{"opacity":null,"stroke":{"paint":null},"typography":{"font_size_px":null}}}]}"#,
    )
    .expect("null on atomic style facets should mean clear");
    let ThemeRuleSetWireV1::Rule { style, .. } = &clear.styles.as_ref().unwrap()[0] else {
        panic!("expected rule wire");
    };
    assert!(matches!(style.opacity, SpecifiedWireV1::Clear));
    assert!(matches!(
        style.stroke.as_ref().unwrap().paint,
        SpecifiedWireV1::Clear
    ));
    assert!(matches!(
        style.typography.as_ref().unwrap().font_size_px,
        SpecifiedWireV1::Clear
    ));

    assert_eq!(omitted.canonical_json_bytes().unwrap(), br#"{}"#);
    assert_eq!(
        empty.canonical_json_bytes().unwrap(),
        br#"{"effects":[],"styles":[]}"#
    );
    assert_eq!(
        clear.canonical_json_bytes().unwrap(),
        br#"{"styles":[{"kind":"rule","style":{"opacity":null,"stroke":{"paint":null},"typography":{"font_size_px":null}},"target":"node"}]}"#
    );
}

#[test]
fn complete_spec_wire_rejects_deep_non_finite_values_in_serde_and_canonical_json() {
    let spec = DiagramThemeSpecWireV1 {
        effects: Some(vec![ThemeEffectEntryWireV1::Graph {
            id: "unsafe-filter".to_owned(),
            color_space: None,
            primitives: vec![ThemeEffectPrimitiveWireV1::ColorMatrix {
                input: Some("source-graphic".to_owned()),
                values: vec![0.0, f32::NAN],
            }],
        }]),
        ..DiagramThemeSpecWireV1::default()
    };

    assert!(serde_json::to_vec(&spec).is_err());
    assert_eq!(
        spec.canonical_json_bytes()
            .expect_err("canonical JSON must reject nested NaN")
            .kind(),
        CanonicalJsonErrorKind::InvalidInput
    );
}

#[test]
fn complete_spec_wire_version_is_owned_by_the_contract_registry() {
    let version = resolve_authoring_version(1, 1).expect("version 1 tuple should resolve");

    assert_eq!(
        DiagramThemeSpecWireV1::spec_schema_version(),
        version.spec_schema_version()
    );
    assert_eq!(DiagramThemeSpecWireV1::spec_schema_version(), 1);
}
