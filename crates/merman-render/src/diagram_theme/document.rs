use std::collections::BTreeMap;

use merman_theme_contract::{
    DiagramThemeSpecWireV1, MaterializedThemeRecipeV1, ThemeAppearance,
    ThemeAppearanceInspectionV1, ThemeAppearanceOverridesV1, ThemeDesignReferenceUse,
    ThemeDesignRuleV1, ThemeDesignV1, ThemeMaterializationDiagnosticV1,
    ThemeMaterializationErrorV1, ThemeRecipeInspectionV1, ThemeRecipeReferenceUseV1, ThemeRecipeV1,
};

use super::definition_admission::ThemeInputEncodingError;
use super::source::ThemeWireSources;
use super::{DiagramTheme, DiagramThemeCompiler, ThemeDesignCompileError};

struct ComposedDesign {
    design: ThemeDesignV1,
    prefix: String,
    context: String,
    locations: BTreeMap<String, String>,
}

impl ComposedDesign {
    fn shared(design: &ThemeDesignV1, prefix: &str) -> Self {
        let mut design = design.clone();
        design.appearances = None;
        design.default_appearance = None;
        Self {
            design,
            prefix: prefix.to_owned(),
            context: prefix.to_owned(),
            locations: BTreeMap::new(),
        }
    }

    fn with_appearance(
        shared: &ThemeDesignV1,
        appearance: ThemeAppearance,
        local: &ThemeAppearanceOverridesV1,
    ) -> Self {
        let mut composed = Self::shared(shared, "/design");
        let prefix = format!("/design/appearances/{appearance}");
        composed.context.clone_from(&prefix);
        for (id, token) in &local.tokens {
            let key = format!("/tokens/{}", escape(id));
            composed
                .locations
                .insert(key.clone(), format!("{prefix}{key}"));
            composed.design.tokens.insert(id.clone(), token.clone());
        }
        for (target, style) in &local.styles {
            let index = composed.design.rules.len();
            let path = format!("{prefix}/styles/{}", escape(target));
            composed
                .locations
                .insert(format!("/rules/{index}/style"), path.clone());
            composed
                .locations
                .insert(format!("/rules/{index}/target"), path.clone());
            composed.locations.insert(format!("/rules/{index}"), path);
            composed.design.rules.push(ThemeDesignRuleV1 {
                target: target.clone(),
                family: None,
                variant: None,
                ordinal: None,
                style: style.clone(),
            });
        }
        for (local_index, rule) in local.rules.iter().enumerate() {
            let index = composed.design.rules.len();
            composed.locations.insert(
                format!("/rules/{index}"),
                format!("{prefix}/rules/{local_index}"),
            );
            composed.design.rules.push(rule.clone());
        }
        for (target, palette) in &local.palettes {
            let key = format!("/palettes/{}", escape(target));
            composed
                .locations
                .insert(key.clone(), format!("{prefix}{key}"));
            composed
                .design
                .palettes
                .insert(target.clone(), palette.clone());
        }
        if let Some(canvas) = &local.canvas {
            composed
                .locations
                .insert("/canvas".to_owned(), format!("{prefix}/canvas"));
            composed.design.canvas = Some(canvas.clone());
        }
        if let Some(typography) = &local.typography {
            composed
                .locations
                .insert("/typography".to_owned(), format!("{prefix}/typography"));
            composed.design.typography = Some(typography.clone());
        }
        if let Some(mermaid) = &local.mermaid {
            let merged = composed.design.mermaid.get_or_insert_with(Default::default);
            if let Some(theme) = &mermaid.theme {
                merged.theme = Some(theme.clone());
                composed.locations.insert(
                    "/mermaid/theme".to_owned(),
                    format!("{prefix}/mermaid/theme"),
                );
            }
            if let Some(dark_mode) = mermaid.dark_mode {
                merged.dark_mode = Some(dark_mode);
                composed.locations.insert(
                    "/mermaid/dark_mode".to_owned(),
                    format!("{prefix}/mermaid/dark_mode"),
                );
            }
            if let Some(variables) = &mermaid.variables {
                let merged_variables = merged.variables.get_or_insert_with(Default::default);
                for (name, value) in variables {
                    let key = format!("/mermaid/variables/{}", escape(name));
                    composed
                        .locations
                        .insert(key.clone(), format!("{prefix}{key}"));
                    merged_variables.insert(name.clone(), value.clone());
                }
            }
        }
        composed
    }

    fn path(&self, path: &str) -> String {
        let mut candidate = path;
        loop {
            if let Some(source) = self.locations.get(candidate) {
                return bounded_path(format!("{source}{}", &path[candidate.len()..]));
            }
            let Some((parent, _)) = candidate.rsplit_once('/') else {
                break;
            };
            if parent.is_empty() {
                break;
            }
            candidate = parent;
        }
        bounded_path(format!("{}{path}", self.prefix))
    }

    fn reference(&self, reference: ThemeDesignReferenceUse) -> ThemeRecipeReferenceUseV1 {
        ThemeRecipeReferenceUseV1 {
            token_path: self.path(&format!("/tokens/{}", escape(&reference.token))),
            path: self.path(&reference.path),
            token: reference.token,
            domain: reference.domain,
        }
    }

    fn sources(&self) -> ThemeWireSources {
        let mut sources = ThemeWireSources::for_design(&self.design);
        if let ThemeWireSources::Design {
            styles,
            canvas_base,
        } = &mut sources
        {
            for source in styles.iter_mut().chain(canvas_base.iter_mut()) {
                for path in &mut source.paths {
                    *path = self.path(path);
                }
            }
        }
        sources
    }

    fn error(&self, error: ThemeMaterializationErrorV1) -> ThemeMaterializationErrorV1 {
        let path = if error.diagnostic().path().is_empty() {
            self.context.clone()
        } else {
            self.path(error.diagnostic().path())
        };
        ThemeMaterializationErrorV1::from_diagnostic(error.into_diagnostic().with_path(path))
    }
}

struct SelectedDocument {
    theme: DiagramTheme,
    spec: DiagramThemeSpecWireV1,
    appearance: Option<ThemeAppearance>,
    references: Vec<ThemeRecipeReferenceUseV1>,
}

struct AdmittedDocument {
    inspection: ThemeRecipeInspectionV1,
    selected: Option<SelectedDocument>,
}

impl DiagramThemeCompiler {
    /// Compiles a portable document after admitting every declared appearance.
    ///
    /// Omitted appearance uses the authored default. Explicit appearances on unclassified
    /// concrete recipes are rejected. Host preference must already have been resolved.
    pub fn compile_document(
        &self,
        recipe: ThemeRecipeV1,
        appearance: Option<ThemeAppearance>,
    ) -> Result<DiagramTheme, ThemeDesignCompileError> {
        self.admit_document(&recipe, appearance, true)
            .map(|admitted| admitted.selected.expect("selected document admitted").theme)
    }

    /// Inspects all declared bindings after admitting the entire document without a host mode.
    pub fn inspect_document(
        &self,
        recipe: &ThemeRecipeV1,
    ) -> Result<ThemeRecipeInspectionV1, ThemeDesignCompileError> {
        self.admit_document(recipe, None, false)
            .map(|admitted| admitted.inspection)
    }

    /// Materializes one concrete appearance after admitting the complete editable document.
    pub fn materialize_document(
        &self,
        recipe: &ThemeRecipeV1,
        appearance: Option<ThemeAppearance>,
    ) -> Result<MaterializedThemeRecipeV1, ThemeDesignCompileError> {
        let admitted = self.admit_document(recipe, appearance, true)?;
        let selected = admitted.selected.expect("selected document admitted");
        Ok(MaterializedThemeRecipeV1::new(
            &admitted.inspection,
            selected.appearance,
            selected.spec,
            selected.references,
        ))
    }

    fn admit_document(
        &self,
        recipe: &ThemeRecipeV1,
        requested: Option<ThemeAppearance>,
        retain_selected: bool,
    ) -> Result<AdmittedDocument, ThemeDesignCompileError> {
        super::definition_admission::check_typed_input_encoded_bytes(
            self.resource_policy(),
            recipe,
        )
        .map_err(encoding_error)?;
        let (supported, default) = document_appearances(recipe)?;
        let effective = requested.or(default);
        if let Some(effective) = effective
            && !supported.contains(&effective)
        {
            return Err(failure(
                "/appearance",
                "declared-theme-appearance",
                "requested appearance is not declared by this theme",
            )
            .into());
        }
        let modes = if supported.is_empty() {
            vec![None]
        } else {
            supported.iter().copied().map(Some).collect()
        };
        let mut selected = None;
        let mut inspections = Vec::with_capacity(modes.len());
        for appearance in modes {
            let composition = match recipe {
                ThemeRecipeV1::Design { design } => Some(match appearance {
                    Some(appearance) => ComposedDesign::with_appearance(
                        design,
                        appearance,
                        &design.appearances.as_ref().expect("validated appearances")[&appearance],
                    ),
                    None => ComposedDesign::shared(design, "/design"),
                }),
                ThemeRecipeV1::CompleteSpec { .. } => None,
            };
            let (spec, references, sources) = match recipe {
                ThemeRecipeV1::CompleteSpec { complete_spec } => (
                    complete_spec.clone(),
                    Vec::new(),
                    ThemeWireSources::CompleteSpec,
                ),
                ThemeRecipeV1::Design { .. } => {
                    self.materialize_composition(composition.as_ref().expect("design composition"))?
                }
            };
            let theme = self
                .compile_spec_wire_with_sources_located(spec.clone(), sources)
                .map_err(|error| match recipe {
                    ThemeRecipeV1::Design { design } if design.appearances.is_some() => {
                        let path = if error.path.starts_with("/design/") {
                            bounded_path(error.path)
                        } else {
                            composition
                                .as_ref()
                                .expect("design composition")
                                .path(&error.path)
                        };
                        ThemeDesignCompileError::Materialization(compilation_error(
                            &path,
                            error.error,
                        ))
                    }
                    _ => ThemeDesignCompileError::Compilation(error.error),
                })?
                .with_selected_appearance(appearance);
            if retain_selected && appearance == effective {
                selected = Some(SelectedDocument {
                    theme,
                    spec,
                    appearance,
                    references: references.clone(),
                });
            }
            inspections.push(ThemeAppearanceInspectionV1 {
                appearance,
                references,
            });
        }
        Ok(AdmittedDocument {
            inspection: ThemeRecipeInspectionV1::new(
                recipe.schema_version(),
                supported,
                default,
                inspections,
            ),
            selected,
        })
    }

    fn materialize_composition(
        &self,
        composition: &ComposedDesign,
    ) -> Result<
        (
            DiagramThemeSpecWireV1,
            Vec<ThemeRecipeReferenceUseV1>,
            ThemeWireSources,
        ),
        ThemeDesignCompileError,
    > {
        let (spec, references) = super::design_materializer::materialize(
            &composition.design,
            self.resource_policy(),
            true,
        )
        .map_err(|error| composition.error(error))?;
        let sources = composition.sources();
        let references = references
            .into_iter()
            .map(|reference| composition.reference(reference))
            .collect();
        Ok((spec, references, sources))
    }
}

fn document_appearances(
    recipe: &ThemeRecipeV1,
) -> Result<(Vec<ThemeAppearance>, Option<ThemeAppearance>), ThemeMaterializationErrorV1> {
    let ThemeRecipeV1::Design {
        design:
            ThemeDesignV1 {
                appearances,
                default_appearance,
                ..
            },
    } = recipe
    else {
        return Ok((Vec::new(), None));
    };
    match (appearances, default_appearance) {
        (None, None) => Ok((Vec::new(), None)),
        (Some(appearances), Some(default))
            if !appearances.is_empty() && appearances.contains_key(default) =>
        {
            Ok((appearances.keys().copied().collect(), Some(*default)))
        }
        (Some(appearances), _) if appearances.is_empty() => Err(failure(
            "/design/appearances",
            "nonempty-theme-appearances",
            "declared theme appearances must not be empty",
        )),
        _ => Err(failure(
            "/design/default_appearance",
            "declared-theme-default",
            "default_appearance must name an entry in the declared appearance collection",
        )),
    }
}

pub(super) fn compilation_error(
    path: &str,
    error: super::ThemeCompileError,
) -> ThemeMaterializationErrorV1 {
    let diagnostic = match error {
        super::ThemeCompileError::ResourceLimit(error) => {
            ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
                path,
                error.limit,
                error.actual,
                error.max,
                "composed appearance exceeds the compiler resource policy",
            )
        }
        error => {
            let message = error.to_string();
            let mut end = message.len().min(512);
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            ThemeMaterializationDiagnosticV1::invalid_token_value(
                path,
                "compiled-theme-appearance",
                &message[..end],
            )
        }
    };
    ThemeMaterializationErrorV1::from_diagnostic(diagnostic)
}

fn encoding_error(error: ThemeInputEncodingError) -> ThemeMaterializationErrorV1 {
    match error {
        ThemeInputEncodingError::ResourceLimit(error) => {
            super::definition_admission::encoded_bytes_contract_error(error)
        }
        ThemeInputEncodingError::InvalidEncoding => failure(
            "",
            "finite-theme-document",
            "theme document cannot be encoded as finite JSON",
        ),
    }
}

fn failure(path: &str, domain: &str, message: &str) -> ThemeMaterializationErrorV1 {
    ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::invalid_token_value(path, domain, message),
    )
}

fn escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn bounded_path(mut path: String) -> String {
    while path.len() > ThemeMaterializationDiagnosticV1::MAX_PATH_BYTES {
        let end = path.rfind('/').unwrap_or(0);
        path.truncate(end);
    }
    path
}
