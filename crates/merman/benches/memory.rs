#[cfg(any(
    feature = "svg",
    all(
        feature = "all-diagrams",
        feature = "compat-json",
        feature = "editor-facts"
    )
))]
#[path = "memory/mod.rs"]
mod probes;

#[cfg(any(
    feature = "svg",
    all(
        feature = "all-diagrams",
        feature = "compat-json",
        feature = "editor-facts"
    )
))]
fn main() -> Result<(), String> {
    probes::run()
}

// Cargo can run all targets even when none of the memory lanes are compiled.
#[cfg(not(any(
    feature = "svg",
    all(
        feature = "all-diagrams",
        feature = "compat-json",
        feature = "editor-facts"
    )
)))]
fn main() {}
