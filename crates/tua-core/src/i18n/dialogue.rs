//! Dialogue source defaults share the catalog index, but are not guaranteed I18n.t values.
use super::*;

impl CatalogIndex {
    pub(crate) fn add_dialogue(
        &mut self,
        path: PathBuf,
        source: &crate::dialogue_asset::DialogueSource,
    ) {
        for section in &source.sections {
            for line in &section.lines {
                for text in
                    std::iter::once(&line.text).chain(line.choices.iter().flat_map(|choice| {
                        std::iter::once(&choice.text).chain(choice.reason.iter())
                    }))
                {
                    let template = text.rich.as_ref().map_or(&text.value, |rich| &rich.source);
                    if let Some(rich) = &text.rich {
                        self.rich_dialogue.insert(text.id.value.clone(), rich.kind);
                    }
                    self.entries.entry(text.id.value.clone()).or_default();
                    self.dialogue_sources
                        .entry(text.id.value.clone())
                        .or_insert_with(|| CatalogEntry {
                            key: text.id.value.clone(),
                            locale: String::new(),
                            path: path.clone(),
                            span: text.id.span,
                            value: CatalogValue::Text(template.clone()),
                            placeholders: dialogue_placeholders(&text.value).unwrap_or_default(),
                        });
                }
            }
        }
    }

    pub(crate) fn check_dialogue_sources(&mut self, config: &I18nConfig) {
        for (key, source) in &self.dialogue_sources {
            let entries = &self.entries[key];
            for catalog in &config.catalogs {
                let Some(entry) = entries.get(&catalog.locale) else {
                    self.diagnostics.entry(source.path.clone()).or_default().push(
                        Diagnostic::new(&source.path, source.span.start.line, source.span.start.column,
                            DiagnosticSeverity::Warning, format!("dialogue text `{key}` has no exact `{}` translation; locale/source fallback applies", catalog.locale))
                            .with_code(DiagnosticCode::DOMAIN_I18N));
                    continue;
                };
                let check = |text: &str| {
                    if let Some(kind) = self.rich_dialogue.get(key) {
                        match crate::dialogue_asset::rich_text::parse(text, *kind) {
                            Ok(template) => {
                                check_text(&template.plain_template, &source.placeholders)
                            }
                            Err(error) => Some(format!(
                                "has invalid rich markup at template bytes {}..{}: {}",
                                error.range.start + 1,
                                error.range.end,
                                error.message
                            )),
                        }
                    } else {
                        check_text(text, &source.placeholders)
                    }
                };
                let invalid = match &entry.value {
                    CatalogValue::Dialogue(_) => {
                        Some("must be scalar text or plural forms, not an array".to_owned())
                    }
                    CatalogValue::Dynamic => None,
                    CatalogValue::Text(text) => check(text),
                    CatalogValue::Plural(forms) => forms.values().find_map(|text| check(text)),
                };
                if let Some(message) = invalid {
                    self.diagnostics
                        .entry(entry.path.clone())
                        .or_default()
                        .push(
                            Diagnostic::error(
                                &entry.path,
                                entry.span.start.line,
                                entry.span.start.column,
                                format!("dialogue text `{key}` {message}"),
                            )
                            .with_code(DiagnosticCode::DOMAIN_I18N),
                        );
                }
                if entry.locale == config.default_locale
                    && matches!(&entry.value, CatalogValue::Text(text) if source.value != CatalogValue::Text(text.clone()))
                {
                    self.diagnostics.entry(entry.path.clone()).or_default().push(
                        Diagnostic::new(&entry.path, entry.span.start.line, entry.span.start.column, DiagnosticSeverity::Warning,
                            format!("dialogue source drift for `{key}`: default-locale text differs from authored prose; review without overwriting translator edits"))
                            .with_code(DiagnosticCode::DOMAIN_I18N));
                }
            }
        }
    }
}

pub(crate) fn dialogue_placeholders(text: &str) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            continue;
        }
        match chars.peek() {
            Some('%') => {
                chars.next();
            }
            Some('{') => {
                chars.next();
                let mut name = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' {
                        closed = true;
                        break;
                    }
                    name.push(ch);
                }
                let name = name.trim();
                if !closed
                    || name.is_empty()
                    || !name.chars().enumerate().all(|(i, ch)| {
                        ch == '_' || ch.is_ascii_alphabetic() || (i > 0 && ch.is_ascii_digit())
                    })
                {
                    return Err("invalid named dialogue placeholder; expected `%{name}`".into());
                }
                names.insert(name.to_owned());
            }
            _ => {}
        }
    }
    Ok(names)
}

fn check_text(text: &str, expected: &BTreeSet<String>) -> Option<String> {
    match dialogue_placeholders(text) {
        Err(message) => Some(message),
        Ok(actual) if &actual != expected => Some(format!(
            "placeholder mismatch: source {expected:?}, translation {actual:?}"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisHost, AnalysisSourceKind, I18nCatalogConfig, ProjectConfig};

    #[test]
    fn dialogue_percent_escaping_and_each_plural_form_are_checked() {
        let expected = BTreeSet::from(["name".to_owned()]);
        assert_eq!(
            dialogue_placeholders("%%%{name} %%{literal}").unwrap(),
            expected
        );
        assert!(check_text("Hello %{name}", &expected).is_none());
        assert!(check_text("Hello", &expected).is_some());
        assert!(check_text("%{broken", &expected).is_some());
        let mut index = CatalogIndex::default();
        let scene = crate::dialogue_asset::parse_dialogue_source(
            "scene.tdialogue",
            "@dialogue 1\n[s]\n// @id hello\n: %%%{name} (end)\n",
        )
        .unwrap();
        index.add_dialogue("scene.tdialogue".into(), &scene);
        let en =
            crate::parse_lua_source("en.lua", "return { en = { hello = '%%%{name}' } }").unwrap();
        let fr =
            crate::parse_lua_source("fr.lua", "return { fr = { hello = '%{name}' } }").unwrap();
        index.add_catalog("en", "en.lua".into(), Some(&en));
        index.add_catalog("fr", "fr.lua".into(), Some(&fr));
        let config = I18nConfig {
            enabled: true,
            catalogs: vec![
                I18nCatalogConfig {
                    locale: "en".into(),
                    path: "en.lua".into(),
                },
                I18nCatalogConfig {
                    locale: "fr".into(),
                    path: "fr.lua".into(),
                },
            ],
            ..I18nConfig::default()
        };
        index.finish(&config);
        index.check_dialogue_sources(&config);
        assert!(!index
            .diagnostics
            .values()
            .flatten()
            .any(|d| d.severity == DiagnosticSeverity::Error));
    }

    #[test]
    fn dialogue_coverage_tracks_source_and_catalog_overlays_without_execution() {
        let mut config = ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs = vec![
            I18nCatalogConfig {
                locale: "en".into(),
                path: "en.lua".into(),
            },
            I18nCatalogConfig {
                locale: "fr".into(),
                path: "fr.lua".into(),
            },
        ];
        let mut host = AnalysisHost::with_config(config);
        host.set_source_roots([PathBuf::from("src")]);
        host.set_file_with_kind(
            "src/en.lua",
            "return { en = { hello = 'Hello %{name}' } }",
            AnalysisSourceKind::Lua,
        );
        host.set_file_with_kind(
            "src/fr.lua",
            "error('never execute'); return { fr = {} }",
            AnalysisSourceKind::Lua,
        );
        let scene =
            "@dialogue 1\n@document greeting\n[start]\n// @id hello\n: Hello %{name} (end)\n";
        host.set_file_with_kind("src/scene.tdialogue", scene, AnalysisSourceKind::Dialogue);
        let first = host.snapshot();
        let page = first.localization_page("", 0, 10);
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].cells[1].status, "fallback");
        assert_eq!(
            page.rows[0].source.as_ref().unwrap().preview.as_deref(),
            Some("Hello %{name}")
        );
        assert!(first
            .pipeline_diagnostics("src/scene.tdialogue")
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning));
        let position = SourcePosition { line: 4, column: 9 };
        assert!(first
            .hover("src/scene.tdialogue", position)
            .unwrap()
            .contents
            .contains("hello"));
        assert!(first
            .definition("src/scene.tdialogue", position)
            .unwrap()
            .path
            .ends_with("en.lua"));
        host.set_file_with_kind(
            "src/fr.lua",
            "return { fr = { hello = 'Bonjour %{wrong}' } }",
            AnalysisSourceKind::Lua,
        );
        assert!(host
            .snapshot()
            .i18n_catalog_diagnostics()
            .iter()
            .any(|d| d.message.contains("placeholder mismatch")));
        let snapshot = host.snapshot();
        let diagnostics = snapshot.pipeline_diagnostics("src/fr.lua");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("placeholder mismatch")),
            "pipeline: {diagnostics:?}; catalogs: {:?}",
            snapshot.i18n_catalog_diagnostics()
        );
        host.set_file_with_kind(
            "src/fr.lua",
            "return { fr = { hello = { 'Bonjour %{name}' } } }",
            AnalysisSourceKind::Lua,
        );
        assert!(host
            .snapshot()
            .i18n_catalog_diagnostics()
            .iter()
            .any(|d| d.message.contains("not an array")));
        host.set_file_with_kind(
            "src/fr.lua",
            "return { fr = { hello = unknown() } }",
            AnalysisSourceKind::Lua,
        );
        assert_eq!(
            host.snapshot().localization_page("hello", 0, 10).rows[0].cells[1].status,
            "dynamic"
        );
        host.set_file_with_kind(
            "src/scene.tdialogue",
            scene.replace("Hello %{name}", "Greetings %{name}"),
            AnalysisSourceKind::Dialogue,
        );
        assert!(host
            .snapshot()
            .i18n_catalog_diagnostics()
            .iter()
            .any(|d| d.message.contains("source drift")));
        host.set_file_with_kind(
            "src/scene.tdialogue",
            scene.replace("@id hello", "@id absent"),
            AnalysisSourceKind::Dialogue,
        );
        let page = host.snapshot().localization_page("absent", 0, 10);
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].cells[0].status, "missing");
        assert!(page.rows[0].source.is_some());
        // A source default is not a guaranteed catalog result for ordinary I18n.t.
        let mut index = CatalogIndex::default();
        index.add_dialogue(
            "src/scene.tdialogue".into(),
            &crate::dialogue_asset::parse_dialogue_source(
                "scene.tdialogue",
                &scene.replace("@id hello", "@id absent"),
            )
            .unwrap(),
        );
        assert!(!index
            .guaranteed_result_types(&host.snapshot().config().i18n)
            .contains_key("absent"));
    }
}

#[cfg(test)]
mod rich_catalog_tests {
    use super::*;
    use crate::{AnalysisHost, AnalysisSourceKind, I18nCatalogConfig, ProjectConfig};

    #[test]
    fn rich_catalog_validation_tracks_forms_field_kind_and_source_edits() {
        let mut config = ProjectConfig::default();
        config.i18n.enabled = true;
        config.i18n.catalogs.push(I18nCatalogConfig {
            locale: "en".into(),
            path: "en.lua".into(),
        });
        let mut host = AnalysisHost::with_config(config);
        host.set_source_roots([PathBuf::from("src")]);
        let scene = "@dialogue 1\n@text rich\n[s]\n// @id hello\n: [pronounce=\"100%\"]Hi %{name}[/pronounce]\n// @id go\n-> Go [target:s]\n";
        host.set_file_with_kind("src/scene.tdialogue", scene, AnalysisSourceKind::Dialogue);
        host.set_file_with_kind("src/en.lua", "return { en = { hello = { one = '[em]Hi %{name}[/em]', other = '[em]Hi %{name}' }, go = '[pause=0]Go' } }", AnalysisSourceKind::Lua);
        let diagnostics = host.snapshot().i18n_catalog_diagnostics();
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("unclosed rich-text mark")),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("only supported in lines")),
            "{diagnostics:?}"
        );
        host.set_file_with_kind(
            "src/en.lua",
            "return { en = { hello = '[pronounce=\"50%\"]%{name}[/pronounce]', go = '[em]Go[/em]' } }",
            AnalysisSourceKind::Lua,
        );
        assert!(!host
            .snapshot()
            .i18n_catalog_diagnostics()
            .iter()
            .any(
                |d| d.message.contains("invalid rich markup") || d.message.contains("placeholder")
            ));
        host.set_file_with_kind(
            "src/en.lua",
            "return { en = { hello = '[unknown]%{name}', go = '[pause=0]Go' } }",
            AnalysisSourceKind::Lua,
        );
        host.set_file_with_kind(
            "src/scene.tdialogue",
            scene
                .replace("@text rich\n", "")
                .replace("[pronounce=\"100%\"]", "")
                .replace("[/pronounce]", ""),
            AnalysisSourceKind::Dialogue,
        );
        assert!(!host
            .snapshot()
            .i18n_catalog_diagnostics()
            .iter()
            .any(|d| d.message.contains("invalid rich markup")));
    }
}
