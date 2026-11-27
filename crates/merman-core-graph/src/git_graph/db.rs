use merman_core_preprocess::scan::langium::LangiumCommonField;
use merman_markup::sanitize::sanitize_text;
use merman_model::families::git_graph::{GitGraphBranch, GitGraphCommit, GitGraphDiagramModel};
use merman_model::warnings::{DiagramWarningFact, GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID};
use merman_model::{MermaidConfig, OperationControl, OperationControlResult, SourceSpan};
use std::collections::HashMap;

use super::{
    BranchDb, COMMIT_TYPE_CHERRY_PICK, COMMIT_TYPE_MERGE, CherryPickDb, CommitDb, GitGraphCommand,
    GitGraphDiagnostic, GitGraphOperation, GitGraphResult, GitGraphSyntax, MergeDb,
};

#[derive(Debug, Clone)]
struct Commit {
    id: String,
    message: String,
    seq: i64,
    commit_type: i64,
    tags: Vec<String>,
    parents: Vec<String>,
    branch: String,
    custom_type: Option<i64>,
    custom_id: Option<bool>,
}

#[derive(Debug, Clone)]
struct BranchConfig {
    order: f64,
}

#[derive(Debug)]
struct GitGraphDb {
    commits: HashMap<String, Commit>,
    branches: HashMap<String, Option<String>>,
    branch_config: HashMap<String, BranchConfig>,
    branch_config_order: Vec<String>,
    head: Option<String>,
    curr_branch: String,
    direction: String,
    seq: i64,
    warning_facts: Vec<DiagramWarningFact>,
    title: String,
    acc_title: String,
    acc_descr: String,
    prng: Option<XorShift64Star>,
    generate_id_hex: fn(&str, u64, usize) -> String,
}

#[derive(Debug, Clone, Copy)]
struct XorShift64Star {
    state: u64,
}

impl XorShift64Star {
    fn new(seed: u64) -> Self {
        let mut state = seed;
        if state == 0 {
            state = 1;
        }
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        // Mirrors the seeded upstream renderer script used by `xtask gen-upstream-svgs`:
        //   x ^= x >> 12; x ^= x << 25; x ^= x >> 27; return x * 0x2545F4914F6CDD1D (mod 2^64)
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn next_hex_digit(&mut self) -> u8 {
        // Seeded upstream uses `Math.floor(Math.random() * 16)` where `Math.random()` is derived
        // from `next_u64() >> 11` (53 bits). This is equivalent to taking the top nibble of
        // `next_u64()`.
        ((self.next_u64() >> 60) & 0xF) as u8
    }

    fn make_random_hex(&mut self, len: usize) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(len);
        for _ in 0..len {
            let idx = self.next_hex_digit() as usize;
            out.push(HEX[idx] as char);
        }
        out
    }
}

impl GitGraphDb {
    fn clear(&mut self, config: &MermaidConfig) {
        self.commits.clear();
        self.branches.clear();
        self.branch_config.clear();
        self.branch_config_order.clear();
        self.head = None;
        self.direction = "LR".to_string();
        self.seq = 0;
        self.warning_facts.clear();
        self.title.clear();
        self.acc_title.clear();
        self.acc_descr.clear();

        // Mermaid gitGraph auto-generates commit ids using `utils.random({ length: 7 })`, which
        // depends on `Math.random()`. For deterministic test runs (and for reproducible upstream
        // SVG baselines), we allow injecting a seed.
        //
        // Without a GitGraph seed, the active operation supplies the generated ID suffix.
        self.prng = seeded_gitgraph_prng(config);

        let main = config
            .get_str("gitGraph.mainBranchName")
            .unwrap_or("main")
            .to_string();
        let main_order = config
            .get_path(&["gitGraph", "mainBranchOrder"])
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0);
        self.curr_branch = main.clone();

        self.branches.insert(main.clone(), None);
        self.branch_config
            .insert(main.clone(), BranchConfig { order: main_order });
        self.branch_config_order.push(main);
    }

    fn set_direction(&mut self, dir: &str) {
        self.direction = dir.to_string();
    }

    fn next_id(&mut self) -> String {
        if let Some(prng) = self.prng.as_mut() {
            prng.make_random_hex(7)
        } else {
            (self.generate_id_hex)("git-graph.commit-id", self.seq as u64, 7)
        }
    }

    fn commit(
        &mut self,
        mut commit_db: CommitDb,
        config: &MermaidConfig,
        statement_span: SourceSpan,
    ) {
        let id_raw = std::mem::take(&mut commit_db.id);
        let msg_raw = std::mem::take(&mut commit_db.msg);
        let tags_raw = std::mem::take(&mut commit_db.tags);

        let id = sanitize_text(&id_raw, config);
        let msg = sanitize_text(&msg_raw, config);
        let tags: Vec<String> = tags_raw
            .into_iter()
            .map(|t| sanitize_text(&t, config))
            .collect();

        let commit_id = if id.is_empty() {
            let seq = self.seq;
            format!("{seq}-{}", self.next_id())
        } else {
            id
        };

        let parents = self
            .head
            .as_ref()
            .map(|h| vec![h.clone()])
            .unwrap_or_default();

        let new_commit = Commit {
            id: commit_id.clone(),
            message: msg,
            seq: self.seq,
            commit_type: commit_db.commit_type,
            tags,
            parents,
            branch: self.curr_branch.clone(),
            custom_type: None,
            custom_id: None,
        };
        self.seq += 1;

        self.head = Some(new_commit.id.clone());
        if self.commits.contains_key(&new_commit.id) {
            self.warning_facts.push(
                DiagramWarningFact::new(
                    GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID,
                    format!("Commit ID {} already exists", new_commit.id),
                )
                .with_span(statement_span),
            );
        }

        self.commits.insert(new_commit.id.clone(), new_commit);

        self.branches
            .insert(self.curr_branch.clone(), Some(commit_id));
    }

    fn branch(&mut self, mut branch_db: BranchDb, config: &MermaidConfig) -> GitGraphResult<()> {
        branch_db.name = sanitize_text(&branch_db.name, config);
        if self.branches.contains_key(&branch_db.name) {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Trying to create an existing branch. (Help: Either use a new name if you want create a new branch or try using \"checkout {}\")",
                branch_db.name
            )));
        }

        let head_id = self.head.clone();
        self.branches.insert(branch_db.name.clone(), head_id);
        self.branch_config.insert(
            branch_db.name.clone(),
            BranchConfig {
                order: branch_db.order,
            },
        );
        self.branch_config_order.push(branch_db.name.clone());
        self.checkout(&branch_db.name, config)?;
        Ok(())
    }

    fn checkout(&mut self, branch: &str, config: &MermaidConfig) -> GitGraphResult<()> {
        let branch = sanitize_text(branch, config);
        if !self.branches.contains_key(&branch) {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Trying to checkout branch which is not yet created. (Help try using \"branch {}\")",
                branch
            )));
        }
        self.curr_branch = branch.clone();
        let id = self.branches.get(&branch).cloned().unwrap_or_default();
        self.head = id;
        Ok(())
    }

    fn merge(&mut self, mut merge_db: MergeDb, config: &MermaidConfig) -> GitGraphResult<()> {
        merge_db.branch = sanitize_text(&merge_db.branch, config);
        if let Some(custom_id) = merge_db.id.as_mut() {
            *custom_id = sanitize_text(custom_id, config);
            if custom_id.is_empty() {
                merge_db.id = None;
            }
        }

        let current_branch = self.curr_branch.clone();
        let other_branch = merge_db.branch.clone();

        let current_commit = self
            .branches
            .get(&current_branch)
            .and_then(|id| id.as_ref())
            .and_then(|id| self.commits.get(id));
        let other_commit = self
            .branches
            .get(&other_branch)
            .and_then(|id| id.as_ref())
            .and_then(|id| self.commits.get(id));

        if let (Some(current_commit), Some(_)) = (current_commit, other_commit)
            && current_commit.branch == other_branch
        {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Cannot merge branch '{}' into itself.",
                other_branch
            )));
        }

        if current_branch == other_branch {
            return Err(GitGraphDiagnostic::fallback(
                "Incorrect usage of \"merge\". Cannot merge a branch to itself".to_string(),
            ));
        }

        let Some(current_commit) = current_commit else {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Incorrect usage of \"merge\". Current branch ({})has no commits",
                current_branch
            )));
        };

        if !self.branches.contains_key(&other_branch) {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({}) does not exist",
                other_branch
            )));
        }

        let Some(other_commit) = other_commit else {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({}) has no commits",
                other_branch
            )));
        };

        if current_commit.id == other_commit.id {
            return Err(GitGraphDiagnostic::fallback(
                "Incorrect usage of \"merge\". Both branches have same head".to_string(),
            ));
        }

        if let Some(custom_id) = merge_db.id.as_ref()
            && self.commits.contains_key(custom_id)
        {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Incorrect usage of \"merge\". Commit with id:{} already exists, use different custom id",
                custom_id
            )));
        }

        let parents = vec![current_commit.id.clone(), other_commit.id.clone()];
        let merge_commit_id = match merge_db.id.clone() {
            Some(id) => id,
            None => {
                let seq = self.seq;
                format!("{seq}-{}", self.next_id())
            }
        };
        let custom_id_flag = merge_db.id.is_some();

        let new_commit = Commit {
            id: merge_commit_id.clone(),
            message: format!("merged branch {} into {}", other_branch, current_branch),
            seq: self.seq,
            commit_type: COMMIT_TYPE_MERGE,
            // Mermaid's merge DB preserves tags verbatim; the SVG writer escapes their text.
            tags: merge_db.tags,
            parents,
            branch: current_branch.clone(),
            custom_type: merge_db.commit_type,
            custom_id: Some(custom_id_flag),
        };
        self.seq += 1;

        self.head = Some(new_commit.id.clone());
        self.commits.insert(new_commit.id.clone(), new_commit);
        self.branches
            .insert(current_branch.clone(), Some(merge_commit_id));
        Ok(())
    }

    fn cherry_pick(&mut self, mut cp: CherryPickDb, config: &MermaidConfig) -> GitGraphResult<()> {
        cp.id = sanitize_text(&cp.id, config);
        cp.parent = sanitize_text(&cp.parent, config);

        if cp.id.is_empty() {
            return Err(GitGraphDiagnostic::fallback(
                "Incorrect usage of \"cherryPick\". Source commit id should exist and provided"
                    .to_string(),
            ));
        }

        let Some(source_commit) = self.commits.get(&cp.id).cloned() else {
            return Err(GitGraphDiagnostic::fallback(
                "Incorrect usage of \"cherryPick\". Source commit id should exist and provided"
                    .to_string(),
            ));
        };
        if !cp.parent.is_empty() && !(source_commit.parents.iter().any(|p| p == &cp.parent)) {
            return Err(GitGraphDiagnostic::fallback("Invalid operation: The specified parent commit is not an immediate parent of the cherry-picked commit.".to_string()));
        }

        if source_commit.commit_type == COMMIT_TYPE_MERGE && cp.parent.is_empty() {
            return Err(GitGraphDiagnostic::fallback("Incorrect usage of cherry-pick: If the source commit is a merge commit, an immediate parent commit must be specified.".to_string()));
        }

        if source_commit.branch == self.curr_branch {
            return Err(GitGraphDiagnostic::fallback(
                "Incorrect usage of \"cherryPick\". Source commit is already on current branch"
                    .to_string(),
            ));
        }

        let current_commit_id = self
            .branches
            .get(&self.curr_branch)
            .cloned()
            .unwrap_or(None);
        if current_commit_id.is_none() {
            return Err(GitGraphDiagnostic::fallback(format!(
                "Incorrect usage of \"cherry-pick\". Current branch ({})has no commits",
                self.curr_branch
            )));
        }

        let tags = match cp.tags {
            Some(t) => t
                .into_iter()
                .map(|v| sanitize_text(&v, config))
                .filter(|tag| !tag.is_empty())
                .collect::<Vec<_>>(),
            None => {
                let mut tag = format!("cherry-pick:{}", source_commit.id);
                if source_commit.commit_type == COMMIT_TYPE_MERGE {
                    tag.push_str(&format!("|parent:{}", cp.parent));
                }
                vec![tag]
            }
        };

        let seq = self.seq;
        let new_id = format!("{seq}-{}", self.next_id());
        let parents = self
            .head
            .as_ref()
            .map(|h| vec![h.clone(), source_commit.id.clone()])
            .unwrap_or_default();
        let commit = Commit {
            id: new_id.clone(),
            message: format!(
                "cherry-picked {} into {}",
                source_commit.message, self.curr_branch
            ),
            seq: self.seq,
            commit_type: COMMIT_TYPE_CHERRY_PICK,
            tags,
            parents,
            branch: self.curr_branch.clone(),
            custom_type: None,
            custom_id: None,
        };
        self.seq += 1;

        self.head = Some(commit.id.clone());
        self.commits.insert(commit.id.clone(), commit);
        self.branches.insert(self.curr_branch.clone(), Some(new_id));

        Ok(())
    }

    fn into_model_controlled(
        self,
        diagram_type: &str,
        control: &OperationControl,
    ) -> OperationControlResult<GitGraphDiagramModel> {
        let branches = self.branches_in_order_controlled(control)?;
        let mut commits = Vec::with_capacity(self.commits.len());
        for commit in self.commits.into_values() {
            control.checkpoint()?;
            commits.push(commit_to_model(commit));
        }
        control.checkpoint()?;
        commits.sort_by_key(|commit| commit.seq);
        control.checkpoint()?;
        Ok(GitGraphDiagramModel {
            diagram_type: diagram_type.to_string(),
            commits,
            branches,
            current_branch: self.curr_branch,
            direction: self.direction,
            title: (!self.title.is_empty()).then_some(self.title),
            acc_title: (!self.acc_title.is_empty()).then_some(self.acc_title),
            acc_descr: (!self.acc_descr.is_empty()).then_some(self.acc_descr),
            warning_facts: self.warning_facts,
        })
    }

    fn branches_in_order_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Vec<GitGraphBranch>> {
        let mut entries: Vec<(String, f64)> = Vec::new();
        for (i, name) in self.branch_config_order.iter().enumerate() {
            control.checkpoint()?;
            let cfg = self.branch_config.get(name);
            let order = cfg.map(|c| c.order);
            let order_f = match order {
                Some(v) => v,
                None => format!("0.{i}").parse::<f64>().unwrap_or(0.0),
            };
            entries.push((name.clone(), order_f));
        }

        control.checkpoint()?;
        entries.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut branches = Vec::with_capacity(entries.len());
        for (name, _) in entries {
            control.checkpoint()?;
            branches.push(GitGraphBranch { name });
        }
        Ok(branches)
    }
}

fn seeded_gitgraph_prng(config: &MermaidConfig) -> Option<XorShift64Star> {
    config
        .get_path(&["gitGraph", "seed"])
        .and_then(|value| value.as_i64())
        .and_then(|v| u64::try_from(v).ok())
        .filter(|v| *v != 0)
        .map(XorShift64Star::new)
}

fn commit_to_model(c: Commit) -> GitGraphCommit {
    GitGraphCommit {
        id: c.id,
        message: c.message,
        seq: c.seq,
        commit_type: c.commit_type,
        tags: c.tags,
        parents: c.parents,
        branch: c.branch,
        custom_type: c.custom_type,
        custom_id: c.custom_id,
    }
}

impl GitGraphCommand {
    fn apply(self, db: &mut GitGraphDb, effective_config: &MermaidConfig) -> GitGraphResult<()> {
        let result = match self.operation {
            GitGraphOperation::Commit(commit) => {
                db.commit(commit, effective_config, self.statement_span);
                Ok(())
            }
            GitGraphOperation::Branch(branch) => db.branch(branch, effective_config),
            GitGraphOperation::Checkout(name) => db.checkout(&name, effective_config),
            GitGraphOperation::Merge(merge) => db.merge(merge, effective_config),
            GitGraphOperation::CherryPick(cherry_pick) => {
                db.cherry_pick(cherry_pick, effective_config)
            }
        };
        result.map_err(|error| error.with_exact_span_if_missing(self.statement_span))
    }
}

fn new_gitgraph_db(generate_id_hex: fn(&str, u64, usize) -> String) -> GitGraphDb {
    GitGraphDb {
        commits: HashMap::new(),
        branches: HashMap::new(),
        branch_config: HashMap::new(),
        branch_config_order: Vec::new(),
        head: None,
        curr_branch: "main".to_string(),
        direction: "LR".to_string(),
        seq: 0,
        warning_facts: Vec::new(),
        title: String::new(),
        acc_title: String::new(),
        acc_descr: String::new(),
        prng: None,
        generate_id_hex,
    }
}

fn apply_git_graph_commands_controlled(
    commands: Vec<GitGraphCommand>,
    db: &mut GitGraphDb,
    effective_config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<GitGraphResult<()>> {
    for command in commands {
        control.checkpoint()?;
        if let Err(error) = command.apply(db, effective_config) {
            return Ok(Err(error));
        }
    }

    Ok(Ok(()))
}

impl GitGraphSyntax {
    /// Replays the validated statements using the facade's operation-scoped ID generator.
    pub fn into_model_controlled(
        self,
        diagram_type: &str,
        effective_config: &MermaidConfig,
        generate_id_hex: fn(&str, u64, usize) -> String,
        control: &OperationControl,
    ) -> OperationControlResult<Result<GitGraphDiagramModel, GitGraphDiagnostic>> {
        let Self {
            direction,
            commands,
            common,
        } = self;
        // Langium keeps the last assignment for each common field, including empty values.
        // The DB's model projection omits empty values.
        let common_value = |field| {
            common
                .last(field)
                .map(|fact| fact.value.clone())
                .unwrap_or_default()
        };
        let acc_descr = common_value(LangiumCommonField::AccDescr);
        let acc_title = common_value(LangiumCommonField::AccTitle);
        let title = common_value(LangiumCommonField::Title);
        let mut db = new_gitgraph_db(generate_id_hex);
        db.clear(effective_config);
        db.acc_descr = acc_descr;
        db.acc_title = acc_title;
        db.title = title;
        if let Some(direction) = direction {
            db.set_direction(&direction);
        }
        if let Err(error) =
            apply_git_graph_commands_controlled(commands, &mut db, effective_config, control)?
        {
            return Ok(Err(error));
        }
        Ok(Ok(db.into_model_controlled(diagram_type, control)?))
    }
}
