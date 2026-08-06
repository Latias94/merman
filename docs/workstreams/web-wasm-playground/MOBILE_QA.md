# Playground Mobile QA

Status: Maintainer runbook
Last updated: 2026-08-05

## Automated Scope

Mobile emulation is an explicit on-demand lane, not a Pages CI gate:

```bash
npm run test:browser:mobile --prefix playground
```

Use `test:browser:mobile:built` after a verified production build. The focused suite owns the
`320x568`, Pixel portrait, `568x320`, and shortened visual-viewport contracts. It exercises touch
activation, compact toolbar menus, workspace and preview tabs, dialog focus restoration, dialog
scroll ownership, viewport pan/zoom/fit, and page-level horizontal overflow.

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
- Check portrait and landscape on a display with a cutout or rounded corners. No toolbar control,
  dialog action, or status content may enter a safe area.
- Rotate while the editor, Preview, Examples, and Bench are active. The selected workspace and
  dialog state must remain coherent, and Preview must refit without a blank or zero-sized SVG.
- Collapse and expand dynamic browser chrome by scrolling. Confirm `100dvh` ownership does not hide
  the toolbar, status bar, dialog close control, or Bench footer.
- Exercise pinch zoom, browser page zoom, text selection, Preview pan, Zoom in, Zoom out, and Fit to
  view. Gestures must not strand focus or make primary controls unreachable.
- Close each dialog with its close button and Escape where the platform exposes a hardware
  keyboard. Focus should return to the launcher.

Record browser version, OS version, device, orientation, and any residual in the release or PR
evidence. Do not convert this checklist into a mandatory browser matrix without a separate runtime
and support-policy decision.
