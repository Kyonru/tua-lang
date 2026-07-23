use super::*;

#[derive(Clone, Copy)]
pub(super) struct CheckQuery<'a> {
    parsed: &'a ParsedSource,
    options: CheckQueryOptions<'a>,
}

#[derive(Clone, Copy)]
pub(super) struct CheckQueryOptions<'a> {
    asset_roots: &'a [PathBuf],
    diagnostics_config: &'a DiagnosticsConfig,
    jit_config: &'a JitConfig,
    domains: BuiltinDomainRegistry,
    asset_index: Option<&'a asset::AssetIndex>,
}

impl<'a> CheckQueryOptions<'a> {
    pub(super) fn new(
        asset_roots: &'a [PathBuf],
        diagnostics_config: &'a DiagnosticsConfig,
        jit_config: &'a JitConfig,
        ecs_config: &'a EcsConfig,
        asset_index: Option<&'a asset::AssetIndex>,
    ) -> Self {
        Self {
            asset_roots,
            diagnostics_config,
            jit_config,
            domains: BuiltinDomainRegistry::with_ecs(ecs_config.enabled),
            asset_index,
        }
    }
}

impl<'a> CheckQuery<'a> {
    pub(super) fn new(parsed: &'a ParsedSource, options: CheckQueryOptions<'a>) -> Self {
        Self { parsed, options }
    }

    pub(super) fn run(self) -> Result<(), Vec<Diagnostic>> {
        let environment = {
            let _trace = profile::diagnostics_span(DiagnosticsTraceCategory::TypeEnvironment);
            let (environment, diagnostics) =
                build_type_environment_with_domain_registry(self.parsed, self.options.domains);

            if diagnostics.is_empty() {
                environment
            } else {
                return Err(diagnostics);
            }
        };

        self.run_with_environment(&environment)
    }

    pub(super) fn run_with_environment(
        self,
        environment: &TypeEnvironment,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut checker = Checker::new(
            self.parsed,
            environment,
            self.options.asset_roots,
            self.options.asset_index,
        );

        {
            let _trace = profile::diagnostics_span(DiagnosticsTraceCategory::Checker);

            checker.check_block(self.parsed.ast.nodes(), None);
            checker.remap_parser_diagnostics();
            checker
                .diagnostics
                .extend(classes::class_contract_diagnostics(
                    self.parsed,
                    environment,
                ));

            if self.options.diagnostics_config.doc_annotations {
                checker
                    .diagnostics
                    .extend(self.parsed.lua_doc_diagnostics.iter().cloned());
            }

            checker
                .diagnostics
                .extend(self.options.domains.diagnostics(self.parsed, environment));
        }

        checker.diagnostics.extend(lint_parsed_source(
            self.parsed,
            self.options.diagnostics_config,
            self.options.jit_config,
            environment,
        ));

        checker.diagnostics = apply_inline_ignores(
            &self.parsed.path,
            &self.parsed.source,
            checker.diagnostics,
            self.options.diagnostics_config,
        );

        if checker.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(checker.diagnostics)
        }
    }
}
