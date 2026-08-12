use super::*;

pub(crate) struct WatchBuildSession {
    cwd: PathBuf,
    workspace: ProjectWorkspace,
    out_dir: PathBuf,
    in_place: bool,
    host: AnalysisHost,
}

impl WatchBuildSession {
    pub(crate) fn start(
        cwd: impl AsRef<Path>,
        loaded: LoadedConfig,
    ) -> Result<(Self, BuildOutcome), CliError> {
        let cwd = cwd.as_ref().to_path_buf();
        let pipeline_outcome = crate::asset_pipeline::run_asset_pipelines(&cwd, &loaded.config);
        let workspace = ProjectWorkspace::from_loaded(&cwd, loaded);
        let source_dir = workspace.primary_source_root().to_path_buf();
        let out_dir = workspace.primary_output_root().to_path_buf();
        let in_place = paths_same_location(&source_dir, &out_dir);
        let mut source_files = discover_build_files(
            &source_dir,
            &workspace.config().extension,
            (!in_place).then_some(out_dir.as_path()),
        )
        .map_err(|error| {
            CliError::from_diagnostics("failed to discover source files", vec![error])
        })?;
        source_files.retain(|source_file| !workspace.is_type_library_path(&source_file.path));
        let mut host = AnalysisHost::with_config(workspace.config().clone());

        host.set_source_roots(workspace.source_roots().iter().cloned());
        host.set_output_roots(workspace.output_roots().iter().cloned());
        host.set_asset_roots(workspace.asset_roots());

        let mut session = Self {
            cwd,
            workspace,
            out_dir,
            in_place,
            host,
        };
        let mut diagnostics = pipeline_outcome.diagnostics;
        diagnostics.extend(load_type_libraries(&mut session.host, &session.workspace));
        let mut tua_paths = BTreeSet::new();
        let mut asset_paths = BTreeSet::new();
        let tua_output_owners = tua_output_owners_for_paths(
            &session.workspace,
            source_files
                .iter()
                .filter(|source_file| source_file.kind == SourceKind::Tua)
                .map(|source_file| source_file.path.clone()),
        );

        for source_file in source_files {
            match source_file.kind {
                SourceKind::Tua => {
                    tua_paths.insert(source_file.path.clone());
                    session.load_semantic_file(
                        &source_file.path,
                        AnalysisSourceKind::Tua,
                        &mut diagnostics,
                    );
                }
                SourceKind::Asset => {
                    let output_path = output_path_for_asset(&session.workspace, &source_file.path);

                    if let Some(tua_source_path) = tua_output_owners
                        .get(&output_path)
                        .and_then(|owners| owners.first())
                    {
                        diagnostics.push(output_collision_diagnostic(
                            &source_file.path,
                            tua_source_path,
                            session.in_place,
                        ));
                        continue;
                    }

                    if session.in_place {
                        continue;
                    }

                    if analysis_kind_for_path(
                        &source_file.path,
                        &session.workspace.config().extension,
                    ) == Some(AnalysisSourceKind::Lua)
                    {
                        session.load_semantic_file(
                            &source_file.path,
                            AnalysisSourceKind::Lua,
                            &mut diagnostics,
                        );
                    }

                    asset_paths.insert(source_file.path);
                }
            }
        }

        let snapshot = session.host.snapshot();
        let outcome =
            session.build_paths(&snapshot, tua_paths, asset_paths, diagnostics, true, false);

        Ok((session, outcome))
    }

    pub(crate) fn rebuild(&mut self, changed: Vec<PathBuf>, deleted: Vec<PathBuf>) -> BuildOutcome {
        let before = self.host.snapshot();
        let extension = self.workspace.config().extension.clone();
        let mut diagnostics = Vec::new();
        let mut changed_tua = BTreeSet::new();
        let mut changed_assets = BTreeSet::new();
        let mut surface_roots = BTreeSet::new();

        for path in deleted {
            if analysis_kind_for_path(&path, &extension).is_some() {
                surface_roots.insert(path.clone());
                self.host.remove_file(&path);
            }
        }

        for path in changed {
            if self.workspace.is_type_library_path(&path) {
                match read_source_file(&path) {
                    Ok(source) => {
                        let _ =
                            self.host
                                .update_file_with_kind(&path, source, AnalysisSourceKind::Lua);
                        surface_roots.insert(path);
                    }
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        surface_roots.insert(path.clone());
                        self.host.remove_file(path);
                    }
                }
                continue;
            }

            let source_kind = SourceKind::for_path(&path, &extension);

            match source_kind {
                SourceKind::Tua => {
                    changed_tua.insert(path.clone());
                }
                SourceKind::Asset => {
                    changed_assets.insert(path.clone());
                }
            }

            let Some(analysis_kind) = analysis_kind_for_path(&path, &extension) else {
                continue;
            };
            let added = before.source(&path).is_none();

            match read_source_file(&path) {
                Ok(source) => {
                    let surface_changed =
                        self.host
                            .update_file_with_kind(&path, source, analysis_kind);

                    if added || surface_changed {
                        surface_roots.insert(path);
                    }
                }
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    surface_roots.insert(path.clone());
                    self.host.remove_file(path);
                }
            }
        }

        let snapshot = self.host.snapshot();
        let mut affected_tua = changed_tua;

        if !surface_roots.is_empty() {
            let affected = before
                .module_graph()
                .reverse_dependency_closure(&surface_roots)
                .into_iter()
                .chain(
                    snapshot
                        .module_graph()
                        .reverse_dependency_closure(&surface_roots),
                );

            affected_tua.extend(affected.filter(|path| {
                SourceKind::for_path(path, &extension) == SourceKind::Tua
                    && snapshot.source(path).is_some()
            }));
        }

        self.build_paths(
            &snapshot,
            affected_tua,
            changed_assets,
            diagnostics,
            false,
            true,
        )
    }

    fn load_semantic_file(
        &mut self,
        path: &Path,
        kind: AnalysisSourceKind,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match read_source_file(path) {
            Ok(source) => self.host.set_file_with_kind(path, source, kind),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    fn build_paths(
        &self,
        snapshot: &tua_core::AnalysisSnapshot,
        tua_paths: BTreeSet<PathBuf>,
        asset_paths: BTreeSet<PathBuf>,
        mut diagnostics: Vec<Diagnostic>,
        stop_on_any_error: bool,
        warn_on_sibling_collisions: bool,
    ) -> BuildOutcome {
        let mut blocked = BTreeSet::new();
        let tua_output_collisions = colliding_tua_outputs(&tua_output_owners_for_paths(
            &self.workspace,
            self.host.file_paths().into_iter().filter(|path| {
                SourceKind::for_path(path, &self.workspace.config().extension) == SourceKind::Tua
                    && snapshot.source(path).is_some()
            }),
        ));
        let relevant_tua_output_collisions =
            filter_tua_output_collisions_for_sources(&tua_output_collisions, &tua_paths);

        push_tua_output_collision_diagnostics(&mut diagnostics, &relevant_tua_output_collisions);
        remove_colliding_outputs(&tua_output_collisions, &mut diagnostics);

        for path in &tua_paths {
            if snapshot.source(path).is_none() {
                continue;
            }

            let mut source_diagnostics = snapshot.pipeline_diagnostics(path);

            if has_errors(&source_diagnostics) {
                blocked.insert(path.clone());
            }
            diagnostics.append(&mut source_diagnostics);
        }

        if stop_on_any_error && has_errors(&diagnostics) {
            return BuildOutcome {
                emitted: 0,
                diagnostics,
            };
        }

        let mut emitted = 0;
        let mut required_bundled_runtimes = BTreeSet::new();
        let annotation_mode = self.workspace.config().emit.annotation_mode;

        for source_path in tua_paths {
            if blocked.contains(&source_path) {
                continue;
            }
            if tua_core::is_tua_declaration_path(&source_path, &self.workspace.config().extension) {
                remove_declaration_outputs(&self.workspace, &source_path, &mut diagnostics);
                continue;
            }
            let Some(source) = snapshot.source(&source_path) else {
                continue;
            };
            if warn_on_sibling_collisions {
                push_sibling_collision_warning(&mut diagnostics, &source_path, self.in_place);
            }

            let emission = match emit_tua_source_with_workspace_artifact(
                &source_path,
                source,
                annotation_mode,
                &self.workspace,
            ) {
                Ok(output) => output,
                Err(mut source_diagnostics) => {
                    diagnostics.append(&mut source_diagnostics);
                    continue;
                }
            };
            let output = emission.code;
            collect_required_bundled_runtimes(&output, &mut required_bundled_runtimes);

            for output_path in output_paths_for_tua_source(&self.workspace, &source_path) {
                if tua_output_collisions.contains_key(&output_path) {
                    continue;
                }

                match write_output(&output_path, output.clone()) {
                    Ok(()) => {
                        emitted += 1;

                        if self.workspace.config().emit_maps {
                            if let Err(diagnostic) = write_source_map(
                                &self.cwd,
                                &source_path,
                                &output_path,
                                &emission.provenance,
                            ) {
                                diagnostics.push(diagnostic);
                            }
                        } else {
                            remove_source_maps(&output_path, &mut diagnostics);
                        }
                    }
                    Err(diagnostic) => diagnostics.push(diagnostic),
                }
            }
        }

        for source_path in asset_paths {
            if self.in_place {
                continue;
            }
            let output_path = output_path_for_asset(&self.workspace, &source_path);

            if let Some(tua_source_path) =
                colliding_tua_source_for_asset(&source_path, &self.workspace.config().extension)
            {
                diagnostics.push(output_collision_diagnostic(
                    &source_path,
                    &tua_source_path,
                    self.in_place,
                ));
                continue;
            }

            match copy_output(&source_path, &output_path) {
                Ok(()) => emitted += 1,
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }

        write_required_bundled_runtimes(
            &self.out_dir,
            &required_bundled_runtimes,
            &mut emitted,
            &mut diagnostics,
        );

        BuildOutcome {
            emitted,
            diagnostics,
        }
    }
}
