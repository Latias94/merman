pub(super) const PAPER_FLAT: &[u8] =
    br#"{"schema_version":2,"extends":"paper","form":{"elevation":"flat"}}"#;
#[cfg(feature = "diagram-flowchart")]
pub(super) const STUDIO: &[u8] = br#"{"schema_version":2,"extends":"studio"}"#;

pub(super) fn nested_title(
    keyword: &str,
    group_keyword: &str,
    html_labels: bool,
    authored_fills: bool,
) -> String {
    let theme = if authored_fills {
        ""
    } else {
        "  theme: redux-color\n  look: neo\n"
    };
    let styles = if authored_fills {
        "style Parent fill:#FF0000\nstyle G fill:#0000FF80\n"
    } else {
        ""
    };
    format!(
        "---\nconfig:\n  layout: shoal\n  htmlLabels: {html_labels}\n{theme}---\n\
         {keyword} TB\n\
         {group_keyword} Parent[Parent]\n\
         {group_keyword} G[A long title with no room beside the entering route]\n\
         A[Short]\nend\nend\n\
         Outside trunk@--> A\n\
         {styles}"
    )
}

#[cfg(feature = "diagram-flowchart")]
pub(super) const DARK_CLUSTER: &str = "\
---
config:
  layout: shoal
---
flowchart TD
  req([Pull Request]) --> gate{CI Gate}
  subgraph ci[Verification]
    direction TB
    unit[Unit Tests]
    lint[Static Checks]
  end
  subgraph prod[Production Deploy]
    rollout[Canary Rollout]
  end
  gate -->|Fast Path| unit
  gate -->|Policy| lint
  unit --> rollout
  lint --> rollout
  style prod fill:#1e3a5f,stroke:#60a5fa,color:#ffffff
";
