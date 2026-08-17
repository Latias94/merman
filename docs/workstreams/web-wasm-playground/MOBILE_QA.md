# Playground Mobile QA

Status: Maintainer runbook
Last updated: 2026-08-16

## Automated Scope

Mobile emulation is an explicit on-demand lane, not a Pages CI gate:

```bash
npm run test:browser:mobile --prefix playground
```

Use `test:browser:mobile:built` after a verified production build. The focused suite owns the
`320x568`, Pixel portrait, `568x320`, safe-area `844x390`, and shortened visual-viewport contracts.
It exercises touch activation, compact toolbar menus, workspace and preview tabs, dialog focus
restoration, dialog scroll ownership, both Preview presentation modes, SVG Bounds, full-screen export,
raster preview/download reachability, viewport pan/pinch/zoom/fit, and page-level horizontal
overflow. The same lane opens the Share menu in portrait and `844x390` safe-area landscape and
distinguishes portable workspace links from issue-reproduction links that restore page/editor/
Preview/presentation/Bounds state without host geometry. One focused mobile WebKit smoke covers layout, tap
activation, and the shared pointer-handler state machine. Synthetic pointer dispatch does not prove
native touch delivery; the real-device checklist below owns that claim. Informational tooltips must
never intercept the following touch target.

Playwright device emulation does not prove real mobile browser behavior. In particular, emulated
safe-area values and viewport resizing cannot reproduce every keyboard, browser-chrome, or display
cutout combination.

## Real-Device Residual Checklist

Run this checklist in current iOS Safari and Android Chrome before making a broad mobile-support
claim:

- Open the editor, place the caret near the final visible line, show the software keyboard, and
  confirm the caret and active line remain reachable without page-level horizontal scrolling.
- Open Examples and Bench with the keyboard both hidden and visible. Confirm the close control,
  scrollable body, and primary/footer actions remain reachable above browser chrome and the home
  indicator.
- Open Export from the toolbar and each Compare pane. Exercise SVG, transparent/custom PNG, JPEG
  quality, width/height/fit sizing, and download in portrait and landscape. The preview may scroll,
  but the close and Download controls must remain reachable and the dialog must not move the page.
- Switch between Infinite Canvas and ViewBox Frame in Visual and Compare. Confirm the control stays
  reachable in portrait and landscape, both Compare panes follow it, and switching does not replace
  the rendered artifact or lose the current camera state.
- Open Share with both presentation modes and SVG Bounds off/on. Confirm the workspace link restores
  only the render workspace, while an issue link restores the selected page, editor tab, Preview
  mode, presentation mode, and Bounds preference without adding host dimensions or camera coordinates.
- Check portrait and landscape on a display with a cutout or rounded corners. No toolbar control,
  dialog action, or status content may enter a safe area.
- Rotate while the editor, Preview, Compare, Examples, Bench, and Export are active. The selected
  workspace, Bounds preference, export recipe, and frozen publication must remain coherent; each
  canvas must refit without a blank or zero-sized SVG, and Compare's second pane must remain
  reachable from its header or gutter.
- Collapse and expand dynamic browser chrome by scrolling. Confirm `100dvh` ownership does not hide
  the toolbar, status bar, dialog close control, or Bench footer.
- Exercise pinch zoom, browser page zoom, text selection, Preview pan, Zoom in, Zoom out, and Fit to
  view. Gestures must not strand focus or make primary controls unreachable.
- Close each dialog with its close button and Escape where the platform exposes a hardware
  keyboard. Focus should return to the launcher.

Record browser version, OS version, device, orientation, and any residual in the release or PR
evidence. Do not convert this checklist into a mandatory browser matrix without a separate runtime
and support-policy decision.
