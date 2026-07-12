//! Strict `MiniJinja` rendering of the explicit template catalog.

use std::path::PathBuf;

use minijinja::{AutoEscape, Environment, UndefinedBehavior, syntax::SyntaxConfig};

use crate::{
    error::CliError,
    init::{
        options::InitOptions,
        plan::{RenderedFile, ScaffoldPlan},
    },
    templates::{
        catalog::{self, TEMPLATES, TemplateCondition},
        context::TemplateContext,
        versions::VERSIONS,
    },
};

/// Renders every explicitly catalogued source file into memory.
pub fn render(options: &InitOptions) -> Result<ScaffoldPlan, CliError> {
    let mut environment = environment()?;
    let specs = catalog::for_framework(options.framework);
    for spec in specs {
        register_template(&mut environment, spec.source)?;
    }
    let package_name = options
        .frontend_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("frontend");
    let context = TemplateContext::new(options, package_name, &VERSIONS);
    let files = specs
        .iter()
        .filter(|spec| matches!(spec.condition, TemplateCondition::Always))
        .map(|spec| {
            let template =
                environment
                    .get_template(spec.source)
                    .map_err(|source| CliError::Template {
                        template: spec.source.to_owned(),
                        source,
                    })?;
            let contents = template
                .render(&context)
                .map_err(|source| CliError::Template {
                    template: spec.source.to_owned(),
                    source,
                })?;
            Ok(RenderedFile {
                relative_path: PathBuf::from(spec.destination),
                contents: contents.into_bytes(),
            })
        })
        .collect::<Result<Vec<_>, CliError>>()?;
    Ok(ScaffoldPlan {
        destination: options.frontend_dir.clone(),
        files,
    })
}

fn environment() -> Result<Environment<'static>, CliError> {
    let mut environment = Environment::new();
    environment.set_syntax(
        SyntaxConfig::builder()
            .block_delimiters("[[%", "%]]")
            .variable_delimiters("[[=", "]]")
            .comment_delimiters("[[#", "#]]")
            .build()
            .map_err(CliError::TemplateSyntax)?,
    );
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment.set_auto_escape_callback(|_| AutoEscape::None);
    environment.set_keep_trailing_newline(true);
    Ok(environment)
}

fn register_template(
    environment: &mut Environment<'static>,
    source_name: &'static str,
) -> Result<(), CliError> {
    let file = TEMPLATES
        .get_file(source_name)
        .ok_or(CliError::MissingEmbeddedTemplate(source_name))?;
    let source = file
        .contents_utf8()
        .ok_or(CliError::TemplateIsNotUtf8(source_name))?;
    environment
        .add_template(source_name, source)
        .map_err(|source| CliError::Template {
            template: source_name.to_owned(),
            source,
        })
}
