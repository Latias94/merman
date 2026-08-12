#import "units.typ": canonical-css-px-string, css-px-number-string, typst-length-to-css-px

#let dictionary-or-none(value, name) = {
  if value == none {
    none
  } else if type(value) == dictionary {
    value
  } else {
    panic(name + " must be a dictionary")
  }
}

#let profile-field(profile, key, alt: none) = {
  let profile = dictionary-or-none(profile, "merman profile")
  if profile == none {
    none
  } else if key in profile {
    profile.at(key)
  } else if alt != none and alt in profile {
    profile.at(alt)
  } else {
    none
  }
}

#let choose-value(profile-value, direct-value, default: none) = {
  if direct-value != none {
    direct-value
  } else if profile-value != none {
    profile-value
  } else {
    default
  }
}

#let merge-dict(base, override, name) = {
  let base = dictionary-or-none(base, name)
  let override = dictionary-or-none(override, name)
  if base == none {
    override
  } else if override == none {
    base
  } else {
    (: ..base, ..override)
  }
}

#let validated-profile(profile) = {
  let profile = dictionary-or-none(profile, "merman profile")
  if profile == none {
    none
  } else {
    for key in ("presentation-profile", "presentation_profile") {
      if key in profile {
        panic("merman profile field `" + key + "` was removed; use `theme-preset` or `diagram-theme`")
      }
    }
    for key in ("host-theme", "host_theme") {
      if key in profile {
        panic("merman profile field `" + key + "` was removed; use `diagram-theme` or `typography`")
      }
    }
    if "theme" in profile {
      panic("merman profile field `theme` was renamed to `theme-variables`")
    }
    profile
  }
}

#let font-descriptor-name(font) = {
  if type(font) == str {
    font
  } else if type(font) == dictionary {
    if "name" not in font or type(font.at("name")) != str {
      panic("merman typography font descriptor must contain a string name")
    }
    font.at("name")
  } else {
    panic("merman typography font must contain only strings or font descriptors")
  }
}

#let font-stack-value(font) = {
  if font == none {
    none
  } else if type(font) == array {
    font.map(font-descriptor-name)
  } else {
    (font-descriptor-name(font),)
  }
}

#let font-size-value(size) = {
  if size == none {
    none
  } else if type(size) == str {
    let canonical = canonical-css-px-string(size, name: "merman typography size")
    float(canonical.slice(0, -2))
  } else if type(size) == length {
    typst-length-to-css-px(size, name: "merman typography size")
  } else if type(size) == int or type(size) == float {
    css-px-number-string(size, name: "merman typography size")
    size
  } else {
    panic("merman typography size must be a CSS px string, absolute Typst length, or pixel number")
  }
}

#let text-style-from-font(font-family, font-size) = {
  let stack = font-stack-value(font-family)
  let size = font-size-value(font-size)
  if stack == none and size == none {
    none
  } else {
    let out = if stack == none { (:) } else { (font_stack: stack) }
    if size == none { out } else { (: ..out, font_size_px: size) }
  }
}

#let context-text-style(font-family, font-size) = {
  text-style-from-font(font-family, font-size)
}

#let field3(dict, a, b, c) = {
  if dict == none {
    none
  } else if a in dict {
    dict.at(a)
  } else if b in dict {
    dict.at(b)
  } else if c in dict {
    dict.at(c)
  } else {
    none
  }
}

#let typography-text-style(typography) = {
  let typography = dictionary-or-none(typography, "merman typography")
  if typography == none {
    none
  } else {
    let allowed = (
      "font",
      "font-family",
      "font_family",
      "size",
      "font-size",
      "font_size",
    )
    for key in typography.keys() {
      if not allowed.contains(key) {
        panic("unsupported merman typography key: " + key)
      }
    }
    text-style-from-font(
      field3(typography, "font", "font-family", "font_family"),
      field3(typography, "size", "font-size", "font_size"),
    )
  }
}

#let merged-typography-style(context-text-style, profile-typography, typography) = {
  let out = context-text-style
  let out = merge-dict(out, typography-text-style(profile-typography), "merman typography")
  merge-dict(out, typography-text-style(typography), "merman typography")
}

#let theme-input(diagram-theme, theme-preset, name) = {
  if diagram-theme != none and theme-preset != none {
    panic(name + " must not contain both `diagram-theme` and `theme-preset`")
  } else if diagram-theme != none {
    (kind: "spec", value: dictionary-or-none(diagram-theme, name + " diagram-theme"))
  } else if theme-preset != none {
    if type(theme-preset) != str or theme-preset.trim() == "" {
      panic(name + " theme-preset must be a non-empty string")
    }
    (kind: "preset", value: theme-preset)
  } else {
    none
  }
}

#let theme-spec-with-typography(spec, text-style) = {
  let spec = dictionary-or-none(spec, "merman diagram-theme")
  if text-style == none {
    spec
  } else {
    let typography = if "typography" in spec {
      let value = dictionary-or-none(spec.at("typography"), "merman diagram-theme typography")
      if value == none { (:) } else { value }
    } else {
      (:)
    }
    let default = if "default" in typography {
      let value = dictionary-or-none(
        typography.at("default"),
        "merman diagram-theme default typography",
      )
      if value == none { (:) } else { value }
    } else {
      (:)
    }
    let default = (: ..default, ..text-style)
    (: ..spec, typography: (: ..typography, default: default))
  }
}

#let build-theme-selection(input, text-style) = {
  if input == none {
    if text-style == none {
      none
    } else {
      (spec: (typography: (default: text-style)))
    }
  } else if input.kind == "preset" {
    if text-style != none {
      panic(
        "merman typography cannot be combined with `theme-preset`; use `diagram-theme` with an explicit typography spec",
      )
    }
    (preset: input.value)
  } else {
    (spec: theme-spec-with-typography(input.value, text-style))
  }
}

#let apply-mermaid-theme-site-config(site-config, theme-variables, theme-name, base-theme) = {
  let theme-name = choose-value(base-theme, theme-name)
  if theme-variables == none and theme-name == none {
    dictionary-or-none(site-config, "merman site-config")
  } else {
    let out = if site-config == none {
      (:)
    } else {
      dictionary-or-none(site-config, "merman site-config")
    }
    let out = if theme-name != none {
      (: ..out, theme: theme-name)
    } else {
      out
    }
    if theme-variables != none {
      (: ..out, themeVariables: theme-variables)
    } else {
      out
    }
  }
}

#let build-layout-options(
  layout,
  container-width,
  container-height,
  base-layout: none,
) = {
  if layout != none {
    layout
  } else {
    let out = if base-layout != none {
      dictionary-or-none(base-layout, "merman layout")
    } else {
      (:)
    }
    let out = if container-width != none {
      (: ..out, container_width: container-width)
    } else {
      out
    }
    let out = if container-height != none {
      (: ..out, container_height: container-height)
    } else {
      out
    }
    out
  }
}

#let layout-container-width(layout) = {
  let layout = dictionary-or-none(layout, "merman layout")
  if layout != none and "container_width" in layout {
    layout.at("container_width")
  } else {
    none
  }
}

#let build-environment-options(
  environment,
  text-measurement,
  math-renderer,
  base-environment: none,
) = {
  let out = merge-dict(base-environment, environment, "merman environment")
  let out = if text-measurement != none {
    let out = if out == none { (:) } else { out }
    (: ..out, text_measurement: text-measurement)
  } else {
    out
  }
  if math-renderer != none {
    let out = if out == none { (:) } else { out }
    (: ..out, math_renderer: math-renderer)
  } else {
    out
  }
}

#let mermaid-profile(
  options: none,
  site-config: none,
  typography: none,
  diagram-theme: none,
  theme-preset: none,
  theme-variables: none,
  theme-name: none,
  base-theme: none,
  pipeline: none,
  id: none,
  diagram-id: none,
  background: none,
  layout: none,
  environment: none,
  scoped-css: none,
  css-override-policy: none,
  drop-native-duplicate-fallbacks: none,
  text-measurement: none,
  math-renderer: none,
  container-width: none,
  container-height: none,
  fixed-today: none,
  fixed-local-offset-minutes: none,
  figure: none,
) = {
  (
    options: options,
    site-config: site-config,
    typography: typography,
    diagram-theme: diagram-theme,
    theme-preset: theme-preset,
    theme-variables: theme-variables,
    theme-name: theme-name,
    base-theme: base-theme,
    pipeline: pipeline,
    id: id,
    diagram-id: diagram-id,
    background: background,
    layout: layout,
    environment: environment,
    scoped-css: scoped-css,
    css-override-policy: css-override-policy,
    drop-native-duplicate-fallbacks: drop-native-duplicate-fallbacks,
    text-measurement: text-measurement,
    math-renderer: math-renderer,
    container-width: container-width,
    container-height: container-height,
    fixed-today: fixed-today,
    fixed-local-offset-minutes: fixed-local-offset-minutes,
    figure: figure,
  )
}

#let render-config(
  options: none,
  profile: none,
  typography: none,
  context-text-style: none,
  site-config: none,
  diagram-theme: none,
  theme-preset: none,
  theme-variables: none,
  theme-name: none,
  base-theme: none,
  pipeline: none,
  id: none,
  diagram-id: none,
  background: none,
  layout: none,
  environment: none,
  scoped-css: none,
  css-override-policy: none,
  drop-native-duplicate-fallbacks: none,
  text-measurement: none,
  math-renderer: none,
  container-width: none,
  container-height: none,
  fixed-today: none,
  fixed-local-offset-minutes: none,
) = {
  let profile = validated-profile(profile)
  let profile-options = profile-field(profile, "options")
  let profile-site-config = profile-field(profile, "site-config", alt: "site_config")
  let profile-typography = profile-field(profile, "typography")
  let profile-theme-input = theme-input(
    profile-field(profile, "diagram-theme", alt: "diagram_theme"),
    profile-field(profile, "theme-preset", alt: "theme_preset"),
    "merman profile",
  )
  let direct-theme-input = theme-input(diagram-theme, theme-preset, "merman options")
  let selected-theme-input = if direct-theme-input != none {
    direct-theme-input
  } else {
    profile-theme-input
  }
  let profile-layout = profile-field(profile, "layout")
  let profile-layout-container-width = layout-container-width(profile-layout)
  let profile-environment = profile-field(profile, "environment")
  let profile-text-measurement = profile-field(profile, "text-measurement")
  let profile-math-renderer = profile-field(profile, "math-renderer")

  let profile-site-config = apply-mermaid-theme-site-config(
    profile-site-config,
    profile-field(profile, "theme-variables", alt: "theme_variables"),
    profile-field(profile, "theme-name", alt: "theme_name"),
    profile-field(profile, "base-theme", alt: "base_theme"),
  )
  let site-config = if site-config == none {
    profile-site-config
  } else {
    dictionary-or-none(site-config, "merman site-config")
  }
  let site-config = apply-mermaid-theme-site-config(
    site-config,
    theme-variables,
    theme-name,
    base-theme,
  )
  let pipeline = choose-value(profile-field(profile, "pipeline"), pipeline, default: "resvg-safe")
  let id = choose-value(profile-field(profile, "id"), id)
  let diagram-id = choose-value(profile-field(profile, "diagram-id", alt: "diagram_id"), diagram-id)
  let background = choose-value(profile-field(profile, "background"), background)
  let scoped-css = choose-value(profile-field(profile, "scoped-css", alt: "scoped_css"), scoped-css)
  let css-override-policy = choose-value(
    profile-field(profile, "css-override-policy", alt: "css_override_policy"),
    css-override-policy,
  )
  let drop-native-duplicate-fallbacks = choose-value(
    profile-field(
      profile,
      "drop-native-duplicate-fallbacks",
      alt: "drop_native_duplicate_fallbacks",
    ),
    drop-native-duplicate-fallbacks,
  )
  let container-width = choose-value(
    profile-field(profile, "container-width", alt: "container_width"),
    container-width,
  )
  let container-height = choose-value(
    profile-field(profile, "container-height", alt: "container_height"),
    container-height,
  )
  let fixed-today = choose-value(profile-field(profile, "fixed-today", alt: "fixed_today"), fixed-today)
  let fixed-local-offset-minutes = choose-value(
    profile-field(profile, "fixed-local-offset-minutes", alt: "fixed_local_offset_minutes"),
    fixed-local-offset-minutes,
  )
  let text-style = merged-typography-style(
    context-text-style,
    profile-typography,
    typography,
  )
  let diagram-theme-selection = build-theme-selection(selected-theme-input, text-style)

  let binding-options = if options != none {
    options
  } else if profile-options != none {
    profile-options
  } else {
    let binding-options = (
      version: 2,
      fixed_today: fixed-today,
      fixed_local_offset_minutes: fixed-local-offset-minutes,
      site_config: site-config,
      layout: build-layout-options(
        layout,
        container-width,
        container-height,
        base-layout: profile-layout,
      ),
      environment: build-environment-options(
        environment,
        text-measurement,
        math-renderer,
        base-environment: build-environment-options(
          profile-environment,
          profile-text-measurement,
          profile-math-renderer,
        ),
      ),
      svg: (
        diagram_id: choose-value(id, diagram-id),
        pipeline: pipeline,
        root_background_color: background,
        scoped_css: scoped-css,
        css_override_policy: css-override-policy,
        drop_native_duplicate_fallbacks: drop-native-duplicate-fallbacks,
      ),
    )
    if diagram-theme-selection == none {
      binding-options
    } else {
      (: ..binding-options, theme: diagram-theme-selection)
    }
  }

  (
    binding_options: binding-options,
    direct_layout: layout,
    direct_options: options,
    direct_container_width: container-width,
    profile_layout: profile-layout,
    profile_layout_container_width: profile-layout-container-width,
    profile_options: profile-options,
  )
}

#let config-with-context-width(config, width) = {
  if width == none or config.direct_layout != none or config.direct_container_width != none or config.direct_options != none or config.profile_options != none or config.profile_layout_container_width != none {
    config
  } else {
    let binding-options = config.binding_options
    let layout = build-layout-options(
      none,
      width,
      none,
      base-layout: config.profile_layout,
    )
    (: ..config, binding_options: (: ..binding-options, layout: layout))
  }
}

#let build-binding-options(..args) = {
  render-config(..args).binding_options
}

#let options-bytes(options) = {
  if options == none {
    bytes(())
  } else {
    bytes(json.encode(options))
  }
}
