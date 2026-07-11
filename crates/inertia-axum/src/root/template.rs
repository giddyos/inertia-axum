use super::{RootContext, RootView};
use crate::ConfigError;
use std::{convert::Infallible, sync::Arc};

const PREFIX: &str = "<!-- inertia:";
const ASSETS: &str = "<!-- inertia:assets -->";
const HEAD: &str = "<!-- inertia:head -->";
const MOUNT: &str = "<!-- inertia:mount -->";

#[derive(Clone)]
pub(crate) struct CompiledRootTemplate {
    source: Arc<str>,
    parts: Arc<[TemplatePart]>,
    static_len: usize,
}

#[derive(Clone, Debug)]
enum TemplatePart {
    Static { start: usize, end: usize },
    Assets,
    Head,
    Mount,
}

impl CompiledRootTemplate {
    pub(crate) fn compile(source: Arc<str>, name: &str) -> Result<Self, ConfigError> {
        let mut parts = Vec::with_capacity(7);
        let mut found = [false; 3];
        let mut cursor = 0;
        let mut static_len = 0;

        while let Some(relative_start) = source[cursor..].find(PREFIX) {
            let start = cursor + relative_start;
            let Some(relative_end) = source[start..].find("-->") else {
                return Err(unknown(name, &source[start..]));
            };
            let end = start + relative_end + 3;
            let directive = &source[start..end];
            let (slot, index) = match directive {
                ASSETS => (TemplatePart::Assets, 0),
                HEAD => (TemplatePart::Head, 1),
                MOUNT => (TemplatePart::Mount, 2),
                _ => return Err(unknown(name, directive)),
            };
            if found[index] {
                return Err(duplicate(name, directive));
            }
            found[index] = true;
            if cursor < start {
                parts.push(TemplatePart::Static {
                    start: cursor,
                    end: start,
                });
                static_len += start - cursor;
            }
            parts.push(slot);
            cursor = end;
        }
        if cursor < source.len() {
            parts.push(TemplatePart::Static {
                start: cursor,
                end: source.len(),
            });
            static_len += source.len() - cursor;
        }

        for (present, marker) in found.into_iter().zip([ASSETS, HEAD, MOUNT]) {
            if !present {
                return Err(missing(name, marker));
            }
        }

        Ok(Self {
            source,
            parts: parts.into(),
            static_len,
        })
    }
}

impl RootView for CompiledRootTemplate {
    type Error = Infallible;

    fn render(&self, context: RootContext<'_>) -> Result<String, Self::Error> {
        let capacity = self.static_len
            + context.assets().as_str().len()
            + context.head().as_str().len()
            + context.mount().as_str().len();
        let mut output = String::with_capacity(capacity);
        for part in self.parts.iter() {
            match part {
                TemplatePart::Static { start, end } => output.push_str(&self.source[*start..*end]),
                TemplatePart::Assets => output.push_str(context.assets().as_str()),
                TemplatePart::Head => output.push_str(context.head().as_str()),
                TemplatePart::Mount => output.push_str(context.mount().as_str()),
            }
        }
        Ok(output)
    }
}

fn missing(name: &str, marker: &str) -> ConfigError {
    ConfigError::new(format!(
        "inertia-axum root template configuration error\n\nTemplate {name} is missing the required marker:\n\n  {marker}\n\nEvery root template must contain exactly one assets, head, and mount marker."
    ))
}

fn duplicate(name: &str, marker: &str) -> ConfigError {
    ConfigError::new(format!(
        "inertia-axum root template configuration error\n\nTemplate {name} contains more than one:\n\n  {marker}\n\nEach Inertia template marker must appear exactly once."
    ))
}

fn unknown(name: &str, directive: &str) -> ConfigError {
    ConfigError::new(format!(
        "inertia-axum root template configuration error\n\nTemplate {name} contains an unknown Inertia directive:\n\n  {directive}\n\nSupported directives are assets, head, and mount."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AssetTags, HeadMarkup, MountMarkup};

    fn compile(source: &str) -> Result<CompiledRootTemplate, ConfigError> {
        CompiledRootTemplate::compile(Arc::from(source), "test.html")
    }

    fn render(source: &str, assets: &str, head: &str, mount: &str) -> String {
        let template = compile(source).unwrap();
        let assets = AssetTags::new(assets.to_owned());
        let head = HeadMarkup::for_test(head);
        let mount = MountMarkup::for_test(mount);
        template
            .render(RootContext::new(&assets, &head, &mount))
            .unwrap()
    }

    #[test]
    fn valid_template_preserves_static_content_and_inserts_fragments() {
        let source = "<!doctype html>\n<head>A<!-- inertia:assets -->B<!-- inertia:head -->C</head>\n<body>D<!-- inertia:mount -->E</body>";
        assert_eq!(
            render(source, "ASSETS", "HEAD", "MOUNT"),
            "<!doctype html>\n<head>AASSETSBHEADC</head>\n<body>DMOUNTE</body>"
        );
    }

    #[test]
    fn marker_order_is_source_order() {
        let source = "<!-- inertia:mount -->|<!-- inertia:assets -->|<!-- inertia:head -->";
        assert_eq!(render(source, "A", "H", "M"), "M|A|H");
    }

    #[test]
    fn ordinary_comments_are_preserved() {
        let source = "<!-- inertia is neat --><!-- keep -->\n<!-- inertia:assets --><!-- inertia:head --><!-- inertia:mount -->";
        assert_eq!(
            render(source, "", "", ""),
            "<!-- inertia is neat --><!-- keep -->\n"
        );
    }

    #[test]
    fn each_missing_marker_is_actionable() {
        for (source, marker) in [
            ("<!-- inertia:head --><!-- inertia:mount -->", ASSETS),
            ("<!-- inertia:assets --><!-- inertia:mount -->", HEAD),
            ("<!-- inertia:assets --><!-- inertia:head -->", MOUNT),
        ] {
            let error = compile(source).err().unwrap().to_string();
            assert!(error.contains("missing the required marker"));
            assert!(error.contains(marker));
            assert!(error.contains("test.html"));
        }
    }

    #[test]
    fn duplicate_marker_is_rejected() {
        let error = compile("<!-- inertia:assets --><!-- inertia:head --><!-- inertia:head --><!-- inertia:mount -->")
            .err().unwrap().to_string();
        assert!(error.contains("contains more than one"));
        assert!(error.contains(HEAD));
    }

    #[test]
    fn unknown_directive_is_rejected() {
        let error = compile("<!-- inertia:assets --><!-- inertia:title --><!-- inertia:head --><!-- inertia:mount -->")
            .err().unwrap().to_string();
        assert!(error.contains("unknown Inertia directive"));
        assert!(error.contains("<!-- inertia:title -->"));
    }

    #[test]
    fn csr_document_is_exact() {
        let source = "<html><head><!-- inertia:assets --><!-- inertia:head --></head><body><!-- inertia:mount --></body></html>";
        let template = compile(source).unwrap();
        let assets = AssetTags::new("<script src=\"/app.js\"></script>".to_owned());
        let head = HeadMarkup::empty();
        let mount = MountMarkup::csr(r#"{"component":"Home"}"#);
        let output = template
            .render(RootContext::new(&assets, &head, &mount))
            .unwrap();
        assert_eq!(
            output,
            r#"<html><head><script src="/app.js"></script></head><body><script data-page="app" type="application/json">{"component":"Home"}</script><div id="app"></div></body></html>"#
        );
    }

    #[test]
    fn ssr_head_and_body_are_placed_once() {
        let source = "<head><!-- inertia:assets --><!-- inertia:head --></head><body><!-- inertia:mount --></body>";
        let output = render(
            source,
            "",
            "<title>SSR</title>",
            "<div id=\"app\">SSR</div>",
        );
        assert_eq!(output.matches("<title>SSR</title>").count(), 1);
        assert_eq!(output.matches("<div id=\"app\">SSR</div>").count(), 1);
        assert_eq!(
            output,
            "<head><title>SSR</title></head><body><div id=\"app\">SSR</div></body>"
        );
    }
}
