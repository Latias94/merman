#import "theme.typ": theme-catalog
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

#let resolve-diagram-id(profile-id, profile-diagram-id, direct-id, direct-diagram-id) = {
  if direct-diagram-id != none {
    direct-diagram-id
  } else if direct-id != none {
    direct-id
  } else if profile-diagram-id != none {
    profile-diagram-id
  } else {
    profile-id
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
        panic("merman profile field `" + key + "` was removed; use `theme`")
      }
    }
    for key in ("host-theme", "host_theme") {
      if key in profile {
        panic("merman profile field `" + key + "` was removed; use `theme` or `typography`")
      }
    }
    for key in ("scoped-css", "scoped_css") {
      if key in profile {
        panic(
          "merman profile field `" + key + "` was removed; use `theme`, `site-config`, or `background`",
        )
      }
    }
    for key in ("css-override-policy", "css_override_policy") {
      if key in profile {
        panic(
          "merman profile field `" + key + "` was removed; CSS override policy is restricted to trusted Rust or native CLI hosts",
        )
      }
    }
    for key in ("diagram-theme", "theme-preset", "theme-recipe", "theme-appearance", "theme-variables", "theme-name", "base-theme") {
      if key in profile or key.replace("-", "_") in profile {
        panic("merman profile field `" + key + "` was removed; use `theme`, `appearance`, or `site-config`")
      }
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
    let validated = css-px-number-string(size, name: "merman typography size")
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

#let merged-typography-style(context-text-style, profile-typography, typography, native: false) = {
  let out = context-text-style
  if native and out != none {
    // Keep inferred document typography within the Theme v2 patch bounds.
    if out.at("font_stack", default: ()).len() > 8 {
      out.font_stack = out.font_stack.slice(0, 8)
    }
    if "font_size_px" in out { out.font_size_px = calc.clamp(out.font_size_px, 10, 24) }
  }
  let out = merge-dict(out, typography-text-style(profile-typography), "merman typography")
  merge-dict(out, typography-text-style(typography), "merman typography")
}

#let apply-mermaid-style(site-config, theme, text-style) = {
  if theme == none and text-style == none { return site-config }

  let config = if site-config == none { (:) } else { site-config }
  if theme != none { config.insert("theme", theme) }
  if text-style != none {
    let variables = dictionary-or-none(
      config.at("themeVariables", default: none),
      "merman site-config.themeVariables",
    )
    let variables = if variables == none { (:) } else { variables }
    if "font_stack" in text-style {
      let family = text-style.font_stack.join(", ")
      config.insert("fontFamily", family)
      variables.insert("fontFamily", family)
    }
    if "font_size_px" in text-style {
      variables.insert("fontSize", css-px-number-string(text-style.font_size_px))
    }
    config.insert("themeVariables", variables)
  }
  config
}

#let native-theme-selection(theme, appearance, text-style) = {
  if type(theme) == str and text-style == none {
    return if appearance == none { theme } else { (preset: theme, appearance: appearance) }
  }

  let document = if type(theme) == str { (schema_version: 2, extends: theme) } else { theme }
  let patch = document.at("typography", default: (:))
  if text-style != none and type(patch) == dictionary {
    if "font_stack" in text-style {
      let stack = text-style.font_stack
      patch.insert("sans", if stack.len() == 1 { stack + ("sans-serif",) } else { stack })
      patch.insert("body_family", "sans")
    }
    if "font_size_px" in text-style { patch.insert("base_size_px", text-style.font_size_px) }
    document.insert("typography", patch)
  }
  if appearance == none { document } else { (document: document, appearance: appearance) }
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
  theme: none,
  appearance: none,
  pipeline: none,
  id: none,
  diagram-id: none,
  background: none,
  layout: none,
  environment: none,
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
    theme: theme,
    appearance: appearance,
    pipeline: pipeline,
    id: id,
    diagram-id: diagram-id,
    background: background,
    layout: layout,
    environment: environment,
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

#let opaque-render-config(binding-options, direct-options: none, profile-options: none) = {
  (
    binding_options: binding-options,
    direct_layout: none,
    direct_options: direct-options,
    direct_container_width: none,
    profile_layout: none,
    profile_layout_container_width: none,
    profile_options: profile-options,
  )
}

#let render-config(
  options: none,
  profile: none,
  typography: none,
  context-text-style: none,
  site-config: none,
  theme: none,
  appearance: none,
  pipeline: none,
  id: none,
  diagram-id: none,
  background: none,
  layout: none,
  environment: none,
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
  if options != none {
    opaque-render-config(options, direct-options: options)
  } else if profile-options != none {
    opaque-render-config(profile-options, profile-options: profile-options)
  } else {
    let appearance = choose-value(
      if theme == none { profile-field(profile, "appearance") } else { none },
      appearance,
    )
    let theme = choose-value(profile-field(profile, "theme"), theme)
    if theme != none and type(theme) != dictionary and (type(theme) != str or theme.trim() == "") {
      panic("merman theme must be a non-empty theme ID or a ThemeDocumentV2 dictionary")
    }
    if appearance != none and not ("light", "dark").contains(appearance) {
      panic("merman appearance must be `light` or `dark`")
    }
    let classic-theme = if (
      type(theme) == str and theme != "none" and theme-catalog().mermaid_themes.contains(theme)
    ) {
      theme
    } else { none }
    let native = theme != none and theme != "none" and classic-theme == none
    if appearance != none and not native {
      panic("merman appearance requires a Merman theme preset or document")
    }
    let profile-layout = profile-field(profile, "layout")
    let profile-layout-container-width = layout-container-width(profile-layout)
    let profile-environment = profile-field(profile, "environment")
    let profile-text-measurement = profile-field(profile, "text-measurement")
    let profile-math-renderer = profile-field(profile, "math-renderer")

    let site-config = dictionary-or-none(
      choose-value(profile-field(profile, "site-config", alt: "site_config"), site-config),
      "merman site-config",
    )
    let pipeline = choose-value(profile-field(profile, "pipeline"), pipeline, default: "resvg-safe")
    let profile-id = profile-field(profile, "id")
    let profile-diagram-id = profile-field(profile, "diagram-id", alt: "diagram_id")
    let background = choose-value(profile-field(profile, "background"), background)
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
      profile-field(profile, "typography"),
      typography,
      native: native,
    )
    let binding-options = (
      version: 4,
      fixed_today: fixed-today,
      fixed_local_offset_minutes: fixed-local-offset-minutes,
      site_config: apply-mermaid-style(site-config, classic-theme, if native { none } else { text-style }),
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
        diagram_id: resolve-diagram-id(
          profile-id,
          profile-diagram-id,
          id,
          diagram-id,
        ),
        pipeline: pipeline,
        drop_native_duplicate_fallbacks: drop-native-duplicate-fallbacks,
      ),
    )
    if native {
      binding-options.insert("theme", native-theme-selection(theme, appearance, text-style))
    }
    if background != none { binding-options.insert("background", background) }
    (
      binding_options: binding-options,
      direct_layout: layout,
      direct_options: none,
      direct_container_width: container-width,
      profile_layout: profile-layout,
      profile_layout_container_width: profile-layout-container-width,
      profile_options: none,
    )
  }
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
