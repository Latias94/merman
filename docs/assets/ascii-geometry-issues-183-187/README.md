# ASCII geometry: issues #183–#187

Actual Unicode CLI output before `2928265122d5a3d35e239e3ddbce885c7e342c59` and after `3be24866d8f373bf573d708a149fb8fa5d7c58da`.

The PNGs are review illustrations, not golden fixtures. `comparison.json` preserves the original
Mermaid inputs, invocation arguments, structured reports, and unmodified text. Each pair uses the
same input. Highlighted cells are presentation annotations.

For #187, the old binary uses its default padding because it does not support the new flag.
The new binary uses `--ascii-node-padding-y 0`; default padding remains unchanged.

Dimensions are terminal display cells, not image pixels. There is no trimming or lossy fallback.

## #183: Loop-back arrow reaches its target

![Issue #183: before and after](issue-183.png)

Input:

```mermaid
flowchart TD
  A[Write code] --> B{Tests pass?}
  B -- Yes --> C[Open PR]
  B -- No --> A
```

Before (16 columns × 25 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical issue.mmd
```

After (17 columns × 25 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical issue.mmd
```

## #184: State transition labels stay separate

![Issue #184: before and after](issue-184.png)

Input:

```mermaid
stateDiagram-v2
  Draft --> Review: submit
  Review --> Draft: changes requested
```

Before (30 columns × 15 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical issue.mmd
```

After (31 columns × 15 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical issue.mmd
```

## #185: Message text fits between lifelines

![Issue #185: before and after](issue-185.png)

Input:

```mermaid
sequenceDiagram
  participant API
  participant DB
  API->>DB: find user by email
```

Before (23 columns × 7 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical issue.mmd
```

After (27 columns × 7 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical issue.mmd
```

## #186: Compact keeps both subgraph frames

![Issue #186: before and after](issue-186.png)

Input:

```mermaid
flowchart LR
  subgraph CI
    L[Lint] --> T[Test]
  end
  subgraph Deploy
    S[Staging] --> P[Prod]
  end
  T --> S
```

Before (48 columns × 11 rows):

```console
merman-cli render --format unicode --ascii-layout-profile compact issue.mmd
```

After (50 columns × 11 rows):

```console
merman-cli render --format unicode --ascii-layout-profile compact issue.mmd
```

## #187: Zero vertical padding saves six rows

![Issue #187: before and after](issue-187.png)

Input:

```mermaid
flowchart TD
  A[Write code] --> B[Open PR] --> C[Merge]
```

Before (14 columns × 25 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical issue.mmd
```

After (14 columns × 19 rows):

```console
merman-cli render --format unicode --ascii-layout-profile canonical --ascii-node-padding-y 0 issue.mmd
```

## Capture notes

Save each input as `issue.mmd`, then run the listed command with the corresponding revision.
Adding `--ascii-report` returns the structured report recorded in `comparison.json`.
The images rasterize that recorded text with an equal-width terminal font; the renderer output
does not contain the colors or highlights.
