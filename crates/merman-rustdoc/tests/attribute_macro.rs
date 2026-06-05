#[merman_rustdoc::merman(source = "details", pipeline = "readable")]
/// Render an inline Mermaid fence.
///
/// ```mermaid
/// flowchart TD
///   A[Start] --> B[Done]
/// ```
fn documented_fence() {}

#[merman_rustdoc::merman(fail = "keep-source")]
/// Render an external Mermaid file.
///
/// include_mmd!("tests/fixtures/simple.mmd")
fn documented_include() {}

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   M[Module] --> D[Docs]
/// ```
mod documented_module {}

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   S[Struct] --> D[Docs]
/// ```
struct DocumentedStruct;

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   T[Trait] --> D[Docs]
/// ```
trait DocumentedTrait {
    fn run(&self);
}

struct ImplTarget;

impl DocumentedTrait for ImplTarget {
    fn run(&self) {}
}

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   I[Impl] --> D[Docs]
/// ```
impl ImplTarget {
    fn method(&self) {}
}

#[merman_rustdoc::merman(scope = "tree", sanitize = "strict")]
mod documented_tree_scope {
    /// ```mermaid
    /// flowchart TD
    ///   Child --> Docs
    /// ```
    pub fn child() {}

    pub struct Child;

    impl Child {
        /// ```mermaid
        /// flowchart TD
        ///   Method --> Docs
        /// ```
        pub fn method(&self) {}
    }
}

#[test]
fn attribute_macro_expands_for_fence_and_include() {
    documented_fence();
    documented_include();
    let _ = DocumentedStruct;
    let target = ImplTarget;
    target.method();
    target.run();
    documented_tree_scope::child();
    let child = documented_tree_scope::Child;
    child.method();
}
