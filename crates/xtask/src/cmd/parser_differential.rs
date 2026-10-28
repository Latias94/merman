use crate::XtaskError;
use merman_core::presentation::PresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SCHEMA: u32 = 1;
const MAX_SOURCE_BYTES: usize = 4096;
const ORACLE_BATCH_CASES: usize = 256;
const DEFAULT_SEED: u64 = 0x6d65726d616e;
const SOURCE_FILES: &[&str] = &[
    "packages/mermaid/src/diagrams/flowchart/parser/flow.jison",
    "packages/mermaid/src/diagrams/sequence/parser/sequenceDiagram.jison",
    "packages/mermaid/src/diagrams/flowchart/parser/flowParser.ts",
    "packages/mermaid/src/Diagram.ts",
    "packages/mermaid/src/mermaidAPI.ts",
    "packages/mermaid/src/preprocess.ts",
    "packages/mermaid/src/diagram-api/comments.ts",
];
const FIXTURES: &[(&str, &str)] = &[
    ("flowchart", "flowchart/basic.mmd"),
    ("sequence", "sequence/basic.mmd"),
    ("sequence", "sequence/alt_loop.mmd"),
    ("sequence", "sequence/notes_placements.mmd"),
    ("sequence", "sequence/box_variants.mmd"),
    ("sequence", "sequence/activation_explicit.mmd"),
    ("sequence", "sequence/create_simple.mmd"),
    ("sequence", "sequence/destroy_simple.mmd"),
    ("sequence", "sequence/participant_types.mmd"),
];

fn failure(message: impl Into<String>) -> XtaskError {
    XtaskError::ParserDifferential(message.into())
}
fn read(path: &Path) -> Result<Vec<u8>, XtaskError> {
    fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), XtaskError> {
    fs::write(path, bytes).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

#[derive(Debug)]
struct Options {
    cases: usize,
    seed: u64,
    timeout: Duration,
    replay: Option<PathBuf>,
    minimize: Option<PathBuf>,
    max_attempts: usize,
}
impl Options {
    fn parse(args: Vec<String>) -> Result<Self, XtaskError> {
        let mut result = Self {
            cases: 256,
            seed: DEFAULT_SEED,
            timeout: Duration::from_secs(900),
            replay: None,
            minimize: None,
            max_attempts: 128,
        };
        let mut args = args.into_iter();
        while let Some(key) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| failure(format!("missing value for {key}")))?;
            match key.as_str() {
                "--cases" => {
                    result.cases = value.parse().map_err(|_| failure("invalid case count"))?
                }
                "--seed" => result.seed = value.parse().map_err(|_| failure("invalid seed"))?,
                "--timeout-seconds" => {
                    result.timeout =
                        Duration::from_secs(value.parse().map_err(|_| failure("invalid timeout"))?)
                }
                "--max-attempts" => {
                    result.max_attempts = value
                        .parse()
                        .map_err(|_| failure("invalid reduction bound"))?
                }
                "--replay" => result.replay = Some(value.into()),
                "--minimize" => result.minimize = Some(value.into()),
                _ => return Err(failure(format!("unknown argument {key}"))),
            }
        }
        if result.cases > 5000
            || result.timeout.is_zero()
            || result.timeout > Duration::from_secs(900)
            || result.max_attempts == 0
            || result.max_attempts > 1024
            || (result.replay.is_some() && result.minimize.is_some())
        {
            return Err(failure(
                "bounds: cases <= 5000 per family, timeout 1..900s, reduction 1..1024 attempts; replay and minimize are exclusive",
            ));
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    family: String,
    lane: String,
    site_config: Value,
    source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    seed: u64,
    corpus_sha256: String,
    ordinal: usize,
    mutations: Vec<Mutation>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Mutation {
    offset: usize,
    removed: String,
    inserted: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    id: String,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observation: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    schema_version: u32,
    request_sha256: String,
    artifact_sha256: String,
    browser_version: String,
    decoder_sha256: String,
    decoder_source_sha256: String,
    outcomes: Vec<Outcome>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedMismatch {
    schema_version: u32,
    identity: Value,
    origin: Origin,
    source_sha256: String,
    case: Case,
    mismatch_class: String,
    local: Outcome,
    upstream: Outcome,
}

fn selected_identity() -> Result<Value, XtaskError> {
    super::verify_mermaid_reference(Vec::new())?;
    let root = super::workspace_root();
    let lock_bytes = read(&root.join("tools/upstreams/REPOS.lock.json"))?;
    let lock: Value = serde_json::from_slice(&lock_bytes)?;
    let expected = lock["repos"]["mermaid"]["commit"]
        .as_str()
        .ok_or_else(|| failure("selected source has no commit"))?;
    let checkout = super::mermaid_repo_root();
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&checkout)
        .output()
        .map_err(|error| failure(format!("selected source unavailable: {error}")))?;
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || actual != expected {
        return Err(failure(format!(
            "stale selected source: expected {expected}, found {actual}"
        )));
    }
    let status = Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .args(SOURCE_FILES)
        .current_dir(&checkout)
        .status()
        .map_err(|error| failure(error.to_string()))?;
    if !status.success() {
        return Err(failure(
            "selected grammar/wrapper source differs from selected commit",
        ));
    }
    let mut sources = serde_json::Map::new();
    for relative in SOURCE_FILES {
        sources.insert(
            (*relative).into(),
            Value::String(crate::util::sha256_hex(&read(&checkout.join(relative))?)),
        );
    }
    let harness: serde_json::Map<String, Value> = [
        "tools/mermaid-cli/parser-oracle.mjs",
        "crates/xtask/src/cmd/parser_differential.rs",
    ]
    .into_iter()
    .map(|relative| {
        Ok((
            relative.into(),
            Value::String(crate::util::sha256_hex(&read(&root.join(relative))?)),
        ))
    })
    .collect::<Result<_, XtaskError>>()?;
    Ok(
        json!({ "mermaid_commit": actual, "repos_lock_sha256": crate::util::sha256_hex(&lock_bytes), "reference_bundle_sha256": crate::util::sha256_hex(&read(&root.join("tools/upstreams/MERMAID_REFERENCE_BUNDLE.json"))?), "sources": sources, "harness": harness }),
    )
}

struct Oracle {
    runtime: Value,
    identity: Value,
    root: PathBuf,
    timeout: Duration,
    browser_version: String,
}
impl Oracle {
    fn new(timeout: Duration) -> Result<Self, XtaskError> {
        let mut identity = selected_identity()?;
        let probe =
            super::generate::probe_upstream_svg_render_environment(&super::mermaid_cli_root())?;
        let environment = probe.verified_render_environment()?;
        identity["render_environment"] = serde_json::to_value(&environment)?;
        let runtime = json!({ "artifact_path": probe.runtime_artifact_path(), "artifact_sha256": environment.mermaid_runtime.artifact_sha256, "browser_executable": probe.browser_executable });
        let root = super::target_root().join("parser-differential");
        fs::create_dir_all(&root).map_err(|error| failure(error.to_string()))?;
        let mut command = Command::new("node");
        command
            .arg(super::mermaid_cli_root().join("parser-oracle.mjs"))
            .arg("--prepare-adapter")
            .arg(&root)
            .current_dir(super::mermaid_cli_root())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = super::spawn_timeout_managed_child(&mut command)
            .map_err(|error| failure(error.to_string()))?;
        let output =
            super::wait_with_bounded_output(&mut child, Duration::from_secs(60), 1024 * 1024)?;
        if !output.status.success() {
            return Err(failure(format!(
                "selected decoder compilation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let decoder: Value = serde_json::from_slice(&output.stdout)?;
        identity["decoder"] = decoder.clone();
        let mut runtime = runtime;
        runtime["decoder"] = decoder;
        Ok(Self {
            runtime,
            identity,
            root,
            timeout,
            browser_version: format!(
                "{}/{}",
                environment.browser.product, environment.browser.version
            ),
        })
    }
    fn batch(&self, cases: &[Case], timeout: Duration) -> Result<Vec<Outcome>, XtaskError> {
        let bytes = serde_json::to_vec(
            &json!({ "schema_version": SCHEMA, "runtime": self.runtime, "cases": cases }),
        )?;
        let digest = crate::util::sha256_hex(&bytes);
        let request = self.root.join(format!("request-{digest}.json"));
        write(&request, &bytes)?;
        let mut command = Command::new("node");
        command
            .arg(super::mermaid_cli_root().join("parser-oracle.mjs"))
            .arg(&request)
            .current_dir(super::mermaid_cli_root())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = super::spawn_timeout_managed_child(&mut command)
            .map_err(|error| failure(error.to_string()))?;
        let output = super::wait_with_bounded_output(
            &mut child,
            timeout.min(self.timeout),
            64 * 1024 * 1024,
        )?;
        if !output.status.success() {
            return Err(failure(format!(
                "oracle process failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let response: Response = serde_json::from_slice(&output.stdout)
            .map_err(|error| failure(format!("invalid oracle protocol: {error}")))?;
        validate_response(
            &response,
            cases,
            &digest,
            self.runtime["artifact_sha256"].as_str().unwrap(),
            &self.browser_version,
        )?;
        if response.decoder_sha256 != self.runtime["decoder"]["artifact_sha256"]
            || response.decoder_source_sha256 != self.runtime["decoder"]["source_sha256"]
        {
            return Err(failure("stale decoder observation identity"));
        }
        Ok(response.outcomes)
    }
}
fn validate_response(
    response: &Response,
    cases: &[Case],
    request: &str,
    artifact: &str,
    browser: &str,
) -> Result<(), XtaskError> {
    if response.schema_version != SCHEMA
        || response.request_sha256 != request
        || response.artifact_sha256 != artifact
        || response.browser_version != browser
        || response.outcomes.len() != cases.len()
    {
        return Err(failure(
            "stale or invalid oracle response identity/schema/count",
        ));
    }
    for (outcome, case) in response.outcomes.iter().zip(cases) {
        if outcome.id != case.id
            || !matches!(outcome.status.as_str(), "accepted" | "rejected")
            || (outcome.status == "accepted") != outcome.observation.is_some()
            || (outcome.status == "rejected") != outcome.error.is_some()
        {
            return Err(failure("invalid oracle outcome id/status/payload"));
        }
    }
    Ok(())
}

fn common(model: &Value, title: Option<&str>) -> Value {
    json!({ "title": title, "accTitle": model["accTitle"], "accDescr": model["accDescr"] })
}
fn flow_observation(model: &Value, title: Option<&str>) -> Value {
    let mut out = common(model, title);
    out["direction"] = model["direction"].clone();
    out["nodes"] = model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["provenance"] != "subgraph_anchor")
        .map(|node| {
            let mut node = node.clone();
            node.as_object_mut().unwrap().remove("layoutShape");
            node.as_object_mut().unwrap().remove("provenance");
            node["tooltip"] = model["tooltips"]
                .get(node["id"].as_str().unwrap())
                .cloned()
                .unwrap_or(Value::Null);
            node
        })
        .collect();
    out["edges"] = model["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| {
            let mut edge = edge.clone();
            let object = edge.as_object_mut().unwrap();
            for key in [
                "arrow",
                "startMarker",
                "endMarker",
                "strokeKind",
                "visibility",
            ] {
                object.remove(key);
            }
            if edge["isUserDefinedId"] != true {
                edge["id"] = Value::Null;
            }
            edge
        })
        .collect();
    out["subgraphs"] = model["subgraphs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            let mut group = group.clone();
            group.as_object_mut().unwrap().remove("styles");
            if group.get("metadata").is_none() {
                group["metadata"] = Value::Null;
            }
            group
        })
        .collect();
    out["classDefs"] = model["classDefs"].clone();
    out["edgeDefaults"] = model["edgeDefaults"].clone();
    out
}
fn sequence_observation(model: &Value, title: Option<&str>) -> Value {
    let mut out = common(model, title.or_else(|| model["title"].as_str()));
    for key in [
        "actorOrder",
        "actors",
        "boxes",
        "createdActors",
        "destroyedActors",
    ] {
        out[key] = model[key].clone();
    }
    out["messages"] = model["messages"].as_array().unwrap().iter().map(|message| {
        json!({ "from": message["from"], "to": message["to"], "type": message["type"], "message": message["message"], "wrap": message["wrap"], "activate": message["activate"].as_bool().unwrap_or(false), "placement": message["placement"], "centralConnection": message["centralConnection"].as_i64().unwrap_or(0) })
    }).collect();
    out
}
fn local(case: &Case) -> Result<Outcome, XtaskError> {
    let engine = MermaidEngine::new()
        .with_site_config(PresentationConfig::from_value(case.site_config.clone()));
    match engine.parse_diagram_sync(&case.source, ParseOptions::strict()) {
        Ok(Some(parsed)) => {
            let observation = if case.lane == "public" {
                json!({ "diagramType": parsed.meta.diagram_type, "config": parsed.meta.config.as_value() })
            } else if case.family == "flowchart" {
                flow_observation(&parsed.model, parsed.meta.title.as_deref())
            } else {
                sequence_observation(&parsed.model, parsed.meta.title.as_deref())
            };
            Ok(Outcome {
                id: case.id.clone(),
                status: "accepted".into(),
                observation: Some(observation),
                error: None,
            })
        }
        Ok(None) => Ok(Outcome {
            id: case.id.clone(),
            status: "rejected".into(),
            observation: None,
            error: Some("No detected diagram".into()),
        }),
        Err(
            error @ (merman_core::Error::DiagramParse { .. }
            | merman_core::Error::DetectType(_)
            | merman_core::Error::MalformedFrontMatter
            | merman_core::Error::InvalidDirectiveJson { .. }
            | merman_core::Error::InvalidFrontMatterYaml { .. }),
        ) => Ok(Outcome {
            id: case.id.clone(),
            status: "rejected".into(),
            observation: None,
            error: Some(error.to_string()),
        }),
        Err(error) => Err(failure(format!(
            "local engine failed for {}: {error}",
            case.id
        ))),
    }
}
fn mismatch(local: &Outcome, upstream: &Outcome) -> Option<&'static str> {
    if local.status != upstream.status {
        Some("acceptance")
    } else if local.observation != upstream.observation {
        Some("semantic")
    } else {
        None
    }
}

fn base_case(
    id: impl Into<String>,
    family: &str,
    source: impl Into<String>,
    lane: &str,
    security: &str,
) -> Case {
    Case {
        id: id.into(),
        family: family.into(),
        lane: lane.into(),
        site_config: json!({ "securityLevel": security }),
        source: source.into(),
    }
}
fn corpus() -> Result<(Vec<Case>, String), XtaskError> {
    let mut cases = Vec::new();
    for (family, relative) in FIXTURES {
        let bytes = read(&super::fixtures_root().join(relative))?;
        let source = String::from_utf8(bytes).map_err(|error| failure(error.to_string()))?;
        cases.push(base_case(*relative, family, source, "grammar", "strict"));
    }
    for (id, family, source) in [
        (
            "flow-records",
            "flowchart",
            "flowchart LR\nsubgraph g[Group]\n A[开始] e@-->|Yes| B{Choice}\n A --> B\nend\nclassDef red fill:red,color:blue\nclass A red\nstyle B stroke:red\naccTitle: Diagram title\naccDescr: Diagram description\n",
        ),
        (
            "flow-classdef-repeat",
            "flowchart",
            "graph TD\nclassDef c fill:red\nclassDef c fill:red\n",
        ),
        (
            "flow-classdef-append",
            "flowchart",
            "flowchart TD\nA:::red\nclassDef red fill:red\nclassDef red stroke:blue\n",
        ),
        (
            "flow-entities",
            "flowchart",
            "flowchart TD\r\n%% comment\r\n A[\"a &amp; b #quot;\"]-->B[\"雪\"]",
        ),
        (
            "flow-security",
            "flowchart",
            "flowchart TD\n A[\"<b>Bold</b><script>alert(1)</script>\"]-->B\nclick A href \"javascript:alert(1)\" \"<b>Tip</b>\"\nclick B callback \"tip\"",
        ),
        (
            "seq-entities",
            "sequence",
            "sequenceDiagram\r\n%% comment\r\n A->>B: 雪 &amp; &#35;",
        ),
        (
            "seq-links",
            "sequence",
            "sequenceDiagram\nparticipant A\nlinks A: {\"Docs\":\"https://example.com\"}\nproperties A: {\"role\":\"admin\"}\nA->>B: hello",
        ),
    ] {
        cases.push(base_case(id, family, source, "grammar", "strict"));
    }
    let mut loose = cases
        .iter()
        .find(|case| case.id == "flow-security")
        .unwrap()
        .clone();
    loose.id += "-loose";
    loose.site_config["securityLevel"] = "loose".into();
    cases.push(loose);
    for keyword in [
        "direction",
        "click",
        "default",
        "call",
        "href",
        "accTitle",
        "accDescr",
        "LR",
        "RL",
        "TB",
        "TD",
        "BT",
        "BR",
        "v",
        "end",
        "graph",
        "flowchart",
        "flowchart-elk",
        "swimlane-beta",
        "subgraph",
        "style",
        "class",
        "classDef",
        "linkStyle",
        "interpolate",
        "_self",
        "_blank",
        "_parent",
        "_top",
    ] {
        for suffix in ["%", "%%tail"] {
            for lane in ["grammar", "public"] {
                cases.push(base_case(
                    format!("flow-percent-keyword-{keyword}-{suffix}-{lane}"),
                    "flowchart",
                    format!("flowchart TD\n{keyword}{suffix}-->B"),
                    lane,
                    "strict",
                ));
            }
        }
    }
    for (id, source) in [
        ("comment-line", "flowchart TD\nA-->B\n%% comment\nC-->D"),
        ("comment-indent", "flowchart TD\n \t%% comment\nA-->B"),
        ("comment-eof", "flowchart TD\nA-->B\n%% comment"),
        ("comment-blank-body", "flowchart TD\n%% \nA-->B"),
        ("percent-bare-eof", "flowchart TD\n%%"),
        ("percent-bare-line", "flowchart TD\n%%\nA-->B"),
        ("percent-bare-indent", "flowchart TD\n  %%\n"),
        ("percent-id-start", "flowchart TD\n%-->B"),
        ("percent-id-end", "flowchart TD\nA%-->B"),
        ("percent-id-middle", "flowchart TD\nA%%B-->C"),
        ("percent-inline-target", "flowchart TD\nA-->B%%tail"),
        ("percent-direction-prefix", "flowchart TD\ndirection%-->B"),
        ("percent-click-prefix", "flowchart TD\nclick%%tail-->B"),
        ("percent-reserved-end", "flowchart TD\nend%-->B"),
        ("percent-reserved-style", "flowchart TD\nstyle%-->B"),
        ("percent-header-direction", "flowchart LR%\nA-->B"),
        ("header-inline-node", "flowchart LR A-->B"),
        ("header-space-semicolon", "flowchart LR ;A-->B"),
        ("header-absent-direction-semicolon", "flowchart ;A-->B"),
        (
            "header-inline-direction-statement",
            "flowchart direction LR\nA-->B",
        ),
        ("header-unicode-separator", "flowchart TD開始 --> B"),
        ("header-br-direction", "flowchart BR\nA-->B"),
        ("percent-inline-statement", "flowchart TD\nA-->B; %%tail"),
        ("percent-inline-bare-statement", "flowchart TD\nA-->B; %%"),
        ("comment-inline-spaced", "flowchart TD\nA-->B %% comment"),
        (
            "comment-inline-continued",
            "flowchart TD\nA-->B %% comment\nC-->D",
        ),
        ("percent-inline-bare", "flowchart TD\nA-->B %%"),
        (
            "comment-inline-statement",
            "flowchart TD\nA-->B; %% comment",
        ),
        ("comment-inline-header", "flowchart TD %% comment\nA-->B"),
        ("percent-text", "flowchart TD\nA[100%% done]"),
        ("percent-quoted", "flowchart TD\nA[\"100%% done\"]"),
        ("percent-markdown", "flowchart TD\nA[\"`100%% done`\"]"),
        ("percent-edge", "flowchart TD\nA-->|\"100%% done\"|B"),
        (
            "percent-markdown-edge",
            "flowchart TD\nA-->|\"`100%% done`\"|B",
        ),
        (
            "comment-quoted-line",
            "flowchart TD\nA[\"first\n%% comment\nlast\"]",
        ),
        (
            "comment-markdown-line",
            "flowchart TD\nA[\"`first\n%% comment\nlast`\"]",
        ),
        (
            "percent-crlf",
            "flowchart TD\r\n%% 雪😀\r\n雪%-->B%%tail\r\n",
        ),
    ] {
        for lane in ["grammar", "public"] {
            cases.push(base_case(
                format!("flow-{id}-{lane}"),
                "flowchart",
                source,
                lane,
                "strict",
            ));
        }
    }
    for (id, family, source) in [
        (
            "frontmatter",
            "flowchart",
            "---\ntitle: Example\nconfig:\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\n A-->B",
        ),
        (
            "init",
            "sequence",
            "%%{init: {\"sequence\":{\"wrap\":true}}}%%\nsequenceDiagram\n A->>B: hello",
        ),
        (
            "secure-config",
            "flowchart",
            "%%{init: {\"securityLevel\":\"loose\",\"flowchart\":{\"htmlLabels\":false}}}%%\nflowchart TD\n A-->B",
        ),
        ("invalid-header", "flowchart", "flowchartwhat TD\n A-->B"),
        ("invalid-body", "sequence", "sequenceDiagram\n A->>"),
        (
            "percent-source-config",
            "flowchart",
            "---\nconfig:\n  flowchart:\n    htmlLabels: false\n---\n%%{init: {\"flowchart\":{\"curve\":\"linear\"}}}%%\n%% 雪😀\nflowchart TD\n雪%-->B%%tail",
        ),
        (
            "percent-source-config-rejected",
            "flowchart",
            "---\nconfig:\n  flowchart:\n    htmlLabels: false\n---\n%%{init: {\"flowchart\":{\"curve\":\"linear\"}}}%%\n%% 雪😀\nflowchart TD\n雪%-->B %% comment",
        ),
    ] {
        cases.push(base_case(id, family, source, "public", "strict"));
    }
    let mut custom_secure = base_case(
        "custom-secure",
        "flowchart",
        "%%{init: {\"securityLevel\":\"loose\",\"maxTextSize\":1,\"flowchart\":{\"htmlLabels\":false}}}%%\nflowchart TD\n A-->B",
        "public",
        "strict",
    );
    custom_secure.site_config["secure"] = json!(["securityLevel", "maxTextSize"]);
    cases.push(custom_secure);
    cases.push(base_case("nested-secure", "flowchart", "%%{init: {\"flowchart\":{\"securityLevel\":\"loose\",\"htmlLabels\":false}}}%%\nflowchart TD\n A-->B", "public", "strict"));
    let digest = crate::util::sha256_hex(&serde_json::to_vec(&cases)?);
    Ok((cases, digest))
}

fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
fn apply(source: &str, mutation: &Mutation) -> Result<String, XtaskError> {
    let end = mutation.offset + mutation.removed.len();
    if source.get(mutation.offset..end) != Some(mutation.removed.as_str()) {
        return Err(failure("mutation bytes do not match retained base"));
    }
    Ok(format!(
        "{}{}{}",
        &source[..mutation.offset],
        mutation.inserted,
        &source[end..]
    ))
}
fn mutate(base: &Case, state: &mut u64, ordinal: usize) -> (Case, Mutation) {
    let mut case = base.clone();
    case.id = format!("mutation-{}-{ordinal}", base.family);
    let boundaries: Vec<usize> = base
        .source
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .filter(|index| *index < base.source.len())
        .collect();
    let header_end = base
        .source
        .find('\n')
        .map_or(base.source.len(), |index| index + 1);
    let offset = boundaries
        .get((next(state) as usize) % boundaries.len().max(1))
        .copied()
        .unwrap_or(header_end);
    let line_end = base.source[offset..]
        .find('\n')
        .map_or(base.source.len(), |index| offset + index + 1);
    let line = &base.source[offset..line_end];
    let fragment = if base.family == "flowchart" {
        [
            "  X --> Y\n",
            "  X[\"雪 &amp; text\"]\n",
            "  X -->|again| Y\n",
            "  classDef probe fill:red\n",
        ]
    } else {
        [
            "  X->>Y: message\n",
            "  Note over X,Y: 雪\n",
            "  participant X\n",
            "  X-->>Y: reply\n",
        ]
    };
    let mutation = match next(state) % 5 {
        0 => Mutation {
            offset,
            removed: String::new(),
            inserted: fragment[(next(state) % 4) as usize].into(),
        },
        1 => Mutation {
            offset,
            removed: line.into(),
            inserted: String::new(),
        },
        2 => Mutation {
            offset,
            removed: line.into(),
            inserted: fragment[(next(state) % 4) as usize].into(),
        },
        3 => Mutation {
            offset,
            removed: String::new(),
            inserted: format!("{line}\n"),
        },
        _ => Mutation {
            offset,
            removed: String::new(),
            inserted: "%% mutation comment\n".into(),
        },
    };
    case.source = apply(&base.source, &mutation).expect("mutation owns UTF-8 line boundaries");
    (case, mutation)
}

fn save(
    oracle: &Oracle,
    case: &Case,
    origin: Origin,
    local: Outcome,
    upstream: Outcome,
    class: &str,
) -> Result<PathBuf, XtaskError> {
    let digest = crate::util::sha256_hex(case.source.as_bytes());
    let value = SavedMismatch {
        schema_version: SCHEMA,
        identity: oracle.identity.clone(),
        origin,
        source_sha256: digest.clone(),
        case: case.clone(),
        mismatch_class: class.into(),
        local,
        upstream,
    };
    write(
        &oracle.root.join(format!("{digest}.mmd")),
        case.source.as_bytes(),
    )?;
    let receipt = serde_json::to_vec_pretty(&value)?;
    let path = oracle.root.join(format!(
        "mismatch-{}.json",
        crate::util::sha256_hex(&receipt)
    ));
    write(&path, &receipt)?;
    Ok(path)
}
fn load_saved(path: &Path, oracle: &Oracle) -> Result<SavedMismatch, XtaskError> {
    let saved: SavedMismatch = serde_json::from_slice(&read(path)?)?;
    if saved.schema_version != SCHEMA
        || saved.source_sha256 != crate::util::sha256_hex(saved.case.source.as_bytes())
        || saved.case.source.len() > MAX_SOURCE_BYTES
        || saved.identity["mermaid_commit"] != oracle.identity["mermaid_commit"]
        || saved.identity["reference_bundle_sha256"] != oracle.identity["reference_bundle_sha256"]
        || saved.identity["render_environment"]["mermaid_runtime"]
            != oracle.identity["render_environment"]["mermaid_runtime"]
    {
        return Err(failure(
            "saved mismatch source or selected reference identity is stale",
        ));
    }
    Ok(saved)
}

fn discovery_cases(
    mut cases: Vec<Case>,
    corpus_digest: &str,
    options: &Options,
) -> Result<(Vec<Case>, Vec<Origin>), XtaskError> {
    let mut origins: Vec<_> = (0..cases.len())
        .map(|ordinal| Origin {
            seed: options.seed,
            corpus_sha256: corpus_digest.into(),
            ordinal,
            mutations: Vec::new(),
        })
        .collect();
    for family in ["flowchart", "sequence"] {
        let bases: Vec<_> = cases
            .iter()
            .filter(|case| case.family == family && case.lane == "grammar")
            .cloned()
            .collect();
        let mut state = options.seed;
        for ordinal in 0..options.cases {
            let base = &bases[(next(&mut state) as usize) % bases.len()];
            let (case, mutation) = mutate(base, &mut state, ordinal);
            if case.source.len() > MAX_SOURCE_BYTES {
                return Err(failure("generator exceeded its source bound"));
            }
            cases.push(case);
            origins.push(Origin {
                seed: options.seed,
                corpus_sha256: corpus_digest.into(),
                ordinal,
                mutations: vec![mutation],
            });
        }
    }
    Ok((cases, origins))
}

pub(crate) fn run(args: Vec<String>) -> Result<(), XtaskError> {
    let options = Options::parse(args)?;
    let started = Instant::now();
    let oracle = Oracle::new(options.timeout)?;
    if let Some(path) = options.replay.as_ref().or(options.minimize.as_ref()) {
        let saved = load_saved(path, &oracle)?;
        let upstream = oracle
            .batch(std::slice::from_ref(&saved.case), options.timeout)?
            .remove(0);
        let observed_local = local(&saved.case)?;
        let class = mismatch(&observed_local, &upstream);
        if options.minimize.is_none() {
            println!("replay {}: {}", path.display(), class.unwrap_or("resolved"));
            return class.map_or(Ok(()), |class| {
                Err(failure(format!("retained {class} mismatch")))
            });
        }
        if class != Some(saved.mismatch_class.as_str()) {
            return Err(failure("reduction requires the retained mismatch class"));
        }
        let mut case = saved.case.clone();
        let mut origin = saved.origin.clone();
        let mut attempts = 0;
        let mut chunk = case.source.lines().count().max(1) / 2;
        while attempts < options.max_attempts && started.elapsed() < options.timeout {
            let boundaries: Vec<usize> = std::iter::once(0)
                .chain(case.source.match_indices('\n').map(|(index, _)| index + 1))
                .chain(std::iter::once(case.source.len()))
                .collect();
            let mut changed = false;
            for start in 1..boundaries.len().saturating_sub(1) {
                if attempts >= options.max_attempts || started.elapsed() >= options.timeout {
                    break;
                }
                let end = (start + chunk.max(1)).min(boundaries.len() - 1);
                let mutation = Mutation {
                    offset: boundaries[start],
                    removed: case.source[boundaries[start]..boundaries[end]].into(),
                    inserted: String::new(),
                };
                let mut candidate = case.clone();
                candidate.source = apply(&case.source, &mutation)?;
                attempts += 1;
                let upstream = oracle
                    .batch(
                        std::slice::from_ref(&candidate),
                        options.timeout.saturating_sub(started.elapsed()),
                    )?
                    .remove(0);
                if mismatch(&local(&candidate)?, &upstream) == Some(saved.mismatch_class.as_str()) {
                    case = candidate;
                    origin.mutations.push(mutation);
                    changed = true;
                    break;
                }
            }
            if !changed {
                if chunk <= 1 {
                    break;
                }
                chunk /= 2;
            }
        }
        let header_end = if case.lane == "grammar" {
            case.source
                .find('\n')
                .map_or(case.source.len(), |offset| offset + 1)
        } else {
            0
        };
        let mut width = (case.source.len() / 2).max(1);
        while attempts < options.max_attempts && started.elapsed() < options.timeout {
            let boundaries: Vec<usize> = case
                .source
                .char_indices()
                .map(|(offset, _)| offset)
                .filter(|offset| *offset >= header_end)
                .chain(std::iter::once(case.source.len()))
                .collect();
            let mut changed = false;
            for (index, start) in boundaries
                .iter()
                .enumerate()
                .take(boundaries.len().saturating_sub(1))
            {
                if attempts >= options.max_attempts || started.elapsed() >= options.timeout {
                    break;
                }
                let end = boundaries
                    .iter()
                    .skip(index + 1)
                    .copied()
                    .find(|offset| offset - start >= width)
                    .unwrap_or(case.source.len());
                let mutation = Mutation {
                    offset: *start,
                    removed: case.source[*start..end].into(),
                    inserted: String::new(),
                };
                let mut candidate = case.clone();
                candidate.source = apply(&case.source, &mutation)?;
                attempts += 1;
                let upstream = oracle
                    .batch(
                        std::slice::from_ref(&candidate),
                        options.timeout.saturating_sub(started.elapsed()),
                    )?
                    .remove(0);
                if mismatch(&local(&candidate)?, &upstream) == Some(saved.mismatch_class.as_str()) {
                    case = candidate;
                    origin.mutations.push(mutation);
                    changed = true;
                    break;
                }
            }
            if !changed {
                if width == 1 {
                    break;
                }
                width /= 2;
            }
        }
        let upstream = oracle
            .batch(
                std::slice::from_ref(&case),
                options.timeout.saturating_sub(started.elapsed()),
            )?
            .remove(0);
        let path = save(
            &oracle,
            &case,
            origin,
            local(&case)?,
            upstream,
            &saved.mismatch_class,
        )?;
        println!(
            "reduced in {attempts} attempts to {} bytes: {}",
            case.source.len(),
            path.display()
        );
        return Ok(());
    }
    let (cases, corpus_digest) = corpus()?;
    write(
        &oracle.root.join(format!("corpus-{corpus_digest}.json")),
        &serde_json::to_vec_pretty(&cases)?,
    )?;
    let (cases, origins) = discovery_cases(cases, &corpus_digest, &options)?;
    let mut mismatches = Vec::new();
    let mut accepted = 0;
    let mut rejected = 0;
    for (case_batch, origin_batch) in cases
        .chunks(ORACLE_BATCH_CASES)
        .zip(origins.chunks(ORACLE_BATCH_CASES))
    {
        let remaining = options.timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(failure("parser differential command deadline exceeded"));
        }
        let outcomes = oracle.batch(case_batch, remaining)?;
        for ((case, origin), upstream) in case_batch.iter().zip(origin_batch).zip(outcomes) {
            if started.elapsed() >= options.timeout {
                return Err(failure("parser differential command deadline exceeded"));
            }
            if upstream.status == "accepted" {
                accepted += 1;
            } else {
                rejected += 1;
            }
            let local = local(case)?;
            if let Some(class) = mismatch(&local, &upstream) {
                mismatches.push(save(&oracle, case, origin.clone(), local, upstream, class)?);
            }
        }
    }
    let report = json!({ "schema_version": SCHEMA, "identity": oracle.identity, "corpus_sha256": corpus_digest, "case_manifest_sha256": crate::util::sha256_hex(&serde_json::to_vec(&cases)?), "seed": options.seed, "mutations_per_family": options.cases, "total": cases.len(), "upstream_accepted": accepted, "upstream_rejected": rejected, "mismatches": mismatches, "elapsed_seconds": started.elapsed().as_secs_f64() });
    write(
        &oracle.root.join("report.json"),
        &serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "parser differential: {} cases ({accepted} accepted, {rejected} rejected), {} mismatches; {}",
        cases.len(),
        mismatches.len(),
        oracle.root.join("report.json").display()
    );
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(failure(format!(
            "{} actual mismatches retained",
            mismatches.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_discovery_batches_retain_the_entire_corpus_and_both_family_budgets() {
        let options = Options::parse(vec!["--cases".into(), "5000".into()]).unwrap();
        let (corpus, digest) = corpus().unwrap();
        let corpus_len = corpus.len();
        let (cases, origins) = discovery_cases(corpus, &digest, &options).unwrap();
        assert_eq!(cases.len(), corpus_len + 2 * options.cases);
        assert_eq!(origins.len(), cases.len());
        assert!(
            cases.len() > 10_064,
            "retain the historical oracle overflow"
        );
        for family in ["flowchart", "sequence"] {
            assert_eq!(
                cases[corpus_len..]
                    .iter()
                    .filter(|case| case.family == family)
                    .count(),
                options.cases
            );
        }
        let mut visited = Vec::new();
        for (batch, origins) in cases
            .chunks(ORACLE_BATCH_CASES)
            .zip(origins.chunks(ORACLE_BATCH_CASES))
        {
            assert!(!batch.is_empty() && batch.len() <= ORACLE_BATCH_CASES);
            assert_eq!(batch.len(), origins.len());
            for (case, origin) in batch.iter().zip(origins) {
                assert_eq!(origin.corpus_sha256, digest);
                assert!(case.source.len() <= MAX_SOURCE_BYTES);
                visited.push(case.id.as_str());
            }
        }
        assert_eq!(
            visited,
            cases
                .iter()
                .map(|case| case.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            visited
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            cases.len()
        );
    }

    #[test]
    fn saved_mismatch_replays_exact_source_independently_of_generator_history() {
        let directory = tempfile::tempdir().unwrap();
        let mut oracle = Oracle {
            runtime: Value::Null,
            identity: json!({
                "mermaid_commit": "selected-commit",
                "reference_bundle_sha256": "selected-bundle",
                "render_environment": {"mermaid_runtime": {"artifact_sha256": "runtime"}},
                "harness": {"generator": "old"},
            }),
            root: directory.path().to_path_buf(),
            timeout: Duration::from_secs(1),
            browser_version: "unused-by-persistence".into(),
        };
        let case = base_case(
            "persisted",
            "flowchart",
            "flowchart TD\r\n A[雪] --> B[λ]\r\n",
            "grammar",
            "strict",
        );
        let digest = crate::util::sha256_hex(case.source.as_bytes());
        let observed = local(&case).unwrap();
        let path = save(
            &oracle,
            &case,
            Origin {
                seed: DEFAULT_SEED,
                corpus_sha256: "old-corpus".into(),
                ordinal: 999,
                mutations: Vec::new(),
            },
            observed,
            Outcome {
                id: case.id.clone(),
                status: "rejected".into(),
                observation: None,
                error: Some("persistence fixture".into()),
            },
            "acceptance",
        )
        .unwrap();
        assert_eq!(
            read(&directory.path().join(format!("{digest}.mmd"))).unwrap(),
            case.source.as_bytes()
        );
        oracle.identity["harness"]["generator"] = json!("new");
        let saved = load_saved(&path, &oracle).unwrap();
        assert_eq!(saved.case.source.as_bytes(), case.source.as_bytes());
        assert_eq!(saved.source_sha256, digest);
        assert_eq!(saved.origin.corpus_sha256, "old-corpus");
        assert_eq!(saved.mismatch_class, "acceptance");
        assert_eq!(
            mismatch(&local(&saved.case).unwrap(), &saved.upstream),
            Some("acceptance")
        );

        oracle.identity["render_environment"]["mermaid_runtime"]["artifact_sha256"] =
            json!("different-runtime");
        assert!(load_saved(&path, &oracle).is_err());
        oracle.identity["render_environment"]["mermaid_runtime"]["artifact_sha256"] =
            json!("runtime");
        let mut corrupt: Value = serde_json::from_slice(&read(&path).unwrap()).unwrap();
        corrupt["case"]["source"] = json!("flowchart TD\n A --> C");
        write(&path, &serde_json::to_vec(&corrupt).unwrap()).unwrap();
        assert!(load_saved(&path, &oracle).is_err());
    }

    #[test]
    fn replay_and_reduction_options_enforce_explicit_bounds() {
        for attempts in [1, 1024] {
            let options = Options::parse(vec![
                "--minimize".into(),
                "saved.json".into(),
                "--max-attempts".into(),
                attempts.to_string(),
            ])
            .unwrap();
            assert_eq!(options.max_attempts, attempts);
        }
        for attempts in [0, 1025] {
            assert!(Options::parse(vec!["--max-attempts".into(), attempts.to_string()]).is_err());
        }
        assert!(
            Options::parse(vec![
                "--replay".into(),
                "saved.json".into(),
                "--minimize".into(),
                "saved.json".into(),
            ])
            .is_err()
        );
    }

    #[test]
    fn mutation_replay_preserves_header_unicode_and_exact_bytes() {
        for family in ["flowchart", "sequence"] {
            let base = base_case(
                "seed",
                family,
                format!(
                    "{}\r\n A[雪]\r\n B\n",
                    if family == "flowchart" {
                        "flowchart TD"
                    } else {
                        "sequenceDiagram"
                    }
                ),
                "grammar",
                "strict",
            );
            let mut state = DEFAULT_SEED;
            for ordinal in 0..256 {
                let (case, mutation) = mutate(&base, &mut state, ordinal);
                assert_eq!(apply(&base.source, &mutation).unwrap(), case.source);
                assert_eq!(case.source.lines().next(), base.source.lines().next());
            }
        }
    }
    #[test]
    fn acceptance_and_real_endpoint_changes_are_mismatches() {
        let one = base_case(
            "one",
            "flowchart",
            "flowchart TD\n A-->B",
            "grammar",
            "strict",
        );
        let mut two = one.clone();
        two.source = "flowchart TD\n A-->C".into();
        assert_eq!(
            mismatch(&local(&one).unwrap(), &local(&two).unwrap()),
            Some("semantic")
        );
        two.source = "flowchart TD\n A[".into();
        assert_eq!(
            mismatch(&local(&one).unwrap(), &local(&two).unwrap()),
            Some("acceptance")
        );
    }
    #[test]
    fn response_rejects_stale_identity_unknown_schema_and_malformed_rows() {
        let cases = vec![base_case(
            "a",
            "flowchart",
            "flowchart TD\n A-->B",
            "grammar",
            "strict",
        )];
        let mut response = Response {
            schema_version: 1,
            request_sha256: "request".into(),
            artifact_sha256: "artifact".into(),
            browser_version: "HeadlessChrome/test".into(),
            decoder_sha256: "decoder".into(),
            decoder_source_sha256: "source".into(),
            outcomes: vec![local(&cases[0]).unwrap()],
        };
        assert!(
            validate_response(
                &response,
                &cases,
                "request",
                "artifact",
                "HeadlessChrome/test"
            )
            .is_ok()
        );
        response.schema_version = 2;
        assert!(
            validate_response(
                &response,
                &cases,
                "request",
                "artifact",
                "HeadlessChrome/test"
            )
            .is_err()
        );
        response.schema_version = 1;
        response.browser_version = "HeadlessChrome/changed".into();
        assert!(
            validate_response(
                &response,
                &cases,
                "request",
                "artifact",
                "HeadlessChrome/test"
            )
            .is_err()
        );
        response.browser_version = "HeadlessChrome/test".into();
        assert!(
            validate_response(
                &response,
                &cases,
                "changed",
                "artifact",
                "HeadlessChrome/test"
            )
            .is_err()
        );
        response.outcomes[0].status = "timeout".into();
        assert!(
            validate_response(
                &response,
                &cases,
                "request",
                "artifact",
                "HeadlessChrome/test"
            )
            .is_err()
        );
    }
}
