//! One child process observes one ordered workload. Verification runs outside the observation.

mod topology;

use merman::svg::{RenderResourcePolicy, RenderResourceProfile, SvgGeneration, SvgPipeline};
use merman::{
    MermaidEngine, OperationControl, ParseOptions, PresentationArtifact, RenderOutput,
    RenderRequest, Renderer, SourcePresentationConfig, SvgRequest,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{hint::black_box, io, time::Instant};

#[derive(Deserialize)]
struct Request {
    source: String,
    diagram_id: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Operation {
    SvgData,
    StaticSvg,
    ResvgSvg,
    Svg,
    LayoutJson,
    PrepareDrop,
    PrepareRetained,
}

impl Operation {
    fn svg_request(self, input: &Request, resources: Option<RenderResourcePolicy>) -> SvgRequest {
        let mut request = SvgRequest::default();
        request.options.diagram_id = Some(input.diagram_id.clone());
        if let Some(resources) = resources {
            request.environment = request.environment.with_resource_policy(resources);
        }
        request.pipeline = match self {
            Self::StaticSvg => Some(SvgPipeline::parity().with_generation(SvgGeneration::Static)),
            Self::ResvgSvg => {
                Some(SvgPipeline::resvg_safe().with_generation(SvgGeneration::Static))
            }
            _ => None,
        };
        request
    }
}

#[derive(Deserialize)]
#[serde(tag = "task", rename_all = "lowercase")]
enum Job {
    Measure {
        metric: String,
        operation: Operation,
        requests: Vec<Request>,
        warmups: usize,
        site: Option<Value>,
        resource_profile: Option<String>,
        #[serde(default)]
        verify_only: bool,
        #[serde(default)]
        verify_layout: bool,
        #[serde(default)]
        lock_site_layout: bool,
        #[serde(default)]
        return_measured_svg: bool,
    },
    Topology {
        source: String,
        nodes: usize,
        edges: usize,
    },
}

enum Product {
    Data(String),
    Layout(Vec<u8>),
    Prepared(PresentationArtifact),
}

fn execute(
    renderer: &Renderer,
    input: &Request,
    operation: Operation,
    resources: Option<RenderResourcePolicy>,
) -> Result<Product, String> {
    let control = OperationControl::new();
    let source = &input.source;
    let empty = || format!("{} returned no diagram", input.diagram_id);
    match operation {
        Operation::SvgData | Operation::StaticSvg | Operation::ResvgSvg => renderer
            .render_svg_data(RenderRequest::svg(
                source,
                control,
                operation.svg_request(input, resources),
            ))
            .map_err(|e| e.to_string())?
            .map(Product::Data)
            .ok_or_else(empty),
        Operation::Svg => match renderer
            .render(RenderRequest::svg(
                source,
                control,
                operation.svg_request(input, resources),
            ))
            .map_err(|e| e.to_string())?
        {
            RenderOutput::Svg(Some(output)) => {
                let (svg, evidence) = output.into_parts();
                black_box(evidence);
                Ok(Product::Data(svg))
            }
            _ => Err(empty()),
        },
        Operation::LayoutJson => {
            let mut bytes = Vec::new();
            let present = renderer
                .write_layout_json(
                    RenderRequest::layout_json(
                        source,
                        control,
                        operation.svg_request(input, resources),
                    ),
                    &mut bytes,
                    false,
                )
                .map_err(|e| e.to_string())?;
            present.then_some(Product::Layout(bytes)).ok_or_else(empty)
        }
        Operation::PrepareDrop | Operation::PrepareRetained => renderer
            .prepare_presentation(source, control)
            .map_err(|e| e.to_string())?
            .map(Product::Prepared)
            .ok_or_else(empty),
    }
}

fn identity(bytes: &[u8]) -> Value {
    json!({"bytes": bytes.len(), "sha256": data_encoding::HEXLOWER.encode(&Sha256::digest(bytes))})
}

impl Product {
    fn result_size(&self) -> usize {
        match self {
            Self::Data(svg) => svg.len(),
            Self::Layout(bytes) => bytes.len(),
            Self::Prepared(artifact) => artifact.diagram_type().len(),
        }
    }

    fn verification_output(self, verify_layout: bool) -> Result<Value, String> {
        let size = self.result_size();
        let mut verified = match self {
            // The driver validates complete SVG text outside the measurement interval.
            Self::Data(svg) => return Ok(Value::String(svg)),
            Self::Layout(bytes) => {
                let parsed: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                if parsed.get("layout").is_none() {
                    return Err("missing layout in JSON".into());
                }
                let mut signature = identity(&bytes);
                if verify_layout {
                    signature["layout_json"] = parsed;
                }
                signature
            }
            Self::Prepared(artifact) => {
                // Hash the public preparation result without applying a second target's export policy.
                let meta = artifact.metadata();
                let metadata = serde_json::to_vec(&(
                    artifact.semantic_kind(),
                    &meta.diagram_type,
                    &meta.title,
                    meta.config.as_value(),
                    meta.effective_config.as_value(),
                    meta.effective_config
                        .explicit_config_paths()
                        .collect::<Vec<_>>(),
                ))
                .map_err(|e| e.to_string())?;
                identity(&metadata)
            }
        };
        verified["observed_size"] = json!(size);
        Ok(verified)
    }
}

pub(super) struct Trial<'a> {
    renderer: Renderer,
    operation: Operation,
    resources: Option<RenderResourcePolicy>,
    inputs: &'a [Request],
    retained: Vec<PresentationArtifact>,
    sizes: Vec<usize>,
}

impl Trial<'_> {
    pub(super) fn batch(&mut self) -> Result<(), String> {
        self.sizes.clear();
        for input in self.inputs {
            let product = black_box(execute(
                &self.renderer,
                input,
                self.operation,
                self.resources,
            )?);
            self.sizes.push(product.result_size());
            if matches!(self.operation, Operation::PrepareRetained)
                && let Product::Prepared(artifact) = product
            {
                self.retained.push(artifact);
            }
        }
        Ok(())
    }

    pub(super) fn release(&mut self) {
        self.retained.clear();
    }

    fn verify(&self, verify_layout: bool) -> Result<Vec<Value>, String> {
        let products = self
            .inputs
            .iter()
            .map(|input| execute(&self.renderer, input, self.operation, self.resources));
        let verify = |result: Result<Product, String>| match result {
            Ok(product) => product.verification_output(verify_layout),
            Err(message) => Ok(json!({"render_error": message})),
        };
        if matches!(self.operation, Operation::PrepareRetained) {
            // Match the observed retention policy before consuming the artifacts for verification.
            let retained: Vec<_> = products.collect();
            return retained.into_iter().map(verify).collect();
        }
        products.map(verify).collect()
    }

    fn layout_evidence(
        &self,
        input: &Request,
        output: &Value,
        #[cfg(feature = "benchmark-stages")] stage_layout: Option<Option<&str>>,
    ) -> Result<Value, String> {
        if output.get("render_error").is_some() {
            return Ok(Value::Null);
        }
        let request = self.operation.svg_request(input, self.resources);
        let control = OperationControl::new();
        let (family, required, work_units) = if matches!(self.operation, Operation::LayoutJson) {
            // The layout writer returns bytes. Query the public planner with the same input.
            let RenderOutput::SvgPlan(Some(plan)) = self
                .renderer
                .render(RenderRequest::svg_plan(&input.source, control, request))
                .map_err(|e| e.to_string())?
            else {
                return Err("missing layout capability plan".into());
            };
            if !plan.is_ready() {
                return Err(
                    "layout writer succeeded with an unavailable planned capability".into(),
                );
            }
            (
                plan.family_id(),
                plan.required_capabilities().to_vec(),
                None,
            )
        } else {
            let RenderOutput::Svg(Some(svg)) = self
                .renderer
                .render(RenderRequest::svg(&input.source, control, request))
                .map_err(|e| e.to_string())?
            else {
                return Err("missing completed SVG layout evidence".into());
            };
            if output.as_str() != Some(svg.svg()) {
                return Err(format!(
                    "{}: SVG differs from its layout witness",
                    input.diagram_id
                ));
            }
            let evidence = svg.evidence();
            (
                evidence.family_id(),
                evidence.required_capabilities().to_vec(),
                Some(evidence.layout_work_units()),
            )
        };
        let mut receipt = json!({
            "diagram_id": input.diagram_id, "family": family.as_str(),
            "source_sha256": data_encoding::HEXLOWER.encode(&Sha256::digest(input.source.as_bytes())),
            "required_capabilities": required.iter().map(|c| c.id()).collect::<Vec<_>>(),
        });
        if let Some(work_units) = work_units {
            receipt["layout_work_units"] = json!(work_units);
        }
        #[cfg(feature = "benchmark-stages")]
        if let Some(requested) = stage_layout {
            // Layout capabilities alone cannot tell family-native, Swimlane, and TidyTree apart.
            // Keep the plan tied to the separately completed SVG witness above.
            let plan_request = RenderRequest::svg_plan(
                &input.source,
                OperationControl::new(),
                self.operation.svg_request(input, self.resources),
            );
            let RenderOutput::SvgPlan(Some(plan)) = self
                .renderer
                .render(plan_request)
                .map_err(|error| error.to_string())?
            else {
                return Err("missing stage layout capability plan".into());
            };
            if !plan.is_ready()
                || plan.family_id() != family
                || plan.required_capabilities() != required.as_slice()
            {
                return Err(format!(
                    "{}: stage plan differs from its SVG witness",
                    input.diagram_id
                ));
            }
            let (backend, fixed) = plan.benchmark_layout_witness(requested);
            receipt["backend"] = json!(backend);
            receipt["fixed_backend"] = json!(fixed);
        }
        Ok(receipt)
    }

    fn measure_returned_svg(&self) -> Result<Value, String> {
        let [input] = self.inputs else {
            return Err("returned-SVG timing requires exactly one request".into());
        };
        let request = RenderRequest::svg(
            &input.source,
            OperationControl::new(),
            self.operation.svg_request(input, self.resources),
        );
        let (elapsed, svg) = match self.operation {
            Operation::Svg => {
                let start = Instant::now();
                let output = black_box(self.renderer.render(request));
                let elapsed = start.elapsed();
                let RenderOutput::Svg(Some(output)) = output.map_err(|e| e.to_string())? else {
                    return Err("render returned no SVG".into());
                };
                (elapsed, output.into_parts().0)
            }
            Operation::SvgData | Operation::StaticSvg | Operation::ResvgSvg => {
                let start = Instant::now();
                let output = black_box(self.renderer.render_svg_data(request));
                let elapsed = start.elapsed();
                (
                    elapsed,
                    output
                        .map_err(|e| e.to_string())?
                        .ok_or("render returned no SVG")?,
                )
            }
            _ => return Err("returned-SVG timing requires an SVG operation".into()),
        };
        Ok(json!({"metrics": {"elapsed_ns": elapsed.as_nanos() as u64}, "outputs": [svg]}))
    }
}

pub(super) fn run(
    metric: &str,
    measure: impl FnOnce(&mut Trial<'_>) -> Result<Value, String>,
) -> Result<(), String> {
    let job = serde_json::from_reader(io::stdin().lock()).map_err(|e| e.to_string())?;
    let renderer = Renderer::new().with_parse_options(ParseOptions::strict());
    let mut row = match job {
        Job::Measure {
            metric: requested,
            operation,
            requests,
            warmups,
            mut site,
            resource_profile,
            verify_only,
            verify_layout,
            lock_site_layout,
            return_measured_svg,
        } => {
            if requested != metric || requests.is_empty() {
                return Err("wrong runner or empty workload".into());
            }
            if return_measured_svg && (metric != "clock" || verify_only) {
                return Err("returned-SVG timing requires a clock measurement".into());
            }
            if lock_site_layout {
                if metric != "clock"
                    || !verify_only
                    || !verify_layout
                    || !matches!(operation, Operation::Svg)
                {
                    return Err("locking the site layout requires untimed SVG verification".into());
                }
                let site = site
                    .as_mut()
                    .and_then(Value::as_object_mut)
                    .filter(|site| site.get("layout").and_then(Value::as_str).is_some())
                    .ok_or("locking the site layout requires a site layout name")?;
                let secure = site
                    .entry("secure")
                    .or_insert_with(|| merman::default_site_config().as_value()["secure"].clone());
                let keys = secure
                    .as_array_mut()
                    .ok_or("site secure keys must be an array")?;
                keys.push(json!("layout"));
            }
            #[cfg(feature = "benchmark-stages")]
            let stage_layout = (verify_only
                && verify_layout
                && matches!(metric, "stages" | "stage-memory"))
            .then(|| {
                site.as_ref()
                    .and_then(|site| site["layout"].as_str())
                    .map(str::to_owned)
            });
            let renderer = match site {
                Some(site) => renderer.with_presentation_engine(
                    MermaidEngine::new()
                        .with_exact_site_config(Some(SourcePresentationConfig::from_value(site))),
                ),
                None => renderer,
            };
            let resources = resource_profile
                .as_deref()
                .map(str::parse::<RenderResourceProfile>)
                .transpose()?
                .map(RenderResourcePolicy::for_profile);
            let renderer = match resources {
                Some(policy) => renderer.with_resource_policy(*policy.input_policy()),
                None => renderer,
            };
            let retained = if matches!(operation, Operation::PrepareRetained) {
                Vec::with_capacity(requests.len())
            } else {
                Vec::new()
            };
            let mut trial = Trial {
                renderer,
                operation,
                resources,
                inputs: &requests,
                retained,
                sizes: Vec::with_capacity(requests.len()),
            };
            let mut response = if verify_only {
                for _ in 0..warmups {
                    trial.verify(false)?;
                }
                let outputs = trial.verify(verify_layout)?;
                let mut verified = json!({});
                if verify_layout {
                    let evidence = requests
                        .iter()
                        .zip(&outputs)
                        .map(|(input, output)| {
                            trial.layout_evidence(
                                input,
                                output,
                                #[cfg(feature = "benchmark-stages")]
                                stage_layout.as_ref().map(Option::as_deref),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    verified["layout_evidence"] = Value::Array(evidence);
                }
                if lock_site_layout {
                    verified["site_layout_locked"] = json!(true);
                }
                verified["outputs"] = Value::Array(outputs);
                verified
            } else {
                for _ in 0..warmups {
                    trial.batch()?;
                    trial.release();
                }
                if return_measured_svg {
                    trial.measure_returned_svg()?
                } else {
                    let metrics = measure(&mut trial)?;
                    json!({"metrics": metrics, "sizes": trial.sizes})
                }
            };
            if let Some(policy) = resources {
                response["resource_profile"] = json!(policy.profile().id());
            }
            response
        }
        Job::Topology {
            source,
            nodes,
            edges,
        } => topology::observe(&renderer, &source, nodes, edges)?,
    };
    // Historical builds can have the same package version and different measurement code.
    let mut harness = Sha256::new();
    for file in [
        include_bytes!("mod.rs").as_slice(),
        include_bytes!("topology.rs"),
        include_bytes!("../workloads.rs"),
        include_bytes!("../memory.rs"),
        include_bytes!("../memory/mod.rs"),
        include_bytes!("../memory/workload.rs"),
        include_bytes!("../memory/stages.rs"),
        include_bytes!("../native_memory/allocator.rs"),
    ] {
        harness.update((file.len() as u64).to_le_bytes());
        harness.update(file);
    }
    row["harness_sha256"] = json!(data_encoding::HEXLOWER.encode(&harness.finalize()));
    println!("{row}");
    Ok(())
}
