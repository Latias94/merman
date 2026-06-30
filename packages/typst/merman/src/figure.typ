#import "options.typ": choose-value, dictionary-or-none, profile-field
#import "render.typ": mermaid, mermaid-context

#let figure-field(figure-profile, key, alt: none) = {
  let figure-profile = dictionary-or-none(figure-profile, "merman figure profile")
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

#let figure-caption(caption, caption-position, caption-separator) = {
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

#let build-figure-options(
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
  let figure-profile = profile-field(profile, "figure")
  let placement = choose-value(figure-field(figure-profile, "placement"), placement)
  let scope = choose-value(figure-field(figure-profile, "scope"), scope)
  let supplement = choose-value(figure-field(figure-profile, "supplement"), supplement)
  let numbering = choose-value(figure-field(figure-profile, "numbering"), numbering)
  let outlined = choose-value(figure-field(figure-profile, "outlined"), outlined)
  let gap = choose-value(figure-field(figure-profile, "gap"), gap)
  let caption-position = choose-value(
    figure-field(figure-profile, "caption-position", alt: "caption_position"),
    caption-position,
  )
  let caption-separator = choose-value(
    figure-field(figure-profile, "caption-separator", alt: "caption_separator"),
    caption-separator,
  )

  let caption = figure-caption(caption, caption-position, caption-separator)
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
  let resolved-figure-options = build-figure-options(
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
  figure(diagram, ..resolved-figure-options)
}
