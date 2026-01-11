# Detector Order (mermaid@11.12.2)

Mermaid type detection is order-dependent: the first matching detector wins.

Upstream implementation:

- Registration: `repo-ref/mermaid/packages/mermaid/src/diagram-api/diagram-orchestration.ts`
- Detection loop: `repo-ref/mermaid/packages/mermaid/src/diagram-api/detectType.ts`

## Full build (includeLargeFeatures=true)

1. `error`
2. `---`
3. `flowchart-elk`
4. `mindmap`
5. `architecture`
6. `c4`
7. `kanban`
8. `classDiagram` (classDiagram-v2 / dagre-wrapper path)
9. `class`
10. `er`
11. `gantt`
12. `info`
13. `pie`
14. `requirement`
15. `sequence`
16. `flowchart-v2`
17. `flowchart`
18. `timeline`
19. `gitGraph`
20. `stateDiagram` (stateDiagram-v2 / dagre-wrapper path)
21. `state`
22. `journey`
23. `quadrantChart`
24. `sankey`
25. `packet`
26. `xychart`
27. `block`
28. `radar`
29. `treemap`

Note: Mermaid registers additional diagrams via lazy loaders, but the detector order above still
governs which loader will be chosen for a given text input.

## Tiny build (includeLargeFeatures=false)

1. `error`
2. `---`
3. `c4`
4. `kanban`
5. `classDiagram`
6. `class`
7. `er`
8. `gantt`
9. `info`
10. `pie`
11. `requirement`
12. `sequence`
13. `flowchart-v2`
14. `flowchart`
15. `timeline`
16. `gitGraph`
17. `stateDiagram`
18. `state`
19. `journey`
20. `quadrantChart`
21. `sankey`
22. `packet`
23. `xychart`
24. `block`
25. `radar`
26. `treemap`
