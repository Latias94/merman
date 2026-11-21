# Changelog

## Unreleased

- Highlight node IDs that start with a dot, including `...`, in Flowchart, Swimlane, and Agentflow. Compact edges such as `.a-.->.b` in Flowchart keep the full operator highlighted.
- Parse and highlight `direction` as a node ID in Flowchart, Swimlane, and Agentflow. Direction statements also recognize values across line breaks and prefixes such as `LR` in `direction LRfoo`.

## 0.3.4 - 2026-11-12

- Keep generic suffixes attached to backtick-quoted Class names in declarations and compact relationship endpoints.
- Recognize compact Class relationship operators without consuming their hyphens or dots as part of the source name. Ordinary and trailing hyphens remain valid in class names.

## 0.3.3 - 2026-11-06

- Refresh the npm development-tool lockfile to semver 7.8.6 and undici 8.12.4.
