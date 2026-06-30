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

#let choose-theme-name(theme-name, base-theme) = {
  if theme-name != none {
    theme-name
  } else {
    base-theme
  }
}

#let choose-diagram-id(id, diagram-id) = {
  if diagram-id != none {
    diagram-id
  } else {
    id
  }
}

#let font-family-value(font) = {
  if font == none {
    none
  } else if type(font) == str {
    str(font)
  } else if type(font) == array {
    font.join(", ")
  } else {
    panic("merman typography font must be a string or array")
  }
}

#let font-size-value(size) = {
  if size == none {
    none
  } else if type(size) == str {
    size
  } else {
    repr(size)
  }
}

#let host-theme-from-font(font-family, font-size) = {
  let family = font-family-value(font-family)
  let size = font-size-value(font-size)
  if family == none and size == none {
    none
  } else {
    (
      font_family: family,
      font_size: size,
    )
  }
}

#let context-host-theme(font-family, font-size) = {
  host-theme-from-font(font-family, font-size)
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

#let typography-host-theme(typography) = {
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
    host-theme-from-font(
      field3(typography, "font", "font-family", "font_family"),
      field3(typography, "size", "font-size", "font_size"),
    )
  }
}

#let merged-host-theme(
  context-host-theme,
  profile-typography,
  profile-host-theme,
  typography,
  host-theme,
) = {
  let out = context-host-theme
  let out = merge-dict(out, typography-host-theme(profile-typography), "merman host-theme")
  let out = merge-dict(out, profile-host-theme, "merman host-theme")
  let out = merge-dict(out, typography-host-theme(typography), "merman host-theme")
  merge-dict(out, host-theme, "merman host-theme")
}

#let build-site-config(site-config, theme, theme-name, base-theme) = {
  if site-config != none {
    site-config
  } else if theme != none or theme-name != none or base-theme != none {
    (
      theme: choose-theme-name(theme-name, base-theme),
      themeVariables: theme,
    )
  } else {
    none
  }
}

#let build-layout-options(
  layout,
  text-measurer,
  math-renderer,
  viewport-width,
  viewport-height,
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
    let out = if viewport-width != none {
      (: ..out, viewport_width: viewport-width)
    } else {
      out
    }
    let out = if viewport-height != none {
      (: ..out, viewport_height: viewport-height)
    } else {
      out
    }
    let out = if text-measurer != none {
      (: ..out, text_measurer: text-measurer)
    } else {
      out
    }
    let out = if math-renderer != none {
      (: ..out, math_renderer: math-renderer)
    } else {
      out
    }
    out
  }
}

#let mermaid-profile(
  options: none,
  site-config: none,
  host-theme: none,
  typography: none,
  theme: none,
  theme-name: none,
  base-theme: none,
  pipeline: none,
  id: none,
  diagram-id: none,
  background: none,
  layout: none,
  scoped-css: none,
  css-override-policy: none,
  drop-native-duplicate-fallbacks: none,
  text-measurer: none,
  math-renderer: none,
  viewport-width: none,
  viewport-height: none,
  fixed-today: none,
  fixed-local-offset-minutes: none,
  figure: none,
) = {
  (
    options: options,
    site-config: site-config,
    host-theme: host-theme,
    typography: typography,
    theme: theme,
    theme-name: theme-name,
    base-theme: base-theme,
    pipeline: pipeline,
    id: id,
    diagram-id: diagram-id,
    background: background,
    layout: layout,
    scoped-css: scoped-css,
    css-override-policy: css-override-policy,
    drop-native-duplicate-fallbacks: drop-native-duplicate-fallbacks,
    text-measurer: text-measurer,
    math-renderer: math-renderer,
    viewport-width: viewport-width,
    viewport-height: viewport-height,
    fixed-today: fixed-today,
    fixed-local-offset-minutes: fixed-local-offset-minutes,
    figure: figure,
  )
}

#let build-binding-options(
  options: none,
  profile: none,
  typography: none,
  context-host-theme: none,
  site-config: none,
  host-theme: none,
  theme: none,
  theme-name: none,
  base-theme: none,
  pipeline: none,
  id: none,
  diagram-id: none,
  background: none,
  layout: none,
  scoped-css: none,
  css-override-policy: none,
  drop-native-duplicate-fallbacks: none,
  text-measurer: none,
  math-renderer: none,
  viewport-width: none,
  viewport-height: none,
  fixed-today: none,
  fixed-local-offset-minutes: none,
) = {
  if options != none {
    options
  } else if profile-field(profile, "options") != none {
    profile-field(profile, "options")
  } else {
    let profile-site-config = profile-field(profile, "site-config", alt: "site_config")
    let profile-host-theme = profile-field(profile, "host-theme", alt: "host_theme")
    let profile-typography = profile-field(profile, "typography")
    let profile-layout = profile-field(profile, "layout")

    let site-config = choose-value(profile-site-config, site-config)
    let theme = choose-value(profile-field(profile, "theme"), theme)
    let theme-name = choose-value(profile-field(profile, "theme-name", alt: "theme_name"), theme-name)
    let base-theme = choose-value(profile-field(profile, "base-theme", alt: "base_theme"), base-theme)
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
    let text-measurer = choose-value(
      profile-field(profile, "text-measurer", alt: "text_measurer"),
      text-measurer,
    )
    let math-renderer = choose-value(
      profile-field(profile, "math-renderer", alt: "math_renderer"),
      math-renderer,
    )
    let viewport-width = choose-value(
      profile-field(profile, "viewport-width", alt: "viewport_width"),
      viewport-width,
    )
    let viewport-height = choose-value(
      profile-field(profile, "viewport-height", alt: "viewport_height"),
      viewport-height,
    )
    let fixed-today = choose-value(profile-field(profile, "fixed-today", alt: "fixed_today"), fixed-today)
    let fixed-local-offset-minutes = choose-value(
      profile-field(profile, "fixed-local-offset-minutes", alt: "fixed_local_offset_minutes"),
      fixed-local-offset-minutes,
    )

    let host-theme = merged-host-theme(
      context-host-theme,
      profile-typography,
      profile-host-theme,
      typography,
      host-theme,
    )

    (
      fixed_today: fixed-today,
      fixed_local_offset_minutes: fixed-local-offset-minutes,
      site_config: build-site-config(site-config, theme, theme-name, base-theme),
      host_theme: host-theme,
      layout: build-layout-options(
        layout,
        text-measurer,
        math-renderer,
        viewport-width,
        viewport-height,
        base-layout: profile-layout,
      ),
      svg: (
        diagram_id: choose-diagram-id(id, diagram-id),
        pipeline: pipeline,
        root_background_color: background,
        scoped_css: scoped-css,
        css_override_policy: css-override-policy,
        drop_native_duplicate_fallbacks: drop-native-duplicate-fallbacks,
      ),
    )
  }
}

#let options-bytes(options) = {
  if options == none {
    bytes(())
  } else {
    bytes(json.encode(options))
  }
}
