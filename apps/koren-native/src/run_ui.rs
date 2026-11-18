//! Run configuration state, persistence, preview, and approved execution for `koren-native`.

use koren_cargo::CargoProvider;
use koren_extensions::{ExtensionManager, ExtensionState};
use koren_projects::{DiscoveryLimits, ProjectManager};
use koren_run::{
    OutputDestination, ResolvedRun, RunConfiguration, RunContext, SavePolicy, load_run_file,
    parse_arguments, resolve as resolve_run, save_run_file, validate as validate_run,
};
use koren_tasks::{
    ExecutionRequestId, TaskApprovalManager, TaskAuthorizationDecision, TaskCaller, TaskDefinition,
    TaskExecutionPolicy, TaskExecutionRequestOutcome, TaskExecutionSnapshot, TaskId, TaskManager,
    TaskOptions, TaskRunId, TaskState,
};
use koren_terminal::{
    TerminalEvent, TerminalId, TerminalManager, TerminalOptions, TerminalProfile,
    TerminalWorkingDirectory,
};
use koren_tools::{
    ToolArch, ToolCategory, ToolDeclaration, ToolManager, ToolPlatform, ToolTrustPolicy,
};
use koren_workspace::WorkspaceManager;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunIdentity {
    pub source: String,
    pub owner: String,
    pub id: String,
}

#[derive(Clone, Debug)]
pub struct RunEntry {
    pub identity: RunIdentity,
    pub configuration: RunConfiguration,
    pub editable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunSettings {
    pub default_output: OutputDestination,
    pub reveal_on_run: bool,
    pub reveal_on_failure: bool,
    pub default_save: SavePolicy,
    pub default_timeout_ms: Option<u64>,
    pub keep_history: bool,
    pub confirm_before_stop: bool,
}

impl Default for RunSettings {
    fn default() -> Self {
        Self {
            default_output: OutputDestination::Run,
            reveal_on_run: true,
            reveal_on_failure: true,
            default_save: SavePolicy::Ask,
            default_timeout_ms: None,
            keep_history: true,
            confirm_before_stop: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PendingRunApproval {
    pub identity: RunIdentity,
    pub name: String,
    pub resolved: ResolvedRun,
    pub output: OutputDestination,
    pub task_id: Option<TaskId>,
    pub request_id: Option<ExecutionRequestId>,
    pub terminal_id: Option<TerminalId>,
    pub disk_warning: bool,
}

#[derive(Clone, Debug)]
pub struct ActiveTaskRun {
    pub name: String,
    pub run_id: TaskRunId,
    pub task_id: TaskId,
    pub state: TaskState,
    pub stdout_len: usize,
    pub stderr_len: usize,
}

#[derive(Clone, Debug)]
pub struct RunDraft {
    pub editing_identity: Option<RunIdentity>,
    pub scope: String,
    pub id: String,
    pub name: String,
    pub executable: String,
    pub args_simple: String,
    pub args_vector: Vec<String>,
    pub advanced_args_mode: bool,
    pub cwd: String,
    pub output: OutputDestination,
    pub save: SavePolicy,
    pub languages_csv: String,
    pub timeout_ms_text: String,
    pub show_advanced: bool,
    pub error: Option<String>,
}

impl RunDraft {
    pub fn new_custom(default_output: OutputDestination, default_save: SavePolicy) -> Self {
        Self {
            editing_identity: None,
            scope: "workspace".to_owned(),
            id: String::new(),
            name: String::new(),
            executable: String::new(),
            args_simple: String::new(),
            args_vector: Vec::new(),
            advanced_args_mode: false,
            cwd: "${workspace}".to_owned(),
            output: default_output,
            save: default_save,
            languages_csv: String::new(),
            timeout_ms_text: String::new(),
            show_advanced: false,
            error: None,
        }
    }

    pub fn from_entry(entry: &RunEntry, duplicate_to_user: bool) -> Self {
        let args_simple = entry
            .configuration
            .args
            .iter()
            .map(|arg| {
                if arg.is_empty() || arg.contains([' ', '"', '\\']) {
                    format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
                } else {
                    arg.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        Self {
            editing_identity: if duplicate_to_user {
                None
            } else {
                Some(entry.identity.clone())
            },
            scope: if duplicate_to_user || entry.identity.source == "user" {
                "user".to_owned()
            } else {
                "workspace".to_owned()
            },
            id: if duplicate_to_user {
                format!("{}-copy", entry.configuration.id)
            } else {
                entry.configuration.id.clone()
            },
            name: if duplicate_to_user {
                format!("{} (Copy)", entry.configuration.name)
            } else {
                entry.configuration.name.clone()
            },
            executable: entry.configuration.executable.clone().unwrap_or_default(),
            args_simple,
            args_vector: entry.configuration.args.clone(),
            advanced_args_mode: false,
            cwd: if entry.configuration.cwd.is_empty() {
                "${workspace}".to_owned()
            } else {
                entry.configuration.cwd.clone()
            },
            output: entry.configuration.output,
            save: entry.configuration.save,
            languages_csv: entry.configuration.languages.join(", "),
            timeout_ms_text: entry
                .configuration
                .timeout_ms
                .map_or_else(String::new, |v| v.to_string()),
            show_advanced: false,
            error: None,
        }
    }

    pub fn to_configuration(&self) -> Result<RunConfiguration, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Name is required.".to_owned());
        }
        let id = if self.id.trim().is_empty() {
            slugify_id(name)
        } else {
            self.id.trim().to_owned()
        };
        let args = if self.advanced_args_mode {
            self.args_vector.clone()
        } else {
            parse_arguments(&self.args_simple).map_err(|e| e.to_string())?
        };
        let languages = self
            .languages_csv
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let timeout_ms = if self.timeout_ms_text.trim().is_empty() {
            None
        } else {
            Some(
                self.timeout_ms_text
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| "Timeout must be a positive integer (ms).".to_owned())?,
            )
        };
        let config = RunConfiguration {
            id,
            name: name.to_owned(),
            executable: Some(self.executable.trim().to_owned()),
            tool: None,
            args,
            cwd: self.cwd.trim().to_owned(),
            environment: BTreeMap::new(),
            languages,
            output: self.output,
            save: self.save,
            timeout_ms,
            debug_adapter: None,
        };
        validate_run(&config).map_err(|e| e.to_string())?;
        Ok(config)
    }
}

fn slugify_id(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_owned();
    if trimmed.is_empty() {
        "custom-run".to_owned()
    } else {
        trimmed
    }
}

pub fn executable_exists(executable: &str) -> bool {
    let trimmed = executable.trim();
    if trimmed.is_empty() {
        return false;
    }
    let path = Path::new(trimmed);
    if path.components().count() > 1 || path.is_absolute() {
        return path.is_file();
    }
    let Some(path_env) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_env).any(|dir| dir.join(trimmed).is_file())
}

struct AlwaysRequireApproval;
impl TaskExecutionPolicy for AlwaysRequireApproval {
    fn decide(&self, _request: &TaskExecutionSnapshot) -> TaskAuthorizationDecision {
        TaskAuthorizationDecision::RequireApproval
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolSettings {
    pub show_system_installations: bool,
    pub confirm_before_download: bool,
    pub show_incompatible_versions: bool,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            show_system_installations: true,
            confirm_before_download: true,
            show_incompatible_versions: false,
        }
    }
}

pub struct RunCoordinator {
    pub config_dir: PathBuf,
    pub projects: ProjectManager,
    pub cargo: CargoProvider,
    pub tasks: TaskManager,
    pub approvals: TaskApprovalManager,
    pub terminals: TerminalManager,
    pub tools: ToolManager,
    pub extensions: ExtensionManager,
    pub tool_settings: ToolSettings,
    pub entries: Vec<RunEntry>,
    pub selected: Option<RunIdentity>,
    pub last_run: Option<RunIdentity>,
    pub settings: RunSettings,
    pub pending_approval: Option<PendingRunApproval>,
    pub active_task: Option<ActiveTaskRun>,
    pub active_terminal: Option<TerminalId>,
    pub terminal_lines: Vec<String>,
    next_ephemeral: u64,
}

impl RunCoordinator {
    pub fn new(config_dir: PathBuf) -> Result<Self, String> {
        let mut projects = ProjectManager::new();
        let cargo = CargoProvider::new();
        cargo
            .register(&mut projects)
            .map_err(|e| format!("cargo provider registration failed: {e}"))?;
        let tasks = TaskManager::new(TaskOptions::default()).map_err(|e| e.to_string())?;
        let approvals = TaskApprovalManager::new();
        let terminals =
            TerminalManager::new(TerminalOptions::default()).map_err(|e| e.to_string())?;
        let tools_root = config_dir.join("tools");
        let tools =
            ToolManager::new(tools_root, ToolTrustPolicy::default()).map_err(|e| e.to_string())?;
        let mut extensions = ExtensionManager::new();
        let ext_dir = std::env::var_os("KOREN_EXTENSIONS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| config_dir.join("extensions"));
        if ext_dir.is_dir() {
            let _ = extensions.discover_installed(&ext_dir);
        }
        Ok(Self {
            config_dir,
            projects,
            cargo,
            tasks,
            approvals,
            terminals,
            tools,
            extensions,
            tool_settings: ToolSettings::default(),
            entries: Vec::new(),
            selected: None,
            last_run: None,
            settings: RunSettings::default(),
            pending_approval: None,
            active_task: None,
            active_terminal: None,
            terminal_lines: Vec::new(),
            next_ephemeral: 0,
        })
    }

    pub fn user_runs_path(&self) -> PathBuf {
        self.config_dir.join("runs.toml")
    }

    pub fn selection_path(&self) -> PathBuf {
        self.config_dir.join("run-selections.json")
    }

    pub fn refresh(&mut self, workspace_root: Option<&Path>) -> Result<(), String> {
        let mut entries = Vec::new();
        let user_file = load_run_file(&self.user_runs_path()).map_err(|e| e.to_string())?;
        for configuration in user_file.runs {
            entries.push(RunEntry {
                identity: RunIdentity {
                    source: "user".to_owned(),
                    owner: String::new(),
                    id: configuration.id.clone(),
                },
                configuration,
                editable: true,
            });
        }
        if let Some(root) = workspace_root {
            let ws_path = root.join(".koren/runs.toml");
            let ws_file = load_run_file(&ws_path).map_err(|e| e.to_string())?;
            for configuration in ws_file.runs {
                entries.push(RunEntry {
                    identity: RunIdentity {
                        source: "workspace".to_owned(),
                        owner: String::new(),
                        id: configuration.id.clone(),
                    },
                    configuration,
                    editable: true,
                });
            }
            let mut ws_mgr = WorkspaceManager::new();
            if ws_mgr.open(root).is_ok()
                && let Ok(discovered) = self.cargo.discover(
                    &ws_mgr,
                    &mut self.projects,
                    DiscoveryLimits {
                        max_directories: 500,
                        max_projects: 16,
                        max_depth: 6,
                    },
                )
            {
                for item in discovered {
                    if let Ok(info) = self.projects.info(item.project_id) {
                        let owner = format!("{}:{}", info.provider_id.as_str(), info.id.get());
                        for task in self.cargo.tasks_for_project(info) {
                            if task.key == "cargo.metadata" {
                                continue;
                            }
                            entries.push(RunEntry {
                                identity: RunIdentity {
                                    source: "project".to_owned(),
                                    owner: owner.clone(),
                                    id: task.key.clone(),
                                },
                                configuration: RunConfiguration {
                                    id: task.key,
                                    name: task.display_name,
                                    executable: Some(
                                        task.executable.to_string_lossy().into_owned(),
                                    ),
                                    tool: None,
                                    args: task.arguments,
                                    cwd: task.working_directory.to_string_lossy().into_owned(),
                                    environment: task.environment,
                                    languages: Vec::new(),
                                    output: OutputDestination::Run,
                                    save: SavePolicy::Ask,
                                    timeout_ms: task
                                        .timeout
                                        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
                                    debug_adapter: None,
                                },
                                editable: false,
                            });
                        }
                    }
                }
            }
        }
        for summary in self.extensions.list() {
            if let Ok(contributions) = self.extensions.contributions(&summary.id) {
                for item in &contributions.run_configurations {
                    let output = match item.output.as_str() {
                        "terminal" => OutputDestination::Terminal,
                        _ => OutputDestination::Run,
                    };
                    let save = match item.save.as_str() {
                        "save" => SavePolicy::Save,
                        "disk" => SavePolicy::Disk,
                        _ => SavePolicy::Ask,
                    };
                    entries.push(RunEntry {
                        identity: RunIdentity {
                            source: "extension".to_owned(),
                            owner: summary.id.clone(),
                            id: item.id.clone(),
                        },
                        configuration: RunConfiguration {
                            id: item.id.clone(),
                            name: item.name.clone(),
                            executable: item.executable.clone(),
                            tool: item.tool.clone(),
                            args: item.arguments.clone(),
                            cwd: item.working_directory.clone(),
                            environment: item.environment.clone(),
                            languages: item.languages.clone(),
                            output,
                            save,
                            timeout_ms: item.timeout_ms,
                            debug_adapter: item.debug_adapter.clone(),
                        },
                        editable: false,
                    });
                }
            }
        }
        self.entries = entries;
        if let Some(root) = workspace_root {
            let selections = read_selections(&self.selection_path());
            let key = root.to_string_lossy().into_owned();
            if let Some(saved) = selections.get(&key) {
                if self.entries.iter().any(|e| &e.identity == saved) {
                    self.selected = Some(saved.clone());
                } else {
                    self.selected = self.entries.first().map(|e| e.identity.clone());
                }
            } else if self
                .selected
                .as_ref()
                .is_none_or(|sel| !self.entries.iter().any(|e| &e.identity == sel))
            {
                self.selected = self.entries.first().map(|e| e.identity.clone());
            }
        }
        Ok(())
    }

    pub fn recommend_extension_for_file(&self, path: &Path) -> Option<(String, String, String)> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)?;
        if ext.is_empty() {
            return None;
        }
        let mut active_supports = false;
        let mut candidate: Option<(String, String)> = None;
        for summary in self.extensions.list() {
            let Ok((manifest, _)) = self.extensions.manifest(&summary.id) else {
                continue;
            };
            let matches_ext = manifest
                .languages
                .iter()
                .any(|lang| lang.extensions.iter().any(|e| e.eq_ignore_ascii_case(&ext)));
            if !matches_ext {
                continue;
            }
            if summary.state == ExtensionState::Active {
                active_supports = true;
                break;
            }
            if candidate.is_none() || summary.id == "org.koren.rust" {
                candidate = Some((summary.id.clone(), summary.name.clone()));
            }
        }
        if active_supports {
            return None;
        }
        candidate.map(|(id, name)| (id, name, ext))
    }

    pub fn enable_and_activate_extension(
        &mut self,
        ext_id: &str,
        workspace_root: Option<&Path>,
    ) -> Result<Option<String>, String> {
        let (manifest, _) = self
            .extensions
            .manifest(ext_id)
            .map_err(|e| e.to_string())?;
        let manifest_tools = manifest.tools.clone();
        self.extensions.enable(ext_id).map_err(|e| e.to_string())?;
        self.extensions
            .activate(ext_id, |_, _| Ok(()))
            .map_err(|e| e.to_string())?;
        let mut first_missing_tool = None;
        let ws_key = workspace_root.map(|p| p.to_string_lossy().into_owned());
        for tool in manifest_tools {
            let decl = ToolDeclaration {
                id: tool.id.clone(),
                name: tool.name.clone(),
                description: tool.description.clone(),
                category: match tool.category.as_str() {
                    "language-server" => ToolCategory::LanguageServer,
                    "runtime" => ToolCategory::Runtime,
                    "compiler" => ToolCategory::Compiler,
                    "formatter" => ToolCategory::Formatter,
                    "linter" => ToolCategory::Linter,
                    "debugger" => ToolCategory::Debugger,
                    _ => ToolCategory::BuildTool,
                },
                optional: tool.optional,
                executables: tool.executables.clone(),
                discovery_hints: tool.discovery_hints.clone(),
                platforms: tool
                    .platforms
                    .iter()
                    .filter_map(|p| match p.as_str() {
                        "linux" => Some(ToolPlatform::Linux),
                        "mac_os" | "macos" => Some(ToolPlatform::MacOs),
                        "windows" => Some(ToolPlatform::Windows),
                        _ => None,
                    })
                    .collect(),
                architectures: tool
                    .architectures
                    .iter()
                    .filter_map(|a| match a.as_str() {
                        "x86_64" | "x64" => Some(ToolArch::X86_64),
                        "aarch64" | "arm64" => Some(ToolArch::Aarch64),
                        _ => None,
                    })
                    .collect(),
                version_requirement: tool.version_requirement.clone(),
                releases: Vec::new(),
            };
            self.tools
                .register_declaration(ext_id, decl)
                .map_err(|e| e.to_string())?;
            if first_missing_tool.is_none()
                && !self.tools.is_prompt_dismissed(&tool.id)
                && self
                    .tools
                    .resolve(&tool.id, None, ws_key.as_deref())
                    .is_err()
            {
                first_missing_tool = Some(tool.id);
            }
        }
        self.refresh(workspace_root)?;
        Ok(first_missing_tool)
    }

    pub fn select(&mut self, identity: Option<RunIdentity>, workspace_root: Option<&Path>) {
        self.selected = identity.clone();
        if let Some(root) = workspace_root {
            let path = self.selection_path();
            let mut map = read_selections(&path);
            let key = root.to_string_lossy().into_owned();
            if let Some(id) = identity {
                map.insert(key, id);
            } else {
                map.remove(&key);
            }
            let _ = write_selections(&path, &map);
        }
    }

    pub fn selected_entry(&self) -> Option<&RunEntry> {
        let sel = self.selected.as_ref()?;
        self.entries.iter().find(|e| &e.identity == sel)
    }

    pub fn save_configuration(
        &mut self,
        scope: &str,
        configuration: RunConfiguration,
        workspace_root: Option<&Path>,
    ) -> Result<RunIdentity, String> {
        let path = match scope {
            "user" => self.user_runs_path(),
            "workspace" => workspace_root
                .map(|r| r.join(".koren/runs.toml"))
                .ok_or_else(|| "No workspace is open.".to_owned())?,
            _ => return Err("Scope must be user or workspace.".to_owned()),
        };
        let mut file = load_run_file(&path).map_err(|e| e.to_string())?;
        if let Some(pos) = file.runs.iter().position(|r| r.id == configuration.id) {
            file.runs[pos] = configuration.clone();
        } else {
            file.runs.push(configuration.clone());
        }
        save_run_file(&path, &file).map_err(|e| e.to_string())?;
        let identity = RunIdentity {
            source: scope.to_owned(),
            owner: String::new(),
            id: configuration.id,
        };
        self.refresh(workspace_root)?;
        self.select(Some(identity.clone()), workspace_root);
        Ok(identity)
    }

    pub fn delete_configuration(
        &mut self,
        identity: &RunIdentity,
        workspace_root: Option<&Path>,
    ) -> Result<(), String> {
        let path = match identity.source.as_str() {
            "user" => self.user_runs_path(),
            "workspace" => workspace_root
                .map(|r| r.join(".koren/runs.toml"))
                .ok_or_else(|| "No workspace is open.".to_owned())?,
            _ => return Err("Only user and workspace configurations can be deleted.".to_owned()),
        };
        let mut file = load_run_file(&path).map_err(|e| e.to_string())?;
        file.runs.retain(|r| r.id != identity.id);
        save_run_file(&path, &file).map_err(|e| e.to_string())?;
        self.refresh(workspace_root)?;
        Ok(())
    }

    pub fn prepare_approval(
        &mut self,
        identity: &RunIdentity,
        workspace_root: &Path,
        active_file: Option<&Path>,
        disk_warning: bool,
    ) -> Result<PendingRunApproval, String> {
        let entry = self
            .entries
            .iter()
            .find(|e| &e.identity == identity)
            .ok_or_else(|| "Run configuration was not found.".to_owned())?
            .clone();
        let mut config_for_resolve = entry.configuration.clone();
        if let Some(ref tool_id) = entry.configuration.tool {
            let ws_key = workspace_root.to_string_lossy();
            let resolved_tool = self
                .tools
                .resolve(tool_id, None, Some(&ws_key))
                .map_err(|e| format!("tool:{tool_id}:{e}"))?;
            config_for_resolve.executable =
                Some(resolved_tool.executable_path.to_string_lossy().into_owned());
            config_for_resolve.tool = None;
        }
        let resolved = resolve_run(
            &config_for_resolve,
            &RunContext {
                workspace: workspace_root,
                file: active_file,
            },
        )
        .map_err(|e| e.to_string())?;
        if !resolved.cwd.is_dir() {
            return Err(format!(
                "Working directory does not exist: {}",
                resolved.cwd.display()
            ));
        }
        self.next_ephemeral += 1;
        if entry.configuration.output == OutputDestination::Terminal {
            let profile = TerminalProfile {
                id: format!("run:{}:{}", entry.identity.id, self.next_ephemeral),
                name: entry.configuration.name.clone(),
                executable: PathBuf::from(&resolved.executable),
                arguments: resolved.args.clone(),
                working_directory: TerminalWorkingDirectory::Fixed(resolved.cwd.clone()),
                environment: resolved.environment.clone(),
                fallback_executable: None,
            };
            let info = self
                .terminals
                .create_session_with_profile(profile, Some(workspace_root), None, 24, 80)
                .map_err(|e| e.to_string())?;
            let approval = PendingRunApproval {
                identity: entry.identity,
                name: entry.configuration.name,
                resolved,
                output: OutputDestination::Terminal,
                task_id: None,
                request_id: None,
                terminal_id: Some(info.id),
                disk_warning,
            };
            self.pending_approval = Some(approval.clone());
            return Ok(approval);
        }
        let task_id = self
            .tasks
            .register(TaskDefinition {
                key: format!("run:{}:{}", entry.identity.id, self.next_ephemeral),
                display_name: entry.configuration.name.clone(),
                category: koren_tasks::TaskCategory::Run,
                executable: PathBuf::from(&resolved.executable),
                arguments: resolved.args.clone(),
                working_directory: resolved.cwd.clone(),
                environment: resolved.environment.clone(),
                timeout: entry
                    .configuration
                    .timeout_ms
                    .or(self.settings.default_timeout_ms)
                    .map(Duration::from_millis),
                project_id: None,
            })
            .map_err(|e| e.to_string())?;
        let outcome = self
            .approvals
            .request_execution(
                &mut self.tasks,
                task_id,
                TaskCaller::InteractiveUser,
                workspace_root.to_path_buf(),
                &AlwaysRequireApproval,
                Duration::from_secs(120),
            )
            .map_err(|e| e.to_string())?;
        let request_id = match outcome {
            TaskExecutionRequestOutcome::ApprovalRequired(req) => Some(req.id),
            TaskExecutionRequestOutcome::Started(_) => None,
        };
        let approval = PendingRunApproval {
            identity: entry.identity,
            name: entry.configuration.name,
            resolved,
            output: OutputDestination::Run,
            task_id: Some(task_id),
            request_id,
            terminal_id: None,
            disk_warning,
        };
        self.pending_approval = Some(approval.clone());
        Ok(approval)
    }

    pub fn approve_pending(&mut self) -> Result<OutputDestination, String> {
        let Some(pending) = self.pending_approval.take() else {
            return Err("No pending run approval.".to_owned());
        };
        self.last_run = Some(pending.identity.clone());
        if pending.output == OutputDestination::Terminal {
            let tid = pending
                .terminal_id
                .ok_or_else(|| "Missing terminal session.".to_owned())?;
            self.terminals
                .start_session(tid)
                .map_err(|e| e.to_string())?;
            self.active_terminal = Some(tid);
            self.terminal_lines.push(format!(
                "[started] {} ({} {})",
                pending.name,
                pending.resolved.executable,
                pending.resolved.args.join(" ")
            ));
            return Ok(OutputDestination::Terminal);
        }
        let req_id = pending
            .request_id
            .ok_or_else(|| "Missing task approval request.".to_owned())?;
        let task_id = pending
            .task_id
            .ok_or_else(|| "Missing task definition.".to_owned())?;
        let run_id = self
            .approvals
            .approve(&mut self.tasks, req_id)
            .map_err(|e| e.to_string())?;
        self.active_task = Some(ActiveTaskRun {
            name: pending.name,
            run_id,
            task_id,
            state: TaskState::Running,
            stdout_len: 0,
            stderr_len: 0,
        });
        Ok(OutputDestination::Run)
    }

    pub fn deny_pending(&mut self) {
        if let Some(pending) = self.pending_approval.take() {
            if let Some(req_id) = pending.request_id {
                let _ = self.approvals.deny(req_id);
            }
            if let Some(task_id) = pending.task_id {
                self.tasks.unregister(task_id);
            }
            if let Some(tid) = pending.terminal_id {
                let _ = self.terminals.close_session(tid, false);
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.active_task
            .as_ref()
            .is_some_and(|t| matches!(t.state, TaskState::Pending | TaskState::Running))
            || self.active_terminal.is_some()
    }

    pub fn stop_active(&mut self) {
        if let Some(active) = &self.active_task {
            let _ = self.tasks.cancel(active.run_id);
        }
        if let Some(tid) = self.active_terminal.take() {
            let _ = self.terminals.close_session(tid, true);
            self.terminal_lines.push("[terminal closed]".to_owned());
        }
    }

    pub fn poll_events(&mut self, output_lines: &mut Vec<String>) -> Option<(String, bool)> {
        let mut finished_toast = None;
        if let Some(mut active) = self.active_task.take() {
            if let Ok(out) = self.tasks.output(active.run_id) {
                if out.stdout.len() > active.stdout_len {
                    let new_text = &out.stdout[active.stdout_len..];
                    for line in new_text.lines() {
                        output_lines.push(line.to_owned());
                    }
                    active.stdout_len = out.stdout.len();
                }
                if out.stderr.len() > active.stderr_len {
                    let new_text = &out.stderr[active.stderr_len..];
                    for line in new_text.lines() {
                        output_lines.push(line.to_owned());
                    }
                    active.stderr_len = out.stderr.len();
                }
            }
            if let Ok(snap) = self.tasks.status(active.run_id) {
                active.state = snap.state.clone();
                match snap.state {
                    TaskState::Pending | TaskState::Running => {
                        self.active_task = Some(active);
                    }
                    TaskState::Succeeded { exit_code } => {
                        output_lines
                            .push(format!("[{}] Succeeded (exit {exit_code})", active.name));
                        self.tasks.unregister(active.task_id);
                        finished_toast = Some((
                            format!("{} succeeded (exit {exit_code})", active.name),
                            false,
                        ));
                    }
                    TaskState::Failed { exit_code } => {
                        let code = exit_code.map_or_else(|| "unknown".into(), |c| c.to_string());
                        output_lines.push(format!("[{}] Failed (exit {code})", active.name));
                        self.tasks.unregister(active.task_id);
                        finished_toast =
                            Some((format!("{} failed (exit {code})", active.name), true));
                    }
                    TaskState::Cancelled => {
                        output_lines.push(format!("[{}] Cancelled", active.name));
                        self.tasks.unregister(active.task_id);
                        finished_toast = Some((format!("{} cancelled", active.name), false));
                    }
                    TaskState::TimedOut => {
                        output_lines.push(format!("[{}] Timed out", active.name));
                        self.tasks.unregister(active.task_id);
                        finished_toast = Some((format!("{} timed out", active.name), true));
                    }
                    TaskState::SpawnFailed { message } => {
                        output_lines.push(format!("[{}] Spawn failed: {message}", active.name));
                        self.tasks.unregister(active.task_id);
                        finished_toast =
                            Some((format!("{} failed to start: {message}", active.name), true));
                    }
                }
            }
        }
        for event in self.terminals.drain_events(true) {
            match event {
                TerminalEvent::Output { bytes, .. } => {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        self.terminal_lines.push(line.to_owned());
                    }
                }
                TerminalEvent::StateChanged {
                    id,
                    state,
                    exit_code,
                    error,
                    ..
                } => {
                    if state.is_finished() {
                        if let Some(err) = error {
                            self.terminal_lines.push(format!("[terminal error] {err}"));
                        } else {
                            self.terminal_lines
                                .push(format!("[process exited with {exit_code:?}]"));
                        }
                        if self.active_terminal == Some(id) {
                            self.active_terminal = None;
                        }
                    }
                }
                TerminalEvent::Closed { id } => {
                    if self.active_terminal == Some(id) {
                        self.active_terminal = None;
                    }
                }
            }
        }
        let _ = self.tasks.drain_events();
        finished_toast
    }
}

fn read_selections(path: &Path) -> BTreeMap<String, RunIdentity> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_selections(path: &Path, selections: &BTreeMap<String, RunIdentity>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(selections).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_parses_simple_and_advanced_args_and_slugifies_id() {
        let mut draft = RunDraft::new_custom(OutputDestination::Run, SavePolicy::Ask);
        draft.name = "Run Lua Script".to_owned();
        draft.executable = "lua5.4".to_owned();
        draft.args_simple = r#""${file}" --flag "hello world""#.to_owned();
        let config = draft.to_configuration().unwrap();
        assert_eq!(config.id, "run-lua-script");
        assert_eq!(config.args, vec!["${file}", "--flag", "hello world"]);
    }

    #[test]
    fn coordinator_persists_selection_and_requires_approval() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("koren-native-run-{nonce}"));
        let ws = root.join("ws");
        let cfg = root.join("cfg");
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&cfg).unwrap();

        let mut coord = RunCoordinator::new(cfg.clone()).unwrap();
        coord.refresh(Some(&ws)).unwrap();
        let id = coord
            .save_configuration(
                "workspace",
                RunConfiguration {
                    id: "echo-test".into(),
                    name: "Echo Test".into(),
                    executable: Some("echo".into()),
                    tool: None,
                    args: vec!["hello".into()],
                    cwd: "${workspace}".into(),
                    environment: BTreeMap::new(),
                    languages: Vec::new(),
                    output: OutputDestination::Run,
                    save: SavePolicy::Ask,
                    timeout_ms: None,
                    debug_adapter: None,
                },
                Some(&ws),
            )
            .unwrap();
        assert_eq!(coord.selected.as_ref(), Some(&id));

        let mut restarted = RunCoordinator::new(cfg).unwrap();
        restarted.refresh(Some(&ws)).unwrap();
        assert_eq!(restarted.selected.as_ref(), Some(&id));

        let approval = restarted.prepare_approval(&id, &ws, None, false).unwrap();
        assert_eq!(approval.resolved.executable, "echo");
        assert!(restarted.pending_approval.is_some());
        restarted.deny_pending();
        assert!(restarted.pending_approval.is_none());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recommends_matching_extension_only_when_available_and_not_active() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let cfg = std::env::temp_dir().join(format!("koren-native-rec-{nonce}"));
        let lua_ext = cfg.join("extensions").join("org.koren.lua");
        fs::create_dir_all(&lua_ext).unwrap();
        fs::write(
            lua_ext.join("koren-extension.toml"),
            r#"schema_version = 1
id = "org.koren.lua"
name = "Lua, LuaJIT & Luau"
publisher = "Koren"
version = "0.1.0"
description = "Lua test fixture"
entry_point = "main.lua"
api_version = "1.0.0"
capabilities = ["language_definitions", "tools"]

[[languages]]
id = "lua"
name = "Lua"
extensions = ["lua"]

[[tools]]
id = "lua"
name = "Lua Interpreter"
description = "Standard Lua runtime"
category = "runtime"
optional = true
executables = ["lua"]
"#,
        )
        .unwrap();
        fs::write(
            lua_ext.join("main.lua"),
            "local ext = {}\nfunction ext.activate() end\nfunction ext.deactivate() end\nreturn ext\n",
        )
        .unwrap();

        let mut coord = RunCoordinator::new(cfg.clone()).unwrap();
        let rec_lua = coord.recommend_extension_for_file(Path::new("hello.lua"));
        assert_eq!(
            rec_lua,
            Some((
                "org.koren.lua".into(),
                "Lua, LuaJIT & Luau".into(),
                "lua".into()
            ))
        );
        assert!(
            coord
                .recommend_extension_for_file(Path::new("unknown.xyz"))
                .is_none()
        );

        let _ = coord
            .enable_and_activate_extension("org.koren.lua", None)
            .unwrap();
        assert!(
            coord
                .recommend_extension_for_file(Path::new("hello.lua"))
                .is_none()
        );
        assert!(!coord.tools.declarations().is_empty());

        let _ = fs::remove_dir_all(cfg);
    }
}
