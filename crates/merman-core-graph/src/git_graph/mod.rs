//! GitGraph command syntax and database construction.

mod db;
mod parser;

use merman_core_preprocess::scan::langium::LangiumCommonFacts;
use merman_model::SourceSpan;

pub use parser::GitGraphHeader;
#[cfg(feature = "editor-facts")]
pub use parser::{
    GitGraphArgumentFact, GitGraphArgumentKind, GitGraphArgumentRole, GitGraphSyntaxEvent,
    is_gitgraph_reference,
};

/// A GitGraph parser or database failure, before facade source mapping.
#[derive(Debug)]
pub struct GitGraphDiagnostic {
    pub message: String,
    pub location: GitGraphDiagnosticLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitGraphDiagnosticLocation {
    Unspanned,
    Exact(SourceSpan),
    CommonInsertionPoint(usize),
}

type GitGraphResult<T> = Result<T, GitGraphDiagnostic>;

impl GitGraphDiagnostic {
    fn fallback(message: String) -> Self {
        Self {
            message,
            location: GitGraphDiagnosticLocation::Unspanned,
        }
    }

    fn exact(message: String, span: SourceSpan) -> Self {
        Self {
            message,
            location: GitGraphDiagnosticLocation::Exact(span),
        }
    }

    fn with_exact_span_if_missing(mut self, span: SourceSpan) -> Self {
        if self.location == GitGraphDiagnosticLocation::Unspanned {
            self.location = GitGraphDiagnosticLocation::Exact(span);
        }
        self
    }
}

const COMMIT_TYPE_NORMAL: i64 = 0;
const COMMIT_TYPE_REVERSE: i64 = 1;
const COMMIT_TYPE_HIGHLIGHT: i64 = 2;
const COMMIT_TYPE_MERGE: i64 = 3;
const COMMIT_TYPE_CHERRY_PICK: i64 = 4;

#[derive(Debug, Clone)]
struct CommitDb {
    id: String,
    msg: String,
    commit_type: i64,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct BranchDb {
    name: String,
    order: f64,
}

#[derive(Debug, Clone)]
struct MergeDb {
    branch: String,
    id: Option<String>,
    commit_type: Option<i64>,
    tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct CherryPickDb {
    id: String,
    parent: String,
    tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
enum GitGraphOperation {
    Commit(CommitDb),
    Branch(BranchDb),
    Checkout(String),
    Merge(MergeDb),
    CherryPick(CherryPickDb),
}

#[derive(Debug, Clone)]
struct GitGraphCommand {
    operation: GitGraphOperation,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<Vec<GitGraphArgumentFact>>,
    statement_span: SourceSpan,
}

/// Validated GitGraph commands, ready for a single source-ordered database replay.
pub struct GitGraphSyntax {
    direction: Option<String>,
    commands: Vec<GitGraphCommand>,
    common: LangiumCommonFacts,
}
