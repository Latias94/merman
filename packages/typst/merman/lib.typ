#let _merman-plugin = plugin("merman_typst_plugin.wasm")

#let merman-capabilities() = {
  json(_merman-plugin.capabilities_json())
}

#let _source-text(source) = {
  if type(source) == str {
    source
  } else {
    source.text
  }
}

#let _typst-layout = layout

#let _dictionary-or-none(value, name) = {
  if value == none {
    none
  } else if type(value) == dictionary {
    value
  } else {
    panic(name + " must be a dictionary")
  }
}

#let _profile-field(profile, key, alt: none) = {
  let profile = _dictionary-or-none(profile, "merman profile")
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

#let _figure-field(figure-profile, key, alt: none) = {
  let figure-profile = _dictionary-or-none(figure-profile, "merman figure profile")
  if figure-profile == none {
    none
  } else if key in figure-profile {
    figure-profile.at(key)
  } else if alt != none and alt in figure-profile {
    figure-profile.at(alt)
  } else {
    none
  }
}

#let _choose(profile-value, direct-value, default: none) = {
  if direct-value != none {
    direct-value
  } else if profile-value != none {
    profile-value
  } else {
    default
  }
}

#let _merge-dict(base, override, name) = {
  let base = _dictionary-or-none(base, name)
  let override = _dictionary-or-none(override, name)
  if base == none {
    override
  } else if override == none {
    base
  } else {
    (: ..base, ..override)
  }
}

#let _theme-name(theme-name, base-theme) = {
  if theme-name != none {
    theme-name
  } else {
    base-theme
  }
}

#let _diagram-id(id, diagram-id) = {
  if diagram-id != none {
    diagram-id
  } else {
    id
  }
}

#let _font-family-value(font) = {
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

#let _font-size-value(size) = {
  if size == none {
    none
  } else if type(size) == str {
    size
  } else {
    repr(size)
  }
}

#let _host-theme-from-font(font-family, font-size) = {
  let family = _font-family-value(font-family)
  let size = _font-size-value(font-size)
  if family == none and size == none {
    none
  } else {
    (
      font_family: family,
      font_size: size,
    )
  }
}

#let _context-host-theme(font-family, font-size) = {
  _host-theme-from-font(font-family, font-size)
}

#let _field3(dict, a, b, c) = {
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

#let _typography-host-theme(typography) = {
  let typography = _dictionary-or-none(typography, "merman typography")
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
    _host-theme-from-font(
      _field3(typography, "font", "font-family", "font_family"),
      _field3(typography, "size", "font-size", "font_size"),
    )
  }
}

#let _merged-host-theme(
  context-host-theme,
  profile-typography,
  profile-host-theme,
  typography,
  host-theme,
) = {
  let out = context-host-theme
  let out = _merge-dict(out, _typography-host-theme(profile-typography), "merman host-theme")
  let out = _merge-dict(out, profile-host-theme, "merman host-theme")
  let out = _merge-dict(out, _typography-host-theme(typography), "merman host-theme")
  _merge-dict(out, host-theme, "merman host-theme")
}

#let _site-config(site-config, theme, theme-name, base-theme) = {
  if site-config != none {
    site-config
  } else if theme != none or theme-name != none or base-theme != none {
    (
      theme: _theme-name(theme-name, base-theme),
      themeVariables: theme,
    )
  } else {
    none
  }
}

#let _layout-options(
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
      _dictionary-or-none(base-layout, "merman layout")
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

#let _scale-factor(factor) = {
  if type(factor) == int or type(factor) == float {
    factor * 100%
  } else {
    factor
  }
}

#let _scaled(body, factor) = {
  if factor == none {
    body
  } else {
    scale(x: _scale-factor(factor), y: _scale-factor(factor), reflow: true, body)
  }
}

#let _error-message(result) = {
  if result.message == none {
    result.code_name
  } else {
    result.message
  }
}

#let _diagram-error(result, error-mode, width) = {
  if error-mode == "text" {
    text(fill: rgb("#b91c1c"))[merman: #_error-message(result)]
  } else if error-mode == "placeholder" {
    block(
      width: width,
      inset: 8pt,
      fill: rgb("#fff7f7"),
      stroke: rgb("#ef4444"),
    )[
      #strong[merman diagram error]
      #linebreak()
      #_error-message(result)
    ]
  } else {
    panic("unknown merman error-mode: " + str(error-mode))
  }
}

#let _svg-bytes-or-panic(result) = {
  if result.ok {
    bytes(result.svg)
  } else {
    panic(_error-message(result))
  }
}

#let _result-image(
  result,
  width,
  height,
  fit,
  alt,
  scale,
  error-mode,
) = {
  if result.ok {
    _scaled(image(
      bytes(result.svg),
      format: "svg",
      width: width,
      height: height,
      fit: fit,
      alt: alt,
    ), scale)
  } else if error-mode == "panic" {
    panic(_error-message(result))
  } else {
    _diagram-error(result, error-mode, width)
  }
}

#let _figure-caption(caption, caption-position, caption-separator) = {
  if caption == none {
    none
  } else if caption-position != none and caption-separator != none {
    figure.caption(position: caption-position, separator: caption-separator, caption)
  } else if caption-position != none {
    figure.caption(position: caption-position, caption)
  } else if caption-separator != none {
    figure.caption(separator: caption-separator, caption)
  } else {
    caption
  }
}

#let _figure-options(
  profile,
  caption,
  placement,
  scope,
  supplement,
  numbering,
  outlined,
  gap,
  caption-position,
  caption-separator,
) = {
  let figure-profile = _profile-field(profile, "figure")
  let placement = _choose(_figure-field(figure-profile, "placement"), placement)
  let scope = _choose(_figure-field(figure-profile, "scope"), scope)
  let supplement = _choose(_figure-field(figure-profile, "supplement"), supplement)
  let numbering = _choose(_figure-field(figure-profile, "numbering"), numbering)
  let outlined = _choose(_figure-field(figure-profile, "outlined"), outlined)
  let gap = _choose(_figure-field(figure-profile, "gap"), gap)
  let caption-position = _choose(
    _figure-field(figure-profile, "caption-position", alt: "caption_position"),
    caption-position,
  )
  let caption-separator = _choose(
    _figure-field(figure-profile, "caption-separator", alt: "caption_separator"),
    caption-separator,
  )

  let caption = _figure-caption(caption, caption-position, caption-separator)
  let out = if caption != none {
    (caption: caption)
  } else {
    (:)
  }
  let out = if placement != none {
    (: ..out, placement: placement)
  } else {
    out
  }
  let out = if scope != none {
    (: ..out, scope: scope)
  } else {
    out
  }
  let out = if supplement != none {
    (: ..out, supplement: supplement)
  } else {
    out
  }
  let out = if numbering != none {
    (: ..out, numbering: numbering)
  } else {
    out
  }
  let out = if outlined != none {
    (: ..out, outlined: outlined)
  } else {
    out
  }
  if gap != none {
    (: ..out, gap: gap)
  } else {
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

#let _binding-options(
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
  } else if _profile-field(profile, "options") != none {
    _profile-field(profile, "options")
  } else {
    let profile-site-config = _profile-field(profile, "site-config", alt: "site_config")
    let profile-host-theme = _profile-field(profile, "host-theme", alt: "host_theme")
    let profile-typography = _profile-field(profile, "typography")
    let profile-layout = _profile-field(profile, "layout")

    let site-config = _choose(profile-site-config, site-config)
    let theme = _choose(_profile-field(profile, "theme"), theme)
    let theme-name = _choose(_profile-field(profile, "theme-name", alt: "theme_name"), theme-name)
    let base-theme = _choose(_profile-field(profile, "base-theme", alt: "base_theme"), base-theme)
    let pipeline = _choose(_profile-field(profile, "pipeline"), pipeline, default: "resvg-safe")
    let id = _choose(_profile-field(profile, "id"), id)
    let diagram-id = _choose(_profile-field(profile, "diagram-id", alt: "diagram_id"), diagram-id)
    let background = _choose(_profile-field(profile, "background"), background)
    let scoped-css = _choose(_profile-field(profile, "scoped-css", alt: "scoped_css"), scoped-css)
    let css-override-policy = _choose(
      _profile-field(profile, "css-override-policy", alt: "css_override_policy"),
      css-override-policy,
    )
    let drop-native-duplicate-fallbacks = _choose(
      _profile-field(
        profile,
        "drop-native-duplicate-fallbacks",
        alt: "drop_native_duplicate_fallbacks",
      ),
      drop-native-duplicate-fallbacks,
    )
    let text-measurer = _choose(
      _profile-field(profile, "text-measurer", alt: "text_measurer"),
      text-measurer,
    )
    let math-renderer = _choose(
      _profile-field(profile, "math-renderer", alt: "math_renderer"),
      math-renderer,
    )
    let viewport-width = _choose(
      _profile-field(profile, "viewport-width", alt: "viewport_width"),
      viewport-width,
    )
    let viewport-height = _choose(
      _profile-field(profile, "viewport-height", alt: "viewport_height"),
      viewport-height,
    )
    let fixed-today = _choose(_profile-field(profile, "fixed-today", alt: "fixed_today"), fixed-today)
    let fixed-local-offset-minutes = _choose(
      _profile-field(profile, "fixed-local-offset-minutes", alt: "fixed_local_offset_minutes"),
      fixed-local-offset-minutes,
    )

    let host-theme = _merged-host-theme(
      context-host-theme,
      profile-typography,
      profile-host-theme,
      typography,
      host-theme,
    )

    (
      fixed_today: fixed-today,
      fixed_local_offset_minutes: fixed-local-offset-minutes,
      site_config: _site-config(site-config, theme, theme-name, base-theme),
      host_theme: host-theme,
      layout: _layout-options(
        layout,
        text-measurer,
        math-renderer,
        viewport-width,
        viewport-height,
        base-layout: profile-layout,
      ),
      svg: (
        diagram_id: _diagram-id(id, diagram-id),
        pipeline: pipeline,
        root_background_color: background,
        scoped_css: scoped-css,
        css_override_policy: css-override-policy,
        drop_native_duplicate_fallbacks: drop-native-duplicate-fallbacks,
      ),
    )
  }
}

#let _options-bytes(options) = {
  if options == none {
    bytes(())
  } else {
    bytes(json.encode(options))
  }
}

#let _render-svg-result(
  source,
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
  let source-text = _source-text(source)
  let binding-options = _binding-options(
    options: options,
    profile: profile,
    typography: typography,
    context-host-theme: context-host-theme,
    site-config: site-config,
    host-theme: host-theme,
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
  )

  json(_merman-plugin.render_svg_json(bytes(source-text), _options-bytes(binding-options)))
}

#let _render-svg-bytes(..args) = {
  _svg-bytes-or-panic(_render-svg-result(..args))
}

#let _validate-payload(
  source,
  options: none,
  profile: none,
  typography: none,
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
  let source-text = _source-text(source)
  let binding-options = _binding-options(
    options: options,
    profile: profile,
    typography: typography,
    site-config: site-config,
    host-theme: host-theme,
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
  )

  json(_merman-plugin.validate_json(bytes(source-text), _options-bytes(binding-options)))
}

#let mermaid-result(
  source,
  options: none,
  profile: none,
  typography: none,
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
) = _render-svg-result(
  source,
  options: options,
  profile: profile,
  typography: typography,
  site-config: site-config,
  host-theme: host-theme,
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
)

#let mermaid-svg(
  source,
  options: none,
  profile: none,
  typography: none,
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
  str(_render-svg-bytes(
    source,
    options: options,
    profile: profile,
    typography: typography,
    site-config: site-config,
    host-theme: host-theme,
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
  ))
}

#let validate-mermaid(
  source,
  options: none,
  profile: none,
  typography: none,
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
) = _validate-payload(
  source,
  options: options,
  profile: profile,
  typography: typography,
  site-config: site-config,
  host-theme: host-theme,
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
)

#let mermaid(
  source,
  options: none,
  profile: none,
  typography: none,
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
  width: auto,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "panic",
) = {
  let result = _render-svg-result(
    source,
    options: options,
    profile: profile,
    typography: typography,
    site-config: site-config,
    host-theme: host-theme,
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
  )
  _result-image(result, width, height, fit, alt, scale, error-mode)
}

#let mermaid-context(
  source,
  options: none,
  profile: none,
  typography: none,
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
  width: auto,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "panic",
) = context {
  let context-host-theme = _context-host-theme(text.font, text.size)
  if layout != none or viewport-width != none {
    let result = _render-svg-result(
      source,
      options: options,
      profile: profile,
      typography: typography,
      context-host-theme: context-host-theme,
      site-config: site-config,
      host-theme: host-theme,
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
    )
    _result-image(result, width, height, fit, alt, scale, error-mode)
  } else {
    _typst-layout(size => {
      let result = _render-svg-result(
        source,
        options: options,
        profile: profile,
        typography: typography,
        context-host-theme: context-host-theme,
        site-config: site-config,
        host-theme: host-theme,
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
        viewport-width: size.width / 1pt,
        viewport-height: viewport-height,
        fixed-today: fixed-today,
        fixed-local-offset-minutes: fixed-local-offset-minutes,
      )
      _result-image(result, width, height, fit, alt, scale, error-mode)
    })
  }
}

#let mermaid-figure(
  source,
  caption: none,
  context-aware: false,
  placement: none,
  scope: none,
  supplement: none,
  numbering: none,
  outlined: none,
  gap: none,
  caption-position: none,
  caption-separator: none,
  options: none,
  profile: none,
  typography: none,
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
  width: auto,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "panic",
) = {
  let diagram = if context-aware {
    mermaid-context(
      source,
      options: options,
      profile: profile,
      typography: typography,
      site-config: site-config,
      host-theme: host-theme,
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
      width: width,
      height: height,
      fit: fit,
      scale: scale,
      alt: alt,
      error-mode: error-mode,
    )
  } else {
    mermaid(
      source,
      options: options,
      profile: profile,
      typography: typography,
      site-config: site-config,
      host-theme: host-theme,
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
      width: width,
      height: height,
      fit: fit,
      scale: scale,
      alt: alt,
      error-mode: error-mode,
    )
  }
  let figure-options = _figure-options(
    profile,
    caption,
    placement,
    scope,
    supplement,
    numbering,
    outlined,
    gap,
    caption-position,
    caption-separator,
  )
  figure(diagram, ..figure-options)
}

#let mermaid-raw(
  block,
  options: none,
  profile: none,
  typography: none,
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
  width: auto,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "panic",
) = {
  mermaid(
    block.text,
    options: options,
    profile: profile,
    typography: typography,
    site-config: site-config,
    host-theme: host-theme,
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
    width: width,
    height: height,
    fit: fit,
    scale: scale,
    alt: alt,
    error-mode: error-mode,
  )
}

#let show-mermaid-blocks-context(
  options: none,
  profile: none,
  typography: none,
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
  width: 100%,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "placeholder",
) = block => mermaid-context(
  block.text,
  options: options,
  profile: profile,
  typography: typography,
  site-config: site-config,
  host-theme: host-theme,
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
  width: width,
  height: height,
  fit: fit,
  scale: scale,
  alt: alt,
  error-mode: error-mode,
)

#let show-mermaid-blocks(
  options: none,
  profile: none,
  typography: none,
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
  width: 100%,
  height: auto,
  fit: "contain",
  scale: none,
  alt: none,
  error-mode: "placeholder",
) = block => mermaid-raw(
  block,
  options: options,
  profile: profile,
  typography: typography,
  site-config: site-config,
  host-theme: host-theme,
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
  width: width,
  height: height,
  fit: fit,
  scale: scale,
  alt: alt,
  error-mode: error-mode,
)
