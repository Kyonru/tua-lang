use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::backends::{
    Backend, Candidate, CurrentFileCommonPrefix, CurrentFileRecall, Phase1, Phase2Tuning,
    Phase2TuningConfig, ProductionRequest, RecallOptions, Request, Semantic,
};
use super::dataset::{self, ManifestSet, SetRole};
use super::metrics::{precision_coverage_curve, score, snippet_literal_prefix, tokens};
use super::*;
use crate::inline_completion::eligibility::cursor_context;
use crate::{AnalysisHost, AnalysisSourceKind, AnalysisWarmPriority};

fn request<'a>(document: &'a str, declared: Option<&'a BTreeSet<String>>) -> Request<'a> {
    let offset = document.find('|').expect("cursor marker");
    let line_index = document[..offset].matches('\n').count();
    // Leak the marker-free text so the request can borrow it for the test.
    let document: &'a str = Box::leak(document.replacen('|', "", 1).into_boxed_str());
    Request {
        document,
        offset,
        line_index,
        context: cursor_context(document, offset).expect("eligible cursor"),
        declared_callbacks: declared,
        include_ecs: true,
        path: std::path::Path::new("test.tua"),
        language: "tua",
        retrieval_index: None,
        ngram_index: None,
        allow_target_file_other_lines: true,
    }
}

/// A temporary directory removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tua-inline-eval-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn recall() -> CurrentFileRecall {
    CurrentFileRecall {
        options: RecallOptions::default(),
    }
}

fn production_phase2_host(continuation: &str) -> (AnalysisHost, PathBuf, String) {
    let active = PathBuf::from("active/main.tua");
    let source = "draw(player,".to_owned();
    let mut host = AnalysisHost::new();
    host.set_project_pattern_eligibility_generation(PRODUCTION_EVAL_CORPUS_GENERATION);
    host.set_file(&active, source.clone());
    host.set_file("one/a.tua", format!("draw(player,{continuation}"));
    host.set_file("two/b.tua", format!("draw(player,{continuation}"));
    let entries = ["one/a.tua", "two/b.tua"].map(|path| {
        crate::PatternCorpusBatchEntry::File(
            host.prepare_project_pattern_file(path)
                .expect("corpus input")
                .build(
                    &crate::ProjectModel::default(),
                    crate::RetrievalLimits::default(),
                ),
        )
    });
    let adoption = host.adopt_project_pattern_batch(PRODUCTION_EVAL_CORPUS_GENERATION, entries);
    assert_eq!(adoption.adopted, 2);
    (host, active, source)
}

fn production_phase2_candidate(
    host: &AnalysisHost,
    path: &std::path::Path,
    document: &str,
) -> Option<Candidate> {
    let offset = document.len();
    let request = Request {
        document,
        offset,
        line_index: 0,
        context: cursor_context(document, offset).expect("eligible cursor"),
        declared_callbacks: None,
        include_ecs: false,
        path,
        language: "tua",
        retrieval_index: None,
        ngram_index: None,
        allow_target_file_other_lines: false,
    };
    Phase2Tuning {
        config: Phase2TuningConfig::pinned(),
    }
    .suggest_with_host(
        &request,
        &ProductionRequest {
            host,
            path,
            position: crate::SourcePosition {
                line: 1,
                column: document.chars().count() + 1,
            },
        },
    )
}

#[test]
fn phase2_combined_backend_uses_the_adopted_production_corpus() {
    let (host, path, source) = production_phase2_host(" score)");
    let candidate = production_phase2_candidate(&host, &path, &source).expect("project hit");
    assert_eq!(candidate.text, " score)");
    assert_eq!(candidate.source, "project");
}

#[test]
fn phase2_combined_backend_rejects_a_parse_worsening_project_hit() {
    let (host, path, source) = production_phase2_host(" end)");
    assert!(production_phase2_candidate(&host, &path, &source).is_none());
}

#[test]
fn phase2_combined_backend_rejects_stale_production_corpus_versions() {
    let (mut host, path, source) = production_phase2_host(" score)");
    host.set_file("one/a.tua", "draw(player, changed)");
    host.set_file("two/b.tua", "draw(player, changed)");
    assert!(production_phase2_candidate(&host, &path, &source).is_none());
}

#[test]
fn tokens_keep_operators_strings_and_numbers_whole() {
    assert_eq!(
        tokens("love.graphics.setColor(1, 0.5) .. \"a b\" ~= x"),
        vec![
            "love", ".", "graphics", ".", "setColor", "(", "1", ",", "0.5", ")", "..", "\"a b\"",
            "~=", "x"
        ]
    );
}

#[test]
fn scoring_rewards_correct_prefixes_and_counts_wrong_characters() {
    let exact = score("x + 1)", false, "x + 1)");
    assert!(exact.exact && exact.first_token && exact.first_three_tokens);
    assert_eq!((exact.chars_saved, exact.incorrect_chars), (6, 0));

    let prefix = score("x +", false, "x + 1)");
    assert!(!prefix.exact && prefix.correct_prefix && prefix.first_token);
    // Two offered tokens can't count as three matching ones.
    assert!(!prefix.first_three_tokens);
    assert_eq!((prefix.chars_saved, prefix.incorrect_chars), (3, 0));

    // Whitespace alone is neither a correct prefix nor useful saved text.
    let blank = score(" ", false, " x + 1)");
    assert!(!blank.correct_prefix);
    assert_eq!(blank.chars_saved, 0);

    let wrong = score("y - 2", false, "x + 1)");
    assert!(!wrong.first_token && !wrong.correct_prefix);
    assert_eq!((wrong.chars_saved, wrong.incorrect_chars), (0, 5));

    // Only the first line of a multi-line suggestion is offered.
    let callback = score("date(dt)\nend", false, "date(dt)");
    assert!(callback.exact);
    assert_eq!(callback.chars_saved, 8);
}

#[test]
fn snippets_offer_their_literal_prefix_and_match_as_templates() {
    assert_eq!(snippet_literal_prefix("al ${1:name} = ${0:nil}"), "al ");
    let fitted = score("al ${1:name} = ${0:nil}", true, "al speed = 300");
    assert!(fitted.template_match && fitted.correct_prefix);
    // The defaults VS Code inserts aren't what the author wrote.
    assert!(!fitted.exact);
    assert_eq!((fitted.chars_saved, fitted.incorrect_chars), (3, 0));
    assert!(score("al ${1:name} = ${0:nil}", true, "al name = nil").exact);

    let unfitted = score("al ${1:name} = ${0:nil}", true, "al function f()");
    assert!(!unfitted.template_match);
    // Placeholders must stand for some text.
    assert!(!score("al ${1:name} = ${0:nil}", true, "al  = ").template_match);
}

#[test]
fn precision_coverage_curves_trade_coverage_for_precision() {
    let scored = [(0.2, false), (0.5, true), (0.9, true), (1.0, true)].map(|(score, exact)| {
        metrics::Scored {
            score,
            exact,
            correct_prefix: exact,
        }
    });
    let curve = precision_coverage_curve(&scored, 10);
    let first = curve.first().expect("points");
    let last = curve.last().expect("points");
    assert!((first.coverage - 0.4).abs() < 1e-9);
    assert!((first.exact_precision - 0.75).abs() < 1e-9);
    assert!(last.coverage < first.coverage);
    assert!((last.exact_precision - 1.0).abs() < 1e-9);
}

#[test]
fn recall_continues_with_the_original_text_of_another_line() {
    let document =
        "love.graphics.setColor(1,  0.5, 0) -- warm\nlocal x = 1\nlove.graphics.setColor(|";
    let candidate = recall().suggest(&request(document, None)).expect("recall");
    // Original spacing is kept and the trailing comment is dropped.
    assert_eq!(candidate.text, "1,  0.5, 0)");
    assert_eq!(candidate.source, "current-file");
}

#[test]
fn recall_normalizes_indentation_and_spacing_for_lookup_only() {
    let document = "    if  player.health <= 0 then\nend\nif player.health |";
    let candidate = recall().suggest(&request(document, None)).expect("recall");
    assert_eq!(candidate.text, "<= 0 then");
}

#[test]
fn recall_never_reads_the_active_line() {
    let document = "local enemyCount = 3|";
    assert!(recall().suggest(&request(document, None)).is_none());
}

#[test]
fn recall_needs_a_meaningful_prefix() {
    let document = "foo(1)\nfoo(|";
    assert!(recall().suggest(&request(document, None)).is_none());
}

#[test]
fn recall_can_require_multiple_supporting_lines() {
    let document = "spawn(enemy, 1)\nspawn(enemy, |";
    let strict = CurrentFileRecall {
        options: RecallOptions {
            min_support: 2,
            ..RecallOptions::default()
        },
    };
    assert!(strict.suggest(&request(document, None)).is_none());

    let repeated = "spawn(enemy, 1)\nspawn(enemy, 1)\nspawn(enemy, |";
    assert!(strict.suggest(&request(repeated, None)).is_some());
}

#[test]
fn common_prefix_recall_limits_low_support_to_one_token() {
    let document = "local result = player.position.x\nlocal result = |";
    let cautious = CurrentFileCommonPrefix {
        recall: CurrentFileRecall {
            options: RecallOptions {
                min_full_prefix_support: 2,
                ..RecallOptions::default()
            },
        },
    };
    let candidate = cautious
        .suggest(&request(document, None))
        .expect("one matching line still offers a token");
    assert_eq!(candidate.text, "player");

    let repeated =
        "local result = player.position.x\nlocal result = player.position.x\nlocal result = |";
    let candidate = cautious
        .suggest(&request(repeated, None))
        .expect("repeated lines support the full prefix");
    assert_eq!(candidate.text, "player.position.x");
}

#[test]
fn recall_suppresses_ambiguous_continuations_unless_one_dominates() {
    let tied = "spawn(enemy, 1)\nspawn(enemy, 2)\nspawn(enemy, |";
    assert!(recall().suggest(&request(tied, None)).is_none());

    let dominant = "spawn(enemy, 1)\nspawn(enemy, 1)\nspawn(enemy, 2)\nspawn(enemy, |";
    let candidate = recall()
        .suggest(&request(dominant, None))
        .expect("dominant");
    assert_eq!(candidate.text, "1)");
    assert!((candidate.score - 2.0 / 3.0).abs() < 1e-9);
}

#[test]
fn statement_snippets_need_a_unique_match_that_continues_the_word() {
    // `if` also matches `if else`, so it is ambiguous.
    assert!(Semantic.suggest(&request("i|", None)).is_none());
    let candidate = Semantic
        .suggest(&request("whi|", None))
        .expect("while snippet");
    assert!(candidate.is_snippet);
    assert!(candidate.text.starts_with("le ${1:condition} do"));
}

#[test]
fn phase1_configuration_controls_semantic_sources_and_prefixes() {
    let configured = |statement_snippets, snippet_min_prefix_chars| Phase1 {
        config: Phase1Config {
            love_callbacks: false,
            statement_snippets,
            snippet_min_prefix_chars,
            ..Phase1Config::default()
        },
    };

    assert!(configured(true, 3).suggest(&request("wh|", None)).is_none());
    assert!(configured(true, 3)
        .suggest(&request("whi|", None))
        .is_some());
    assert!(configured(false, 3)
        .suggest(&request("whi|", None))
        .is_none());
}

#[test]
fn love_callbacks_need_cached_facts_and_skip_declared_ones() {
    assert!(Semantic
        .suggest(&request("function love.upd|", None))
        .is_none());

    let none_declared = BTreeSet::new();
    let candidate = Semantic
        .suggest(&request("function love.upd|", Some(&none_declared)))
        .expect("update callback");
    assert_eq!(candidate.text, "ate(dt)\nend");
    assert_eq!(candidate.source, "love-callback");

    let declared = BTreeSet::from(["update".to_owned()]);
    assert!(Semantic
        .suggest(&request("function love.upd|", Some(&declared)))
        .is_none());
}

#[test]
fn positions_cover_token_ends_partial_words_and_line_ends() {
    let source = "-- header\nlocal x = f(a, b) -- note\n\n  y = \"s t\"\n";
    let (positions, ineligible) = sample_positions(source, 0);
    let line = source.find("local").expect("line");
    let kind_at = |offset: usize| {
        positions
            .iter()
            .find(|position| position.offset == offset)
            .map(|position| position.prefix)
    };
    assert_eq!(kind_at(line + "local".len()), Some(PrefixKind::TokenEnd));
    assert_eq!(kind_at(line + "l".len()), Some(PrefixKind::Partial1));
    assert_eq!(kind_at(line + "lo".len()), Some(PrefixKind::Partial2));
    assert_eq!(kind_at(line + "loc".len()), Some(PrefixKind::Partial3));
    assert_eq!(kind_at(line + "loca".len()), Some(PrefixKind::PartialNear));
    assert_eq!(kind_at(line + "local ".len()), Some(PrefixKind::Whitespace));
    assert_eq!(
        cursor_context(source, line + "local ".len()),
        Some(CursorContext::Whitespace)
    );
    // The end of the line is sampled; the comment and blank line aren't.
    assert_eq!(
        kind_at(line + "local x = f(a, b)".len()),
        Some(PrefixKind::TokenEnd)
    );
    assert!(!positions.iter().any(|position| position.offset < line));
    // The internal space immediately before a string is sampled, then counted
    // as ineligible because the cursor is already at the string boundary.
    assert_eq!(ineligible, 1);
}

#[test]
fn capped_positions_are_evenly_spaced_and_keep_the_last() {
    let source = "local alpha = beta + gamma\nreturn alpha\n";
    let (all, _) = sample_positions(source, 0);
    let (limited, _) = sample_positions(source, 4);
    assert_eq!(limited.len(), 4);
    assert_eq!(limited[0].offset, all[0].offset);
    assert_eq!(limited[3].offset, all[all.len() - 1].offset);
    let (again, _) = sample_positions(source, 4);
    assert_eq!(
        limited
            .iter()
            .map(|position| position.offset)
            .collect::<Vec<_>>(),
        again
            .iter()
            .map(|position| position.offset)
            .collect::<Vec<_>>()
    );
}

#[test]
fn auto_closed_brackets_must_agree_with_the_suggestion_structurally() {
    let candidate = |text: &str| Candidate {
        text: text.to_owned(),
        is_snippet: false,
        source: "test",
        score: 1.0,
        tier: None,
    };
    let agreed = |text: &str, suffix: &str| {
        agree_with_suffix(candidate(text), suffix).map(|candidate| candidate.text)
    };

    // Closes the outer bracket at its end: the existing closer stands in for it.
    assert_eq!(agreed("a, b)", ")").as_deref(), Some("a, b"));
    // Closes nothing from before the cursor: the existing closer stays after it,
    // even though the suggestion ends with a `)` of its own.
    assert_eq!(agreed("foo()", ")").as_deref(), Some("foo()"));
    assert_eq!(agreed("a, b", ")").as_deref(), Some("a, b"));
    // The line still reads exactly as the suggestion does.
    assert_eq!(agreed("a) + g(b)", ")").as_deref(), Some("a) + g(b"));
    assert_eq!(agreed("x[1]))", "))").as_deref(), Some("x[1]"));
    // Closing early would leave an extra closer at the end of the line.
    assert_eq!(agreed("a) + b", ")"), None);
    // Brackets inside strings don't count.
    assert_eq!(agreed("\")\"", ")").as_deref(), Some("\")\""));
}

#[test]
fn an_open_menu_only_allows_suggestions_extending_its_selection() {
    let candidate = Candidate {
        text: "ctor(1, 2)".to_owned(),
        is_snippet: false,
        source: "test",
        score: 1.0,
        tier: None,
    };
    let document = "local p = ve";
    let selected = |text: &str| MenuSelection {
        text: text.to_owned(),
        typed_prefix: "ve".to_owned(),
    };
    assert!(extends_selection(
        candidate.clone(),
        Some(&selected("vector")),
        document,
        document.len()
    )
    .is_some());
    assert!(extends_selection(
        candidate.clone(),
        Some(&selected("velocity")),
        document,
        document.len()
    )
    .is_none());
    assert!(extends_selection(candidate, None, document, document.len()).is_some());
}

fn manifest_set(role: SetRole, files: Vec<PathBuf>) -> ManifestSet {
    ManifestSet {
        role,
        description: String::new(),
        projects: Vec::new(),
        files,
        exclude: Vec::new(),
    }
}

#[test]
fn the_split_rejects_shared_aliased_and_nested_paths() {
    let directory = TempDir::new("split");
    std::fs::create_dir_all(directory.path().join("examples/game/src")).expect("create");
    let load = |sets: &str| {
        let path = directory.path().join("manifest.toml");
        std::fs::write(&path, format!("version = 1\n{sets}")).expect("write manifest");
        dataset::Manifest::load(&path)
    };

    for sets in [
        // The same project twice.
        "[sets.tune]\nrole = \"tune\"\nprojects = [\"examples/game\"]\n[sets.holdout]\nrole = \"holdout\"\nprojects = [\"examples/game\"]\n",
        // The same project spelled differently.
        "[sets.tune]\nrole = \"tune\"\nprojects = [\"./examples/game\"]\n[sets.holdout]\nrole = \"holdout\"\nprojects = [\"examples/../examples/game\"]\n",
        // A project inside another.
        "[sets.tune]\nrole = \"tune\"\nprojects = [\"examples\"]\n[sets.holdout]\nrole = \"holdout\"\nprojects = [\"examples/game\"]\n",
        // Overlap within one set counts files twice.
        "[sets.tune]\nrole = \"tune\"\nprojects = [\"examples/game\", \"examples/game/src\"]\n",
    ] {
        let error = load(sets).expect_err("overlapping split");
        assert!(error.contains("overlap"), "{error}");
    }

    let manifest =
        load("root = \"examples\"\n[sets.tune]\nrole = \"tune\"\nprojects = [\"game\"]\n")
            .expect("paths resolve from the root next to the manifest");
    let resolved = &manifest.sets["tune"].projects[0];
    assert_eq!(
        resolved,
        &directory
            .path()
            .join("examples/game")
            .canonicalize()
            .expect("canonical")
    );
}

#[test]
fn duplicate_files_stay_only_in_the_highest_priority_set() {
    let directory = TempDir::new("dedup");
    let shared = directory.path().join("shared.lua");
    let copy = directory.path().join("copy.lua");
    let unique = directory.path().join("unique.lua");
    std::fs::write(&shared, "local a = 1\nreturn a\n").expect("write");
    std::fs::write(&copy, "local a = 1\r\nreturn a\r\n\n").expect("write");
    std::fs::write(&unique, "return 2\n").expect("write");

    let (sets, duplicates) = dataset::load_sets([
        (
            "tune".to_owned(),
            manifest_set(SetRole::Tune, vec![copy.clone(), unique]),
        ),
        (
            "holdout".to_owned(),
            manifest_set(SetRole::Holdout, vec![shared]),
        ),
    ])
    .expect("sets");

    let holdout = sets
        .iter()
        .find(|set| set.name == "holdout")
        .expect("holdout");
    let tune = sets.iter().find(|set| set.name == "tune").expect("tune");
    assert_eq!(holdout.projects.len(), 1);
    assert_eq!(
        tune.projects.len(),
        1,
        "the duplicate is removed from tuning"
    );
    assert_eq!(
        duplicates.dropped,
        vec![format!("tune: {}", copy.display())]
    );
}

fn gate_report(roles: &[(&str, SetRole)], suggestions: usize) -> EvalReport {
    let mut summary = metrics::Summary::default();
    for index in 0..4 {
        let outcome = (index < suggestions).then_some(metrics::Outcome {
            exact: true,
            template_match: true,
            correct_prefix: true,
            first_token: true,
            first_three_tokens: true,
            chars_saved: 4,
            incorrect_chars: 0,
        });
        summary.add(outcome, 1_000);
    }
    let mut backend = BackendReport::default();
    backend
        .by_language
        .insert("tua".to_owned(), summary.finish());
    EvalReport {
        sets: roles
            .iter()
            .map(|(name, role)| SetReport {
                name: (*name).to_owned(),
                role: Some(*role),
                backends: BTreeMap::from([("phase1".to_owned(), backend.clone())]),
                ..SetReport::default()
            })
            .collect(),
        ..EvalReport::default()
    }
}

fn thresholds_for(set: &str, backend: &str, languages: &[&str]) -> Thresholds {
    Thresholds {
        backend: backend.to_owned(),
        phase1: None,
        phase2_tuning: None,
        sets: BTreeMap::from([(
            set.to_owned(),
            GateSetThresholds {
                languages: languages
                    .iter()
                    .map(|language| {
                        (
                            (*language).to_owned(),
                            LanguageThresholds {
                                min_suggestions: Some(2),
                                min_correct_prefix_precision: Some(0.6),
                                max_wrong_to_saved_ratio: Some(1.0),
                                min_chars_saved_per_eligible: Some(5.0),
                                ..LanguageThresholds::default()
                            },
                        )
                    })
                    .collect(),
            },
        )]),
    }
}

#[test]
fn quality_gates_ignore_the_cold_validation_distribution() {
    let mut report = gate_report(&[("holdout", SetRole::Holdout)], 4);
    report.sets[0].inline_validation = InlineValidationReport {
        candidate_attempts: 10,
        two_parse_candidates: 10,
        parses_per_candidate_attempt: 2.0,
        ..InlineValidationReport::default()
    };

    let gates = check_gates(&report, &thresholds_for("holdout", "phase1", &["tua"]));
    assert_eq!(gates.len(), 4);
    assert!(gates.iter().all(|gate| !gate.check.contains("parse")));
}

#[test]
fn gates_check_every_limit_on_gate_sets_only() {
    let report = gate_report(&[("holdout", SetRole::Holdout), ("tune", SetRole::Tune)], 4);
    let gates = check_gates(&report, &thresholds_for("holdout", "phase1", &["tua"]));
    assert_eq!(gates.len(), 4);
    assert!(gates.iter().all(|gate| gate.set == "holdout"));
    let failed = gates
        .iter()
        .filter(|gate| !gate.passed)
        .map(|gate| gate.check)
        .collect::<Vec<_>>();
    assert_eq!(failed, vec!["min_chars_saved_per_eligible"]);
}

#[test]
fn gates_fail_when_the_contract_cannot_be_checked() {
    let failures = |report: &EvalReport, thresholds: &Thresholds| {
        check_gates(report, thresholds)
            .into_iter()
            .filter(|gate| !gate.passed)
            .map(|gate| gate.check)
            .collect::<Vec<_>>()
    };
    let holdout = gate_report(&[("holdout", SetRole::Holdout)], 4);

    assert_eq!(
        failures(
            &gate_report(&[("tune", SetRole::Tune)], 4),
            &thresholds_for("holdout", "phase1", &["tua"])
        ),
        vec!["no gate set was evaluated"]
    );
    assert_eq!(
        failures(&holdout, &thresholds_for("holdout", "phase-one", &["tua"])),
        vec!["the thresholds' backend was not evaluated"]
    );
    assert_eq!(
        failures(&holdout, &thresholds_for("external", "phase1", &["lua"])),
        vec!["the gate set has no thresholds"]
    );
    assert_eq!(
        failures(
            &holdout,
            &thresholds_for("holdout", "phase1", &["tua", "lua"])
        ),
        vec![
            "no eligible requests in this language",
            "min_chars_saved_per_eligible"
        ]
    );
    assert_eq!(
        failures(&holdout, &thresholds_for("holdout", "phase1", &[])),
        vec!["the gate set names no languages"]
    );
    let mut unlimited = thresholds_for("holdout", "phase1", &["tua"]);
    unlimited
        .sets
        .get_mut("holdout")
        .expect("holdout thresholds")
        .languages
        .insert("tua".to_owned(), LanguageThresholds::default());
    assert_eq!(
        failures(&holdout, &unlimited),
        vec!["this language has no limits"]
    );

    let sparse = gate_report(&[("holdout", SetRole::Holdout)], 1);
    assert!(
        failures(&sparse, &thresholds_for("holdout", "phase1", &["tua"]))
            .contains(&"min_suggestions")
    );
}

#[test]
fn gate_sets_check_only_their_configured_languages() {
    let mut report = gate_report(&[("external", SetRole::External)], 4);
    let backend = report.sets[0]
        .backends
        .get_mut("phase1")
        .expect("phase1 backend");
    let lua = backend.by_language.remove("tua").expect("fixture language");
    backend.by_language.insert("lua".to_owned(), lua);

    let gates = check_gates(&report, &thresholds_for("external", "phase1", &["lua"]));
    assert!(gates.iter().all(|gate| gate.language == "lua"));
    assert!(!gates
        .iter()
        .any(|gate| gate.check == "no eligible requests in this language"));
}

#[test]
fn aggregate_phase_gate_fails_closed_for_missing_pending_or_evidence_free_items() {
    let mut artifact = PhaseGateArtifact {
        version: 1,
        prerequisites: PHASE1_REQUIRED_PREREQUISITES
            .into_iter()
            .map(|name| {
                (
                    name.to_owned(),
                    PhaseGatePrerequisite {
                        passed: true,
                        evidence: "artifact.json".to_owned(),
                    },
                )
            })
            .collect(),
    };
    assert!(check_phase_gate(&artifact).is_empty());
    artifact.prerequisites.remove("menu_replay");
    artifact
        .prerequisites
        .get_mut("scheduler_cache_hit")
        .expect("scheduler prerequisite")
        .passed = false;
    artifact
        .prerequisites
        .get_mut("completion_performance")
        .expect("performance prerequisite")
        .evidence
        .clear();
    let failures = check_phase_gate(&artifact).join("\n");
    assert!(failures.contains("missing prerequisite `menu_replay`"));
    assert!(failures.contains("scheduler_cache_hit` is not passed"));
    assert!(failures.contains("completion_performance` has no evidence"));
}

#[test]
fn per_source_rates_divide_by_every_eligible_request() {
    let mut accumulator = BackendAccumulator::default();
    let keys = Keys {
        abstain: false,
        language: "tua",
        scenario: "edit",
        menu: "closed",
        context: "comma",
        prefix: "token-end",
        document: "parses",
        project: "p",
    };
    let candidate = Candidate {
        text: "x".to_owned(),
        is_snippet: false,
        source: "current-file",
        score: 1.0,
        tier: None,
    };
    let outcome = score("x", false, "x");
    accumulator.add(&keys, Some(&candidate), Some(outcome), 1, "not-applicable");
    for _ in 0..3 {
        accumulator.add(&keys, None, None, 1, "not-applicable");
    }
    let report = accumulator.finish();
    let source = &report.by_source["current-file"];
    assert_eq!((source.eligible, source.suggested), (4, 1));
    assert!((source.suggestions_per_eligible - 0.25).abs() < 1e-9);
}

#[test]
fn phase2_retrieval_reports_holdouts_states_tiers_and_index_costs() {
    let project = EvalProject {
        name: "phase2-fixture".to_owned(),
        config: crate::ProjectConfig::default(),
        source_roots: vec![PathBuf::from("src")],
        output_roots: vec![],
        asset_roots: vec![],
        files: vec![
            EvalFile {
                path: PathBuf::from("src/main.tua"),
                kind: AnalysisSourceKind::Tua,
                source: "draw(player, score)\ndraw(pla".to_owned(),
            },
            EvalFile {
                path: PathBuf::from("src/render.tua"),
                kind: AnalysisSourceKind::Tua,
                source: "draw(player)\nprint(player)".to_owned(),
            },
            EvalFile {
                path: PathBuf::from("one/a.tua"),
                kind: AnalysisSourceKind::Tua,
                source: "draw(player, score)".to_owned(),
            },
            EvalFile {
                path: PathBuf::from("two/b.tua"),
                kind: AnalysisSourceKind::Tua,
                source: "draw(player, score)".to_owned(),
            },
        ],
    };
    let options = EvalOptions {
        max_positions_per_file: 8,
        simulate_menu: false,
        measure_warmup: false,
        parse_repeats: 1,
        phase2_retrieval: true,
        keep_records: true,
        ..EvalOptions::default()
    };
    let report = evaluate(
        &[EvalSet {
            name: "tune".to_owned(),
            role: SetRole::Tune,
            projects: vec![project],
        }],
        DuplicateReport::default(),
        &options,
    );
    let set = &report.sets[0];
    assert_eq!(set.retrieval_indexes.len(), 1);
    assert_eq!(set.retrieval_indexes[0].1.files_indexed, 4);
    assert!(set.retrieval_indexes[0].1.build_ms >= 0.0);
    assert!(set.retrieval_indexes[0].1.complete_bytes > 0);
    assert!(set.backends.contains_key("project-file-holdout-cold"));
    assert!(set.backends.contains_key("project-line-holdout-complete"));
    assert!(set
        .backends
        .contains_key("same-directory-only-file-holdout-complete"));
    assert!(set
        .backends
        .contains_key("project-only-file-holdout-complete"));
    assert!(set
        .backends
        .contains_key("phase2-tuning-phase1-plus-project-common-prefix"));
    assert!(set.backends.contains_key("bundled-corpus"));
    assert!(set
        .bundled_corpus
        .as_ref()
        .is_some_and(|corpus| corpus.approximate_bytes > 0));
    assert!(set.records.iter().any(|record| {
        record.backend == "phase2-tuning-phase1-plus-project-common-prefix"
            && record
                .candidate
                .as_ref()
                .is_some_and(|candidate| candidate.source == "project")
    }));
    let complete = &set.backends["project-line-holdout-complete"];
    assert!(complete.by_index_state["complete"].eligible > 0);
    assert!(complete.by_tier.contains_key("current-file"));
}

#[test]
#[should_panic(expected = "Phase 2 retrieval is tuning-only")]
fn phase2_retrieval_cannot_evaluate_gate_sets_through_the_core_api() {
    evaluate(
        &[EvalSet {
            name: "holdout".to_owned(),
            role: SetRole::Holdout,
            projects: vec![],
        }],
        DuplicateReport::default(),
        &EvalOptions {
            phase2_retrieval: true,
            ..EvalOptions::default()
        },
    );
}

#[test]
#[should_panic(expected = "Phase 3 n-grams are tuning-only")]
fn phase3_ngram_cannot_evaluate_gate_sets_through_the_core_api() {
    evaluate(
        &[EvalSet {
            name: "external".to_owned(),
            role: SetRole::External,
            projects: vec![],
        }],
        DuplicateReport::default(),
        &EvalOptions {
            phase3_ngram: true,
            ..EvalOptions::default()
        },
    );
}

#[test]
fn phase3_reports_model_costs_and_both_experimental_backends() {
    let project = EvalProject {
        name: "phase3-fixture".to_owned(),
        config: crate::ProjectConfig::default(),
        source_roots: vec![PathBuf::from("src")],
        output_roots: vec![],
        asset_roots: vec![],
        files: vec![
            EvalFile {
                path: PathBuf::from("src/main.tua"),
                kind: AnalysisSourceKind::Tua,
                source: "value = value + 1\n".to_owned(),
            },
            EvalFile {
                path: PathBuf::from("src/other.tua"),
                kind: AnalysisSourceKind::Tua,
                source: "score = score + 1\n".to_owned(),
            },
        ],
    };
    let report = evaluate(
        &[EvalSet {
            name: "tune".to_owned(),
            role: SetRole::Tune,
            projects: vec![project],
        }],
        DuplicateReport::default(),
        &EvalOptions {
            max_positions_per_file: 0,
            simulate_menu: false,
            measure_warmup: false,
            parse_repeats: 1,
            phase2_retrieval: true,
            phase3_ngram: true,
            ..EvalOptions::default()
        },
    );
    let set = &report.sets[0];
    assert_eq!(set.ngram_indexes.len(), 1);
    assert!(set.ngram_indexes[0].1.approximate_bytes > 0);
    assert!(set.backends.contains_key("phase3-ngram"));
    assert!(set
        .backends
        .contains_key("phase3-phase2-plus-ngram-fallback"));
}

#[test]
fn peeking_never_builds_and_matches_only_the_current_source() {
    let path = PathBuf::from("src/main.tua");
    let mut host = AnalysisHost::new();
    host.set_file_with_kind(
        &path,
        "function love.update(dt: number)\nend\n",
        AnalysisSourceKind::Tua,
    );

    let before = host.cache_stats();
    assert_eq!(
        host.peek_declared_love_callbacks(&path),
        None,
        "nothing is cached yet"
    );
    assert_eq!(host.cache_stats(), before, "a miss builds nothing");

    host.warm_file(&path, AnalysisWarmPriority::High);
    let warmed = host.cache_stats();
    let declared = host
        .peek_declared_love_callbacks(&path)
        .expect("cached facts");
    assert_eq!(declared, BTreeSet::from(["update".to_owned()]));
    assert_eq!(host.cache_stats(), warmed, "a hit records nothing");

    host.open_document_with_kind(
        &path,
        "function love.draw()\nend\n",
        AnalysisSourceKind::Tua,
    );
    assert_eq!(
        host.peek_declared_love_callbacks(&path),
        None,
        "a stale entry is never returned"
    );
}
