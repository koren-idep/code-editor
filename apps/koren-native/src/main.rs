//! Native Rust desktop IDE application for Koren IDE Platform.
//!
//! Built on `koren-ui` (`wgpu` + `winit` + `taffy` + `cosmic-text`) and directly
//! integrated with Koren's workspace, document, search, git, and run services.

mod run_ui;

use koren_documents::{DocumentId, DocumentManager};
use koren_run::{OutputDestination, RunContext, SavePolicy, resolve as resolve_run};
use koren_ui::app::{NativeApp, WindowCommand, run_app};
use koren_ui::draw::{FontWeight, IconKind, Point, QuadCommand, Rect, Size, TextCommand};
use koren_ui::editor::{CodeEditorState, LanguageMode};
use koren_ui::input::{FocusId, InputState};
use koren_ui::layout::UiContext;
use koren_ui::theme::{Color, ThemeKind, ThemeTokens};
use koren_ui::widgets::{
    Button, ButtonVariant, Checkbox, MenuItem, TabItem, TextInput, Toggle, TreeRowItem,
    paint_dialog_frame, paint_menu, paint_tab_bar, paint_toast, paint_tree_view,
};
use koren_ui::winit::event::MouseButton;
use koren_ui::winit::keyboard::{Key, ModifiersState, NamedKey};
use koren_workspace::{EntryKind, WorkspaceEntry, WorkspaceManager};
use run_ui::{RunCoordinator, RunDraft, RunIdentity, executable_exists};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SidebarTab {
    Files,
    Search,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BottomTab {
    Output,
    Problems,
    Terminal,
}

use koren_tools::ToolCategory;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsTab {
    General,
    RunDebug,
    ToolsRuntimes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActiveModal {
    None,
    Settings,
    CommandPalette,
    RunApproval,
    RunSelector,
    CreateRunConfig,
    RunManager,
    DirtyRunPrompt,
    ToolManager,
    ToolSetupPrompt,
    MainMenu,
}

struct OpenTab {
    doc_id: DocumentId,
    rel_path: PathBuf,
    title: String,
    dirty: bool,
    editor: CodeEditorState,
}

struct TreeNode {
    rel_path: PathBuf,
    label: String,
    depth: usize,
    is_dir: bool,
}

struct ToastMessage {
    title: String,
    body: String,
    is_error: bool,
}

struct KorenNativeApp {
    start_instant: Instant,
    smoke_mode: bool,
    theme: ThemeTokens,
    workspace: WorkspaceManager,
    documents: DocumentManager,
    workspace_name: String,
    workspace_root: Option<PathBuf>,
    git_branch: Option<String>,
    expanded_dirs: BTreeSet<PathBuf>,
    dir_cache: HashMap<PathBuf, Vec<WorkspaceEntry>>,
    tree_nodes: Vec<TreeNode>,
    selected_tree_path: Option<PathBuf>,
    tree_scroll_y: f32,
    sidebar_visible: bool,
    sidebar_width: f32,
    sidebar_tab: SidebarTab,
    bottom_visible: bool,
    bottom_height: f32,
    bottom_tab: BottomTab,
    tabs: Vec<OpenTab>,
    active_tab: usize,
    active_modal: ActiveModal,
    toast: Option<ToastMessage>,
    search_query: String,
    search_results: Vec<PathBuf>,
    palette_query: String,
    output_lines: Vec<String>,
    setting_line_numbers: bool,
    setting_indent_guides: bool,
    settings_tab: SettingsTab,
    run_coord: RunCoordinator,
    run_draft: Option<RunDraft>,
    pending_dirty_identity: Option<RunIdentity>,
    tool_category_filter: Option<ToolCategory>,
    selected_tool_id: Option<String>,
    tool_manual_path: String,
    pending_tool_setup: Option<String>,
    recommended_extension: Option<(String, String, String)>,
    dismissed_ext_recommendations: BTreeSet<String>,
    last_sidebar_rect: Rect,
    last_editor_rect: Rect,
    last_tree_rect: Rect,
}

fn default_config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("koren");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/koren");
    }
    std::env::temp_dir().join("koren-config")
}

impl KorenNativeApp {
    fn new(workspace_arg: Option<PathBuf>, smoke_mode: bool) -> Self {
        let run_coord =
            RunCoordinator::new(default_config_dir()).expect("run coordinator initialization");
        let mut app = Self {
            start_instant: Instant::now(),
            smoke_mode,
            theme: ThemeTokens::dark(),
            workspace: WorkspaceManager::new(),
            documents: DocumentManager::new(),
            workspace_name: "koren".to_owned(),
            workspace_root: None,
            git_branch: None,
            expanded_dirs: BTreeSet::new(),
            dir_cache: HashMap::new(),
            tree_nodes: Vec::new(),
            selected_tree_path: None,
            tree_scroll_y: 0.0,
            sidebar_visible: true,
            sidebar_width: 320.0,
            sidebar_tab: SidebarTab::Files,
            bottom_visible: false,
            bottom_height: 190.0,
            bottom_tab: BottomTab::Output,
            tabs: Vec::new(),
            active_tab: 0,
            active_modal: ActiveModal::None,
            toast: None,
            search_query: String::new(),
            search_results: Vec::new(),
            palette_query: String::new(),
            output_lines: vec![
                "Koren Native UI initialized (wgpu + cosmic-text + taffy).".to_owned(),
                "Press Ctrl+K for Command Palette, Ctrl+, for Settings, Play to run.".to_owned(),
            ],
            setting_line_numbers: true,
            setting_indent_guides: true,
            settings_tab: SettingsTab::General,
            run_coord,
            run_draft: None,
            pending_dirty_identity: None,
            tool_category_filter: None,
            selected_tool_id: None,
            tool_manual_path: String::new(),
            pending_tool_setup: None,
            recommended_extension: None,
            dismissed_ext_recommendations: BTreeSet::new(),
            last_sidebar_rect: Rect::default(),
            last_editor_rect: Rect::default(),
            last_tree_rect: Rect::default(),
        };

        let target = workspace_arg
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        app.open_workspace(&target);
        app
    }

    fn open_workspace(&mut self, path: &Path) {
        if let Err(err) = self.workspace.open(path) {
            self.toast = Some(ToastMessage {
                title: "Workspace Error".to_owned(),
                body: err.to_string(),
                is_error: true,
            });
            return;
        }
        if let Ok(meta) = self.workspace.metadata() {
            self.workspace_name = meta.name.to_string_lossy().into_owned();
            self.workspace_root = Some(meta.root.clone());
            self.detect_git_branch(&meta.root);
        }
        self.expanded_dirs.insert(PathBuf::from("src"));
        self.rebuild_tree();
        if let Err(err) = self.run_coord.refresh(self.workspace_root.as_deref()) {
            self.toast = Some(ToastMessage {
                title: "Run Config Load Warning".to_owned(),
                body: err,
                is_error: true,
            });
        }

        // Auto-open a sensible entry file if present
        for candidate in ["src/main.rs", "Cargo.toml", "README.md"] {
            let rel = PathBuf::from(candidate);
            if self.open_file(&rel) {
                self.selected_tree_path = Some(rel);
                break;
            }
        }
    }

    fn detect_git_branch(&mut self, root: &Path) {
        let head = root.join(".git/HEAD");
        if let Ok(contents) = std::fs::read_to_string(head) {
            let trimmed = contents.trim();
            if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
                self.git_branch = Some(branch.to_owned());
            } else if !trimmed.is_empty() {
                self.git_branch = Some(trimmed.chars().take(7).collect());
            }
        }
    }

    fn rebuild_tree(&mut self) {
        self.tree_nodes.clear();
        self.dir_cache.clear();
        self.populate_dir(Path::new(""), 0);
    }

    fn populate_dir(&mut self, rel: &Path, depth: usize) {
        let entries = match self.workspace.list_dir(rel) {
            Ok(mut list) => {
                list.sort_by(|a, b| {
                    let a_dir = a.kind == EntryKind::Directory;
                    let b_dir = b.kind == EntryKind::Directory;
                    b_dir
                        .cmp(&a_dir)
                        .then_with(|| a.name.to_string_lossy().cmp(&b.name.to_string_lossy()))
                });
                list
            }
            Err(_) => return,
        };
        for entry in &entries {
            let name = entry.name.to_string_lossy().into_owned();
            if name == ".git" {
                continue;
            }
            let is_dir = entry.kind == EntryKind::Directory;
            let child_rel = entry.path.clone();
            self.tree_nodes.push(TreeNode {
                rel_path: child_rel.clone(),
                label: name,
                depth,
                is_dir,
            });
            if is_dir && self.expanded_dirs.contains(&child_rel) {
                self.populate_dir(&child_rel, depth + 1);
            }
        }
        self.dir_cache.insert(rel.to_path_buf(), entries);
    }

    fn refresh_extension_recommendation(&mut self) {
        self.recommended_extension = self
            .tabs
            .get(self.active_tab)
            .and_then(|tab| self.run_coord.recommend_extension_for_file(&tab.rel_path))
            .filter(|(id, _, _)| !self.dismissed_ext_recommendations.contains(id));
    }

    fn open_file(&mut self, rel_path: &Path) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.rel_path == rel_path) {
            self.active_tab = idx;
            self.refresh_extension_recommendation();
            return true;
        }
        let Ok(doc_id) = self.documents.open_file(&self.workspace, rel_path) else {
            return false;
        };
        let Ok(snap) = self.documents.snapshot(doc_id) else {
            return false;
        };
        let title = rel_path.file_name().map_or_else(
            || "untitled".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let lang = LanguageMode::from_path(&title);
        let editor_id = FocusId::from_str(&format!("editor:{}", rel_path.display()));
        let editor = CodeEditorState::new(editor_id, &snap.text, lang);
        self.tabs.push(OpenTab {
            doc_id,
            rel_path: rel_path.to_path_buf(),
            title,
            dirty: false,
            editor,
        });
        self.active_tab = self.tabs.len() - 1;
        self.selected_tree_path = Some(rel_path.to_path_buf());
        self.refresh_extension_recommendation();
        true
    }

    fn save_active_tab(&mut self) {
        let Some(tab) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        if let Some(root) = &self.workspace_root {
            let abs = root.join(&tab.rel_path);
            match std::fs::write(&abs, tab.editor.text()) {
                Ok(()) => {
                    tab.dirty = false;
                    self.toast = Some(ToastMessage {
                        title: "Saved".to_owned(),
                        body: format!("Wrote {}", tab.rel_path.display()),
                        is_error: false,
                    });
                }
                Err(err) => {
                    self.toast = Some(ToastMessage {
                        title: "Save Failed".to_owned(),
                        body: err.to_string(),
                        is_error: true,
                    });
                }
            }
        }
    }

    fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        let doc_id = self.tabs[idx].doc_id;
        let _ = self.documents.close(doc_id, true);
        self.tabs.remove(idx);
        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    fn run_search(&mut self) {
        self.search_results.clear();
        let q = self.search_query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return;
        }
        for node in &self.tree_nodes {
            if !node.is_dir && node.label.to_ascii_lowercase().contains(&q) {
                self.search_results.push(node.rel_path.clone());
            }
        }
    }

    fn active_file_abs(&self) -> Option<PathBuf> {
        let root = self.workspace_root.as_ref()?;
        let tab = self.tabs.get(self.active_tab)?;
        Some(root.join(&tab.rel_path))
    }

    fn trigger_run_for_identity(&mut self, identity: RunIdentity, dirty_action: Option<&str>) {
        let Some(root) = self.workspace_root.clone() else {
            self.toast = Some(ToastMessage {
                title: "No Workspace".to_owned(),
                body: "Open a workspace before running configurations.".to_owned(),
                is_error: true,
            });
            return;
        };
        let Some(entry) = self
            .run_coord
            .entries
            .iter()
            .find(|e| e.identity == identity)
            .cloned()
        else {
            self.toast = Some(ToastMessage {
                title: "Configuration Missing".to_owned(),
                body: "Selected run configuration no longer exists.".to_owned(),
                is_error: true,
            });
            return;
        };
        let mut disk_warning = false;
        let active_dirty = self.tabs.get(self.active_tab).is_some_and(|t| t.dirty);
        if active_dirty {
            match (entry.configuration.save, dirty_action) {
                (SavePolicy::Ask, None) => {
                    self.pending_dirty_identity = Some(identity);
                    self.active_modal = ActiveModal::DirtyRunPrompt;
                    return;
                }
                (_, Some("save")) | (SavePolicy::Save, None) => {
                    self.save_active_tab();
                    if self.tabs.get(self.active_tab).is_some_and(|t| t.dirty) {
                        return;
                    }
                }
                (_, Some("disk")) | (SavePolicy::Disk, None) => {
                    disk_warning = true;
                }
                _ => {}
            }
        }
        let active_file = self.active_file_abs();
        match self.run_coord.prepare_approval(
            &identity,
            &root,
            active_file.as_deref(),
            disk_warning,
        ) {
            Ok(_) => {
                self.active_modal = ActiveModal::RunApproval;
            }
            Err(err) => {
                if let Some(rest) = err.strip_prefix("tool:")
                    && let Some((tool_id, _)) = rest.split_once(':')
                    && !self.run_coord.tools.is_prompt_dismissed(tool_id)
                {
                    self.pending_tool_setup = Some(tool_id.to_owned());
                    self.active_modal = ActiveModal::ToolSetupPrompt;
                    return;
                }
                self.toast = Some(ToastMessage {
                    title: "Cannot Prepare Run".to_owned(),
                    body: err,
                    is_error: true,
                });
            }
        }
    }

    fn execute_approved_run(&mut self) {
        match self.run_coord.approve_pending() {
            Ok(OutputDestination::Run) => {
                if self.run_coord.settings.reveal_on_run {
                    self.bottom_visible = true;
                    self.bottom_tab = BottomTab::Output;
                }
                if let Some(active) = &self.run_coord.active_task {
                    self.output_lines
                        .push(format!("> Running: {}", active.name));
                }
            }
            Ok(OutputDestination::Terminal) => {
                if self.run_coord.settings.reveal_on_run {
                    self.bottom_visible = true;
                    self.bottom_tab = BottomTab::Terminal;
                }
            }
            Err(err) => {
                self.toast = Some(ToastMessage {
                    title: "Run Failed".to_owned(),
                    body: err,
                    is_error: true,
                });
            }
        }
    }

    fn paint_top_toolbar(&self, ctx: &mut UiContext<'_>, viewport: Size) {
        let h = self.theme.metrics.toolbar_height;
        let bar = Rect::new(0.0, 0.0, viewport.width, h);
        ctx.draw.push_quad(QuadCommand {
            rect: bar,
            color: self.theme.canvas_bg,
            ..Default::default()
        });

        // Left cluster
        let y = (h - 26.0) * 0.5;
        Button::icon(FocusId::from_str("tb.logo"), IconKind::KorenMark, "Koren")
            .icon_color(self.theme.accent)
            .paint_at(ctx, Rect::new(8.0, y, 26.0, 26.0));
        Button::icon(FocusId::from_str("tb.menu"), IconKind::Menu, "Main Menu")
            .paint_at(ctx, Rect::new(38.0, y, 26.0, 26.0));
        Button::icon(
            FocusId::from_str("tb.left"),
            IconKind::PanelLeft,
            "Toggle Left Sidebar",
        )
        .icon_color(if self.sidebar_visible {
            self.theme.text_primary
        } else {
            self.theme.text_secondary
        })
        .paint_at(ctx, Rect::new(68.0, y, 26.0, 26.0));
        Button::icon(
            FocusId::from_str("tb.bottom"),
            IconKind::PanelBottom,
            "Toggle Bottom Panel",
        )
        .icon_color(if self.bottom_visible {
            self.theme.text_primary
        } else {
            self.theme.text_secondary
        })
        .paint_at(ctx, Rect::new(98.0, y, 26.0, 26.0));
        Button::icon(
            FocusId::from_str("tb.right"),
            IconKind::PanelRight,
            "Toggle Theme",
        )
        .paint_at(ctx, Rect::new(128.0, y, 26.0, 26.0));

        // Subtle separator
        ctx.draw.push_quad(QuadCommand {
            rect: Rect::new(160.0, 10.0, 1.0, 16.0),
            color: self.theme.separator,
            ..Default::default()
        });

        // Centered workspace title
        let title_w = ctx.text.measure_width(
            &self.workspace_name,
            self.theme.metrics.font_caption,
            FontWeight::Medium,
            false,
        );
        ctx.draw.push_text(TextCommand {
            pos: Point::new((viewport.width - title_w) * 0.5, 10.0),
            max_width: Some(300.0),
            text: self.workspace_name.clone(),
            size: self.theme.metrics.font_caption,
            line_height: 16.0,
            color: self.theme.text_secondary,
            weight: FontWeight::Medium,
            monospace: false,
            italic: false,
            clip: Some(bar),
        });

        // Right cluster (run controls + utility actions + window controls matching old UI)
        let rx = viewport.width;
        let close_id = FocusId::from_str("win.close");
        let close_hovered = ctx.input.hovered == Some(close_id);
        if close_hovered {
            ctx.draw.push_quad(QuadCommand {
                rect: Rect::new(rx - 38.0, 0.0, 38.0, h),
                color: Color::from_hex(0xc42b1c),
                ..Default::default()
            });
        }
        Button::icon(close_id, IconKind::Close, "Close")
            .icon_color(if close_hovered {
                Color::WHITE
            } else {
                self.theme.text_secondary
            })
            .paint_at(ctx, Rect::new(rx - 32.0, y, 26.0, 26.0));
        Button::icon(FocusId::from_str("win.max"), IconKind::Maximize, "Maximize")
            .paint_at(ctx, Rect::new(rx - 64.0, y, 26.0, 26.0));
        Button::icon(FocusId::from_str("win.min"), IconKind::Minimize, "Minimize")
            .paint_at(ctx, Rect::new(rx - 96.0, y, 26.0, 26.0));

        ctx.draw.push_quad(QuadCommand {
            rect: Rect::new(rx - 104.0, 10.0, 1.0, 16.0),
            color: self.theme.separator,
            ..Default::default()
        });

        Button::icon(
            FocusId::from_str("tb.settings"),
            IconKind::Settings,
            "Settings",
        )
        .paint_at(ctx, Rect::new(rx - 134.0, y, 26.0, 26.0));
        Button::icon(
            FocusId::from_str("tb.right"),
            IconKind::PanelRight,
            "Toggle Theme",
        )
        .paint_at(ctx, Rect::new(rx - 164.0, y, 26.0, 26.0));
        Button::icon(
            FocusId::from_str("tb.search"),
            IconKind::Search,
            "Command Palette",
        )
        .paint_at(ctx, Rect::new(rx - 194.0, y, 26.0, 26.0));

        // [Configuration ▾] [▶ Run / ■ Stop] [↻ Rerun] [Debug]
        Button::icon(
            FocusId::from_str("tb.debug"),
            IconKind::Bolt,
            "Debug Configuration",
        )
        .icon_color(self.theme.accent)
        .paint_at(ctx, Rect::new(rx - 224.0, y, 26.0, 26.0));

        if self.run_coord.is_running() {
            Button::new(FocusId::from_str("tb.stop"), "■ Stop")
                .variant(ButtonVariant::Secondary)
                .paint_at(ctx, Rect::new(rx - 286.0, y, 58.0, 26.0));
        } else {
            Button::icon(FocusId::from_str("tb.run"), IconKind::Play, "Run Selected")
                .icon_color(self.theme.success)
                .paint_at(ctx, Rect::new(rx - 254.0, y, 26.0, 26.0));
            if self.run_coord.last_run.is_some() {
                Button::new(FocusId::from_str("tb.rerun"), "↻")
                    .variant(ButtonVariant::Ghost)
                    .paint_at(ctx, Rect::new(rx - 282.0, y, 24.0, 26.0));
            }
        }

        let sel_label = match self.run_coord.selected_entry() {
            Some(entry) => {
                let missing = entry
                    .configuration
                    .executable
                    .as_deref()
                    .is_some_and(|e| !e.contains("${") && !executable_exists(e));
                if missing {
                    format!("⚠ {} ▾", entry.configuration.name)
                } else {
                    format!("{} ▾", entry.configuration.name)
                }
            }
            None => "Create Configuration…".to_owned(),
        };
        let sel_w = 178.0;
        let sel_x = if self.run_coord.is_running() || self.run_coord.last_run.is_some() {
            rx - 290.0 - sel_w
        } else {
            rx - 260.0 - sel_w
        };
        Button::new(FocusId::from_str("tb.run_select"), sel_label)
            .variant(ButtonVariant::Secondary)
            .paint_at(ctx, Rect::new(sel_x, y, sel_w, 26.0));
    }

    fn paint_sidebar_island(&mut self, ctx: &mut UiContext<'_>, rect: Rect) {
        self.last_sidebar_rect = rect;
        ctx.draw.push_quad(QuadCommand {
            rect,
            color: self.theme.island_bg,
            border_color: self.theme.island_border,
            border_width: 1.0,
            corner_radius: self.theme.metrics.radius_island,
            ..Default::default()
        });

        // Header tabs: Files | Search
        let files_rect = Rect::new(rect.x + 10.0, rect.y + 8.0, 56.0, 24.0);
        let search_rect = Rect::new(rect.x + 70.0, rect.y + 8.0, 68.0, 24.0);

        Button::new(FocusId::from_str("sb.files"), "Files")
            .variant(if self.sidebar_tab == SidebarTab::Files {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            })
            .paint_at(ctx, files_rect);
        Button::new(FocusId::from_str("sb.search"), "Search")
            .variant(if self.sidebar_tab == SidebarTab::Search {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            })
            .paint_at(ctx, search_rect);

        match self.sidebar_tab {
            SidebarTab::Files => {
                // Workspace root title
                ctx.draw.push_text(TextCommand {
                    pos: Point::new(rect.x + 14.0, rect.y + 42.0),
                    max_width: Some((rect.width - 28.0).max(0.0)),
                    text: self.workspace_name.clone(),
                    size: self.theme.metrics.font_ui,
                    line_height: 18.0,
                    color: self.theme.text_primary,
                    weight: FontWeight::SemiBold,
                    monospace: false,
                    italic: false,
                    clip: Some(rect),
                });

                let tree_rect = Rect::new(
                    rect.x,
                    rect.y + 66.0,
                    rect.width,
                    (rect.height - 72.0).max(0.0),
                );
                self.last_tree_rect = tree_rect;

                let rows: Vec<TreeRowItem> = self
                    .tree_nodes
                    .iter()
                    .map(|n| {
                        let (badge, badge_color) = LanguageMode::from_path(&n.label).badge();
                        TreeRowItem {
                            id: FocusId::from_str(&format!("tree:{}", n.rel_path.display())),
                            label: n.label.clone(),
                            depth: n.depth,
                            is_dir: n.is_dir,
                            expanded: self.expanded_dirs.contains(&n.rel_path),
                            selected: self.selected_tree_path.as_ref() == Some(&n.rel_path),
                            badge: badge.to_owned(),
                            badge_color,
                        }
                    })
                    .collect();
                paint_tree_view(ctx, tree_rect, &rows, self.tree_scroll_y);
            }
            SidebarTab::Search => {
                let input_rect = Rect::new(rect.x + 10.0, rect.y + 42.0, rect.width - 20.0, 28.0);
                TextInput {
                    id: FocusId::from_str("sb.search_input"),
                    value: &self.search_query,
                    placeholder: "Filter workspace files...",
                    layer: 0,
                }
                .paint_at(ctx, input_rect);

                let list_rect = Rect::new(
                    rect.x,
                    rect.y + 78.0,
                    rect.width,
                    (rect.height - 84.0).max(0.0),
                );
                let rows: Vec<TreeRowItem> = self
                    .search_results
                    .iter()
                    .map(|p| {
                        let label = p.to_string_lossy().into_owned();
                        let (badge, badge_color) = LanguageMode::from_path(&label).badge();
                        TreeRowItem {
                            id: FocusId::from_str(&format!("search:{}", p.display())),
                            label,
                            depth: 0,
                            is_dir: false,
                            expanded: false,
                            selected: self.selected_tree_path.as_ref() == Some(p),
                            badge: badge.to_owned(),
                            badge_color,
                        }
                    })
                    .collect();
                paint_tree_view(ctx, list_rect, &rows, 0.0);
            }
        }
    }

    fn paint_editor_island(&mut self, ctx: &mut UiContext<'_>, rect: Rect) {
        self.last_editor_rect = rect;
        ctx.draw.push_quad(QuadCommand {
            rect,
            color: self.theme.island_bg,
            border_color: self.theme.island_border,
            border_width: 1.0,
            corner_radius: self.theme.metrics.radius_island,
            ..Default::default()
        });

        let tab_h = self.theme.metrics.tabbar_height;
        let tab_rect = Rect::new(rect.x, rect.y, rect.width, tab_h);
        let tab_items: Vec<TabItem> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let (badge, badge_color) = t.editor.language.badge();
                TabItem {
                    id: FocusId::from_str(&format!("tab:{i}")),
                    close_id: FocusId::from_str(&format!("tab_close:{i}")),
                    title: t.title.clone(),
                    badge: badge.to_owned(),
                    badge_color,
                    dirty: t.dirty,
                    active: i == self.active_tab,
                }
            })
            .collect();
        paint_tab_bar(ctx, tab_rect, &tab_items);

        let mut body_y = rect.y + tab_h;
        let mut body_h = (rect.height - tab_h).max(0.0);
        if let Some((_, ref ext_name, ref file_ext)) = self.recommended_extension {
            let banner_h = 32.0;
            let banner = Rect::new(rect.x + 1.0, body_y, (rect.width - 2.0).max(0.0), banner_h);
            ctx.draw.push_quad(QuadCommand {
                rect: banner,
                color: self.theme.floating_bg,
                border_color: self.theme.separator,
                border_width: 1.0,
                ..Default::default()
            });
            ctx.draw.push_text(TextCommand {
                pos: Point::new(banner.x + 12.0, banner.y + 8.0),
                max_width: Some((banner.width - 256.0).max(80.0)),
                text: format!("Enable '{ext_name}' extension to support .{file_ext} files?"),
                size: 12.5,
                line_height: 16.0,
                color: self.theme.text_primary,
                weight: FontWeight::Medium,
                monospace: false,
                italic: false,
                clip: Some(banner),
            });
            Button::new(FocusId::from_str("ext_rec.enable"), "Enable & Activate")
                .variant(ButtonVariant::Primary)
                .paint_at(
                    ctx,
                    Rect::new(banner.x + banner.width - 236.0, banner.y + 4.0, 152.0, 24.0),
                );
            Button::new(FocusId::from_str("ext_rec.dismiss"), "Dismiss")
                .variant(ButtonVariant::Ghost)
                .paint_at(
                    ctx,
                    Rect::new(banner.x + banner.width - 78.0, banner.y + 4.0, 70.0, 24.0),
                );
            body_y += banner_h;
            body_h = (body_h - banner_h).max(0.0);
        }
        let body_rect = Rect::new(rect.x, body_y, rect.width, body_h);
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.editor.paint(ctx, body_rect);
        } else {
            let msg = "Open a file from the explorer or press Ctrl+K";
            let w = ctx
                .text
                .measure_width(msg, 13.0, FontWeight::Regular, false);
            ctx.draw.push_text(TextCommand {
                pos: Point::new(
                    body_rect.x + (body_rect.width - w) * 0.5,
                    body_rect.y + body_rect.height * 0.45,
                ),
                max_width: Some(body_rect.width),
                text: msg.to_owned(),
                size: 13.0,
                line_height: 18.0,
                color: self.theme.text_secondary,
                weight: FontWeight::Regular,
                monospace: false,
                italic: false,
                clip: Some(body_rect),
            });
        }
    }

    fn paint_bottom_island(&self, ctx: &mut UiContext<'_>, rect: Rect) {
        ctx.draw.push_quad(QuadCommand {
            rect,
            color: self.theme.island_bg,
            border_color: self.theme.island_border,
            border_width: 1.0,
            corner_radius: self.theme.metrics.radius_island,
            ..Default::default()
        });
        Button::new(FocusId::from_str("bp.output"), "Output")
            .variant(if self.bottom_tab == BottomTab::Output {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            })
            .paint_at(ctx, Rect::new(rect.x + 10.0, rect.y + 6.0, 62.0, 24.0));
        Button::new(FocusId::from_str("bp.problems"), "Problems")
            .variant(if self.bottom_tab == BottomTab::Problems {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            })
            .paint_at(ctx, Rect::new(rect.x + 76.0, rect.y + 6.0, 74.0, 24.0));
        Button::new(FocusId::from_str("bp.terminal"), "Terminal")
            .variant(if self.bottom_tab == BottomTab::Terminal {
                ButtonVariant::Secondary
            } else {
                ButtonVariant::Ghost
            })
            .paint_at(ctx, Rect::new(rect.x + 154.0, rect.y + 6.0, 70.0, 24.0));

        if self.run_coord.is_running() {
            Button::new(FocusId::from_str("bp.stop"), "■ Stop")
                .variant(ButtonVariant::Secondary)
                .paint_at(
                    ctx,
                    Rect::new(rect.x + rect.width - 74.0, rect.y + 6.0, 64.0, 24.0),
                );
        } else if self.run_coord.last_run.is_some() {
            Button::new(FocusId::from_str("bp.rerun"), "↻ Rerun")
                .variant(ButtonVariant::Ghost)
                .paint_at(
                    ctx,
                    Rect::new(rect.x + rect.width - 78.0, rect.y + 6.0, 68.0, 24.0),
                );
        }

        ctx.draw.push_quad(QuadCommand {
            rect: Rect::new(rect.x, rect.y + 34.0, rect.width, 1.0),
            color: self.theme.separator,
            ..Default::default()
        });

        let content_rect = Rect::new(
            rect.x + 12.0,
            rect.y + 40.0,
            rect.width - 24.0,
            rect.height - 46.0,
        );
        let lines: Vec<&str> = match self.bottom_tab {
            BottomTab::Output => self.output_lines.iter().map(String::as_str).collect(),
            BottomTab::Problems => vec!["No diagnostics reported by local workspace."],
            BottomTab::Terminal => {
                if self.run_coord.terminal_lines.is_empty() {
                    vec![
                        "koren-terminal PTY session ready. Run an interactive configuration to start.",
                    ]
                } else {
                    self.run_coord
                        .terminal_lines
                        .iter()
                        .map(String::as_str)
                        .collect()
                }
            }
        };
        for (i, line) in lines.iter().rev().take(7).rev().enumerate() {
            ctx.draw.push_text(TextCommand {
                pos: Point::new(content_rect.x, content_rect.y + i as f32 * 18.0),
                max_width: Some(content_rect.width),
                text: (*line).to_owned(),
                size: 12.0,
                line_height: 17.0,
                color: self.theme.text_secondary,
                weight: FontWeight::Regular,
                monospace: true,
                italic: false,
                clip: Some(content_rect),
            });
        }
    }

    fn paint_status_bar(&self, ctx: &mut UiContext<'_>, viewport: Size) {
        let h = self.theme.metrics.statusbar_height;
        let y = viewport.height - h;
        let bar = Rect::new(0.0, y, viewport.width, h);
        ctx.draw.push_quad(QuadCommand {
            rect: bar,
            color: self.theme.canvas_bg,
            ..Default::default()
        });

        let breadcrumb = self
            .tabs
            .get(self.active_tab)
            .map(|t| t.rel_path.to_string_lossy().replace('/', " / "))
            .unwrap_or_else(|| self.workspace_name.clone());
        let left_text = if let Some(ref branch) = self.git_branch {
            format!("{breadcrumb}   •   git:{branch}")
        } else {
            breadcrumb
        };

        ctx.draw.push_text(TextCommand {
            pos: Point::new(12.0, y + 6.0),
            max_width: Some(viewport.width * 0.6),
            text: left_text,
            size: self.theme.metrics.font_caption,
            line_height: 16.0,
            color: self.theme.text_secondary,
            weight: FontWeight::Regular,
            monospace: false,
            italic: false,
            clip: Some(bar),
        });

        let (line, col) = self
            .tabs
            .get(self.active_tab)
            .map_or((1, 1), |t| t.editor.cursor_display_pos());
        let lang = self
            .tabs
            .get(self.active_tab)
            .map_or("Plain Text", |t| t.editor.language.label());
        let theme_label = match self.theme.kind {
            ThemeKind::Dark => "Dark",
            ThemeKind::OledBlack => "OLED",
        };
        let right_text = format!("{line}:{col}   UTF-8   {lang}   {theme_label}");
        let rw = ctx.text.measure_width(
            &right_text,
            self.theme.metrics.font_caption,
            FontWeight::Regular,
            false,
        );
        ctx.draw.push_text(TextCommand {
            pos: Point::new(viewport.width - rw - 14.0, y + 6.0),
            max_width: Some(400.0),
            text: right_text,
            size: self.theme.metrics.font_caption,
            line_height: 16.0,
            color: self.theme.text_secondary,
            weight: FontWeight::Regular,
            monospace: false,
            italic: false,
            clip: Some(bar),
        });
    }

    fn paint_overlays(&self, ctx: &mut UiContext<'_>, viewport: Size) {
        if let Some(ref toast) = self.toast {
            paint_toast(
                ctx,
                Point::new(
                    viewport.width - 14.0,
                    self.theme.metrics.toolbar_height + 8.0,
                ),
                &toast.title,
                &toast.body,
                toast.is_error,
                FocusId::from_str("toast.dismiss"),
            );
        }

        let vp_rect = Rect::new(0.0, 0.0, viewport.width, viewport.height);
        match self.active_modal {
            ActiveModal::None => {}
            ActiveModal::MainMenu => {
                let items = vec![
                    MenuItem {
                        id: FocusId::from_str("menu.save"),
                        label: "Save File".to_owned(),
                        shortcut: "Ctrl+S".to_owned(),
                    },
                    MenuItem {
                        id: FocusId::from_str("menu.palette"),
                        label: "Command Palette".to_owned(),
                        shortcut: "Ctrl+K".to_owned(),
                    },
                    MenuItem {
                        id: FocusId::from_str("menu.settings"),
                        label: "Settings".to_owned(),
                        shortcut: "Ctrl+,".to_owned(),
                    },
                    MenuItem {
                        id: FocusId::from_str("menu.theme"),
                        label: "Toggle Dark / OLED Theme".to_owned(),
                        shortcut: String::new(),
                    },
                ];
                paint_menu(ctx, Point::new(38.0, 38.0), 240.0, &items);
            }
            ActiveModal::Settings => {
                let r = paint_dialog_frame(ctx, vp_rect, 540.0, 370.0, "Koren Settings");
                Button::new(FocusId::from_str("set.tab_general"), "General")
                    .variant(if self.settings_tab == SettingsTab::General {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .layer(2)
                    .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 48.0, 90.0, 26.0));
                Button::new(FocusId::from_str("set.tab_run"), "Run & Debug")
                    .variant(if self.settings_tab == SettingsTab::RunDebug {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .layer(2)
                    .paint_at(ctx, Rect::new(r.x + 114.0, r.y + 48.0, 110.0, 26.0));
                Button::new(FocusId::from_str("set.tab_tools"), "Tools & Runtimes")
                    .variant(if self.settings_tab == SettingsTab::ToolsRuntimes {
                        ButtonVariant::Secondary
                    } else {
                        ButtonVariant::Ghost
                    })
                    .layer(2)
                    .paint_at(ctx, Rect::new(r.x + 230.0, r.y + 48.0, 140.0, 26.0));

                match self.settings_tab {
                    SettingsTab::General => {
                        Toggle {
                            id: FocusId::from_str("set.oled"),
                            label: "OLED-Black High-Contrast Theme".to_owned(),
                            on: self.theme.kind == ThemeKind::OledBlack,
                        }
                        .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 90.0, r.width - 36.0, 28.0));
                        Checkbox {
                            id: FocusId::from_str("set.linenums"),
                            label: "Show Editor Line Numbers".to_owned(),
                            checked: self.setting_line_numbers,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 128.0, r.width - 36.0, 26.0),
                        );
                        Checkbox {
                            id: FocusId::from_str("set.guides"),
                            label: "Show Indent Guides".to_owned(),
                            checked: self.setting_indent_guides,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 162.0, r.width - 36.0, 26.0),
                        );
                        ctx.draw.push_text_layer(
                            2,
                            TextCommand {
                                pos: Point::new(r.x + 18.0, r.y + 204.0),
                                max_width: Some(r.width - 36.0),
                                text: "UI Scale: Ctrl + '+' / '-' / '0'".to_owned(),
                                size: 12.0,
                                line_height: 16.0,
                                color: self.theme.text_secondary,
                                weight: FontWeight::Regular,
                                monospace: false,
                                italic: false,
                                clip: Some(r),
                            },
                        );
                    }
                    SettingsTab::RunDebug => {
                        let out_label = match self.run_coord.settings.default_output {
                            OutputDestination::Run => {
                                "Default Output: Run Output (click to toggle)"
                            }
                            OutputDestination::Terminal => {
                                "Default Output: Integrated Terminal (click to toggle)"
                            }
                        };
                        Button::new(FocusId::from_str("set.run_output"), out_label)
                            .variant(ButtonVariant::Secondary)
                            .layer(2)
                            .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 90.0, r.width - 36.0, 28.0));
                        let save_label = match self.run_coord.settings.default_save {
                            SavePolicy::Ask => "Save Before Run: Ask (click to cycle)",
                            SavePolicy::Save => {
                                "Save Before Run: Save Automatically (click to cycle)"
                            }
                            SavePolicy::Disk => {
                                "Save Before Run: Run On-Disk File (click to cycle)"
                            }
                        };
                        Button::new(FocusId::from_str("set.run_save"), save_label)
                            .variant(ButtonVariant::Secondary)
                            .layer(2)
                            .paint_at(
                                ctx,
                                Rect::new(r.x + 18.0, r.y + 124.0, r.width - 36.0, 28.0),
                            );
                        Checkbox {
                            id: FocusId::from_str("set.run_reveal"),
                            label: "Reveal output panel on run".to_owned(),
                            checked: self.run_coord.settings.reveal_on_run,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 160.0, r.width - 36.0, 26.0),
                        );
                        Checkbox {
                            id: FocusId::from_str("set.run_reveal_fail"),
                            label: "Reveal output panel on failure".to_owned(),
                            checked: self.run_coord.settings.reveal_on_failure,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 192.0, r.width - 36.0, 26.0),
                        );
                        Checkbox {
                            id: FocusId::from_str("set.run_history"),
                            label: "Keep completed run history".to_owned(),
                            checked: self.run_coord.settings.keep_history,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 224.0, r.width - 36.0, 26.0),
                        );
                        Checkbox {
                            id: FocusId::from_str("set.run_confirm_stop"),
                            label: "Confirm before stopping active run".to_owned(),
                            checked: self.run_coord.settings.confirm_before_stop,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 256.0, r.width - 36.0, 26.0),
                        );
                    }
                    SettingsTab::ToolsRuntimes => {
                        Checkbox {
                            id: FocusId::from_str("set.tool_system"),
                            label: "Show system installations (PATH & hints)".to_owned(),
                            checked: self.run_coord.tool_settings.show_system_installations,
                        }
                        .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 90.0, r.width - 36.0, 26.0));
                        Checkbox {
                            id: FocusId::from_str("set.tool_confirm"),
                            label: "Require explicit confirmation before managed downloads"
                                .to_owned(),
                            checked: self.run_coord.tool_settings.confirm_before_download,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 124.0, r.width - 36.0, 26.0),
                        );
                        Checkbox {
                            id: FocusId::from_str("set.tool_incompat"),
                            label: "Show incompatible versions".to_owned(),
                            checked: self.run_coord.tool_settings.show_incompatible_versions,
                        }
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0, r.y + 158.0, r.width - 36.0, 26.0),
                        );
                        ctx.draw.push_text_layer(
                            2,
                            TextCommand {
                                pos: Point::new(r.x + 18.0, r.y + 196.0),
                                max_width: Some(r.width - 36.0),
                                text: format!(
                                    "Managed tool storage: {}",
                                    self.run_coord.tools.storage_root().display()
                                ),
                                size: 12.0,
                                line_height: 16.0,
                                color: self.theme.text_secondary,
                                weight: FontWeight::Regular,
                                monospace: true,
                                italic: false,
                                clip: Some(r),
                            },
                        );
                        Button::new(FocusId::from_str("cmd.tools_manager"), "Open Tool Manager…")
                            .variant(ButtonVariant::Primary)
                            .layer(2)
                            .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 230.0, 170.0, 28.0));
                    }
                }

                Button::new(FocusId::from_str("modal.close"), "Done")
                    .variant(ButtonVariant::Primary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 94.0, r.y + r.height - 42.0, 76.0, 28.0),
                    );
            }
            ActiveModal::CommandPalette => {
                let r = paint_dialog_frame(ctx, vp_rect, 520.0, 360.0, "Command Palette");
                TextInput {
                    id: FocusId::from_str("pal.input"),
                    value: &self.palette_query,
                    placeholder: "Type a file name or command (e.g. Run, Stop, Create)...",
                    layer: 2,
                }
                .paint_at(ctx, Rect::new(r.x + 14.0, r.y + 52.0, r.width - 28.0, 30.0));

                let q = self.palette_query.trim().to_ascii_lowercase();
                let mut rows: Vec<(FocusId, String)> = Vec::new();
                let run_cmds = [
                    ("cmd.run_selected", "Run: Run Selected Configuration (F5)"),
                    ("cmd.run_stop", "Run: Stop Active Run (Shift+F5)"),
                    (
                        "cmd.run_rerun",
                        "Run: Rerun Previous Configuration (Ctrl+F5)",
                    ),
                    ("cmd.run_select", "Run: Open Configuration Selector"),
                    ("cmd.run_create", "Run: Create Custom Configuration"),
                    ("cmd.run_manage", "Run: Manage Configurations"),
                    ("cmd.tools_manager", "Tools: Open Tools & Runtimes Manager"),
                ];
                for (id, label) in run_cmds {
                    if q.is_empty() || label.to_ascii_lowercase().contains(&q) {
                        rows.push((FocusId::from_str(id), label.to_owned()));
                    }
                }
                for node in self.tree_nodes.iter().filter(|n| {
                    !n.is_dir && (q.is_empty() || n.label.to_ascii_lowercase().contains(&q))
                }) {
                    if rows.len() >= 9 {
                        break;
                    }
                    rows.push((
                        FocusId::from_str(&format!("pal_open:{}", node.rel_path.display())),
                        node.rel_path.to_string_lossy().into_owned(),
                    ));
                }
                for (i, (id, label)) in rows.into_iter().take(9).enumerate() {
                    let row = Rect::new(
                        r.x + 14.0,
                        r.y + 92.0 + i as f32 * 26.0,
                        r.width - 28.0,
                        24.0,
                    );
                    Button::new(id, label)
                        .variant(ButtonVariant::Ghost)
                        .layer(2)
                        .align_left()
                        .paint_at(ctx, row);
                }
            }
            ActiveModal::RunSelector => {
                let mut items: Vec<MenuItem> = self
                    .run_coord
                    .entries
                    .iter()
                    .take(10)
                    .enumerate()
                    .map(|(i, entry)| {
                        let sel_mark = if self.run_coord.selected.as_ref() == Some(&entry.identity)
                        {
                            "✓ "
                        } else {
                            ""
                        };
                        MenuItem {
                            id: FocusId::from_str(&format!("run_pick:{i}")),
                            label: format!("{sel_mark}{}", entry.configuration.name),
                            shortcut: entry.identity.source.clone(),
                        }
                    })
                    .collect();
                items.push(MenuItem {
                    id: FocusId::from_str("run_menu.create"),
                    label: "+ Create Custom Tool…".to_owned(),
                    shortcut: String::new(),
                });
                items.push(MenuItem {
                    id: FocusId::from_str("run_menu.manage"),
                    label: "Manage Configurations…".to_owned(),
                    shortcut: String::new(),
                });
                let sel_x = if self.run_coord.is_running() || self.run_coord.last_run.is_some() {
                    viewport.width - 290.0 - 178.0
                } else {
                    viewport.width - 260.0 - 178.0
                };
                paint_menu(ctx, Point::new(sel_x.max(16.0), 38.0), 280.0, &items);
            }
            ActiveModal::CreateRunConfig => {
                let r = paint_dialog_frame(
                    ctx,
                    vp_rect,
                    560.0,
                    if self.run_draft.as_ref().is_some_and(|d| d.show_advanced) {
                        470.0
                    } else {
                        390.0
                    },
                    if self
                        .run_draft
                        .as_ref()
                        .is_some_and(|d| d.editing_identity.is_some())
                    {
                        "Edit Run Configuration"
                    } else {
                        "Create Run Configuration"
                    },
                );
                if let Some(ref draft) = self.run_draft {
                    TextInput {
                        id: FocusId::from_str("rc.name"),
                        value: &draft.name,
                        placeholder: "Name (e.g. Run Lua)",
                        layer: 2,
                    }
                    .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 50.0, r.width - 36.0, 28.0));

                    TextInput {
                        id: FocusId::from_str("rc.exec"),
                        value: &draft.executable,
                        placeholder: "Executable (e.g. lua5.4 or /usr/bin/python3)",
                        layer: 2,
                    }
                    .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 86.0, r.width - 36.0, 28.0));

                    TextInput {
                        id: FocusId::from_str("rc.args"),
                        value: &draft.args_simple,
                        placeholder: "Arguments (e.g. ${file} --flag \"with spaces\")",
                        layer: 2,
                    }
                    .paint_at(
                        ctx,
                        Rect::new(r.x + 18.0, r.y + 122.0, r.width - 36.0, 28.0),
                    );

                    TextInput {
                        id: FocusId::from_str("rc.cwd"),
                        value: &draft.cwd,
                        placeholder: "Working Directory (default: ${workspace})",
                        layer: 2,
                    }
                    .paint_at(
                        ctx,
                        Rect::new(r.x + 18.0, r.y + 158.0, r.width - 36.0, 28.0),
                    );

                    let out_txt = match draft.output {
                        OutputDestination::Run => "Output: Run Output",
                        OutputDestination::Terminal => "Output: Integrated Terminal",
                    };
                    Button::new(FocusId::from_str("rc.toggle_output"), out_txt)
                        .variant(ButtonVariant::Secondary)
                        .layer(2)
                        .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 194.0, 248.0, 26.0));

                    let scope_txt = if draft.scope == "user" {
                        "Scope: User"
                    } else {
                        "Scope: Workspace (.koren/runs.toml)"
                    };
                    Button::new(FocusId::from_str("rc.toggle_scope"), scope_txt)
                        .variant(ButtonVariant::Secondary)
                        .layer(2)
                        .paint_at(ctx, Rect::new(r.x + 274.0, r.y + 194.0, 268.0, 26.0));

                    Button::new(
                        FocusId::from_str("rc.toggle_adv"),
                        if draft.show_advanced {
                            "Hide Advanced ▾"
                        } else {
                            "Show Advanced ▸"
                        },
                    )
                    .variant(ButtonVariant::Ghost)
                    .layer(2)
                    .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 228.0, 140.0, 24.0));

                    let mut preview_y = r.y + 260.0;
                    if draft.show_advanced {
                        let save_txt = match draft.save {
                            SavePolicy::Ask => "Save Policy: Ask",
                            SavePolicy::Save => "Save Policy: Save Before Run",
                            SavePolicy::Disk => "Save Policy: Run On-Disk",
                        };
                        Button::new(FocusId::from_str("rc.toggle_save"), save_txt)
                            .variant(ButtonVariant::Secondary)
                            .layer(2)
                            .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 258.0, 248.0, 26.0));
                        TextInput {
                            id: FocusId::from_str("rc.langs"),
                            value: &draft.languages_csv,
                            placeholder: "Languages (optional, e.g. lua, rust)",
                            layer: 2,
                        }
                        .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 292.0, 250.0, 28.0));
                        TextInput {
                            id: FocusId::from_str("rc.timeout"),
                            value: &draft.timeout_ms_text,
                            placeholder: "Timeout ms (optional)",
                            layer: 2,
                        }
                        .paint_at(ctx, Rect::new(r.x + 276.0, r.y + 292.0, 266.0, 28.0));
                        preview_y = r.y + 330.0;
                    }

                    let preview_text = if let Some(ref err) = draft.error {
                        format!("Error: {err}")
                    } else if let Some(root) = &self.workspace_root {
                        match draft.to_configuration().and_then(|cfg| {
                            let active = self.active_file_abs();
                            resolve_run(
                                &cfg,
                                &RunContext {
                                    workspace: root,
                                    file: active.as_deref(),
                                },
                            )
                            .map_err(|e| e.to_string())
                        }) {
                            Ok(resolved) => {
                                let warn = if executable_exists(&resolved.executable) {
                                    ""
                                } else {
                                    " [missing executable]"
                                };
                                format!(
                                    "Preview: {} {}{warn}\nCwd: {}",
                                    resolved.executable,
                                    resolved.args.join(" "),
                                    resolved.cwd.display()
                                )
                            }
                            Err(err) => format!("Preview: {err}"),
                        }
                    } else {
                        "Preview requires an open workspace.".to_owned()
                    };
                    ctx.draw.push_text_layer(
                        2,
                        TextCommand {
                            pos: Point::new(r.x + 18.0, preview_y),
                            max_width: Some(r.width - 36.0),
                            text: preview_text,
                            size: 11.5,
                            line_height: 16.0,
                            color: if draft.error.is_some() {
                                self.theme.error
                            } else {
                                self.theme.text_secondary
                            },
                            weight: FontWeight::Regular,
                            monospace: true,
                            italic: false,
                            clip: Some(r),
                        },
                    );
                }
                Button::new(FocusId::from_str("modal.close"), "Cancel")
                    .variant(ButtonVariant::Secondary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 182.0, r.y + r.height - 42.0, 80.0, 28.0),
                    );
                Button::new(FocusId::from_str("rc.save"), "Save")
                    .variant(ButtonVariant::Primary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 94.0, r.y + r.height - 42.0, 76.0, 28.0),
                    );
            }
            ActiveModal::RunManager => {
                let r = paint_dialog_frame(ctx, vp_rect, 620.0, 420.0, "Run Configurations");
                Button::new(FocusId::from_str("rm.new"), "+ Create")
                    .variant(ButtonVariant::Primary)
                    .layer(2)
                    .paint_at(ctx, Rect::new(r.x + 18.0, r.y + 46.0, 88.0, 26.0));
                for (i, entry) in self.run_coord.entries.iter().take(10).enumerate() {
                    let row_y = r.y + 82.0 + i as f32 * 28.0;
                    let label = format!("[{}] {}", entry.identity.source, entry.configuration.name);
                    Button::new(FocusId::from_str(&format!("rm_select:{i}")), label)
                        .variant(
                            if self.run_coord.selected.as_ref() == Some(&entry.identity) {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Ghost
                            },
                        )
                        .layer(2)
                        .align_left()
                        .paint_at(ctx, Rect::new(r.x + 18.0, row_y, 320.0, 24.0));
                    Button::new(FocusId::from_str(&format!("rm_run:{i}")), "Run")
                        .variant(ButtonVariant::Ghost)
                        .layer(2)
                        .paint_at(ctx, Rect::new(r.x + 344.0, row_y, 50.0, 24.0));
                    Button::new(FocusId::from_str(&format!("rm_dup:{i}")), "Copy")
                        .variant(ButtonVariant::Ghost)
                        .layer(2)
                        .paint_at(ctx, Rect::new(r.x + 398.0, row_y, 56.0, 24.0));
                    if entry.editable {
                        Button::new(FocusId::from_str(&format!("rm_edit:{i}")), "Edit")
                            .variant(ButtonVariant::Ghost)
                            .layer(2)
                            .paint_at(ctx, Rect::new(r.x + 458.0, row_y, 54.0, 24.0));
                        Button::new(FocusId::from_str(&format!("rm_del:{i}")), "Delete")
                            .variant(ButtonVariant::Ghost)
                            .layer(2)
                            .paint_at(ctx, Rect::new(r.x + 516.0, row_y, 64.0, 24.0));
                    }
                }
                Button::new(FocusId::from_str("modal.close"), "Done")
                    .variant(ButtonVariant::Secondary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 94.0, r.y + r.height - 42.0, 76.0, 28.0),
                    );
            }
            ActiveModal::DirtyRunPrompt => {
                let r = paint_dialog_frame(
                    ctx,
                    vp_rect,
                    480.0,
                    190.0,
                    "Unsaved Changes in Active File",
                );
                ctx.draw.push_text_layer(
                    2,
                    TextCommand {
                        pos: Point::new(r.x + 18.0, r.y + 56.0),
                        max_width: Some(r.width - 36.0),
                        text: "The active document has unsaved edits. Save before running or run the existing on-disk file?".to_owned(),
                        size: 13.0,
                        line_height: 19.0,
                        color: self.theme.text_primary,
                        weight: FontWeight::Regular,
                        monospace: false,
                        italic: false,
                        clip: Some(r),
                    },
                );
                Button::new(FocusId::from_str("modal.close"), "Cancel")
                    .variant(ButtonVariant::Secondary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + 18.0, r.y + r.height - 42.0, 80.0, 28.0),
                    );
                Button::new(FocusId::from_str("dirty.disk"), "Run On-Disk")
                    .variant(ButtonVariant::Secondary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 210.0, r.y + r.height - 42.0, 100.0, 28.0),
                    );
                Button::new(FocusId::from_str("dirty.save"), "Save & Run")
                    .variant(ButtonVariant::Primary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 102.0, r.y + r.height - 42.0, 84.0, 28.0),
                    );
            }
            ActiveModal::RunApproval => {
                let r = paint_dialog_frame(ctx, vp_rect, 520.0, 230.0, "Approve Process Execution");
                let details = if let Some(ref pending) = self.run_coord.pending_approval {
                    let env_keys = if pending.resolved.environment.is_empty() {
                        "none".to_owned()
                    } else {
                        pending
                            .resolved
                            .environment
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let warn = if pending.disk_warning {
                        "\nWarning: running on-disk file with unsaved buffer edits."
                    } else {
                        ""
                    };
                    format!(
                        "Name: {}\nExecutable: {}\nArguments: {:?}\nWorking dir: {}\nEnv keys: {env_keys}{warn}",
                        pending.name,
                        pending.resolved.executable,
                        pending.resolved.args,
                        pending.resolved.cwd.display()
                    )
                } else {
                    "No pending run request.".to_owned()
                };
                ctx.draw.push_text_layer(
                    2,
                    TextCommand {
                        pos: Point::new(r.x + 18.0, r.y + 54.0),
                        max_width: Some(r.width - 36.0),
                        text: details,
                        size: 12.5,
                        line_height: 18.5,
                        color: self.theme.text_primary,
                        weight: FontWeight::Regular,
                        monospace: true,
                        italic: false,
                        clip: Some(r),
                    },
                );
                Button::new(FocusId::from_str("run.deny"), "Cancel")
                    .variant(ButtonVariant::Secondary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 230.0, r.y + r.height - 42.0, 80.0, 28.0),
                    );
                Button::new(FocusId::from_str("run.approve"), "Approve & Run")
                    .variant(ButtonVariant::Primary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 142.0, r.y + r.height - 42.0, 124.0, 28.0),
                    );
            }
            ActiveModal::ToolManager => {
                let r = paint_dialog_frame(ctx, vp_rect, 660.0, 440.0, "Tools & Runtimes");
                let cats: [(&str, Option<ToolCategory>, &str); 5] = [
                    ("tm.cat_all", None, "All"),
                    ("tm.cat_lsp", Some(ToolCategory::LanguageServer), "LSP"),
                    ("tm.cat_rt", Some(ToolCategory::Runtime), "Runtimes"),
                    ("tm.cat_fmt", Some(ToolCategory::Formatter), "Formatters"),
                    ("tm.cat_dbg", Some(ToolCategory::Debugger), "Debuggers"),
                ];
                for (idx, (fid, cat, label)) in cats.iter().enumerate() {
                    Button::new(FocusId::from_str(fid), *label)
                        .variant(if self.tool_category_filter == *cat {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .layer(2)
                        .paint_at(
                            ctx,
                            Rect::new(r.x + 18.0 + idx as f32 * 96.0, r.y + 46.0, 90.0, 24.0),
                        );
                }

                let decls: Vec<_> = self
                    .run_coord
                    .tools
                    .declarations()
                    .into_iter()
                    .filter(|(d, _)| self.tool_category_filter.is_none_or(|c| d.category == c))
                    .collect();

                if decls.is_empty() {
                    ctx.draw.push_text_layer(
                        2,
                        TextCommand {
                            pos: Point::new(r.x + 18.0, r.y + 92.0),
                            max_width: Some(r.width - 36.0),
                            text: "No extension-declared tools active yet. Enable an extension with [[tools]] or configure a tool manually below.".to_owned(),
                            size: 12.5,
                            line_height: 18.0,
                            color: self.theme.text_secondary,
                            weight: FontWeight::Regular,
                            monospace: false,
                            italic: false,
                            clip: Some(r),
                        },
                    );
                } else {
                    for (i, (decl, owners)) in decls.iter().take(6).enumerate() {
                        let y = r.y + 84.0 + i as f32 * 28.0;
                        let active = self.selected_tool_id.as_deref() == Some(decl.id.as_str());
                        Button::new(
                            FocusId::from_str(&format!("tm_pick:{i}")),
                            format!("{} ({})", decl.name, owners.join(", ")),
                        )
                        .variant(if active {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Ghost
                        })
                        .layer(2)
                        .align_left()
                        .paint_at(ctx, Rect::new(r.x + 18.0, y, 240.0, 24.0));
                    }
                    if let Some(ref tid) = self.selected_tool_id {
                        let ws_key = self
                            .workspace_root
                            .as_ref()
                            .map(|p| p.to_string_lossy().into_owned());
                        let status_text =
                            match self.run_coord.tools.resolve(tid, None, ws_key.as_deref()) {
                                Ok(res) => format!(
                                    "Active: {} ({:?})",
                                    res.executable_path.display(),
                                    res.source
                                ),
                                Err(_) => "Status: Not configured".to_owned(),
                            };
                        ctx.draw.push_text_layer(
                            2,
                            TextCommand {
                                pos: Point::new(r.x + 276.0, r.y + 88.0),
                                max_width: Some(r.width - 294.0),
                                text: status_text,
                                size: 12.0,
                                line_height: 17.0,
                                color: self.theme.text_primary,
                                weight: FontWeight::Regular,
                                monospace: true,
                                italic: false,
                                clip: Some(r),
                            },
                        );
                        if self.run_coord.tool_settings.show_system_installations {
                            let discovered = self.run_coord.tools.discover_local(tid);
                            if let Some(first) = discovered.first() {
                                Button::new(
                                    FocusId::from_str("tm.use_detected"),
                                    format!("Use Detected: {}", first.path.display()),
                                )
                                .variant(ButtonVariant::Secondary)
                                .layer(2)
                                .paint_at(
                                    ctx,
                                    Rect::new(r.x + 276.0, r.y + 124.0, r.width - 294.0, 26.0),
                                );
                            }
                        }
                        let managed = self.run_coord.tools.managed_versions(tid);
                        for (mi, inst) in managed.iter().take(4).enumerate() {
                            let my = r.y + 160.0 + mi as f32 * 28.0;
                            Button::new(
                                FocusId::from_str(&format!("tm_use_managed:{mi}")),
                                format!("Use Managed v{}", inst.version),
                            )
                            .variant(ButtonVariant::Secondary)
                            .layer(2)
                            .paint_at(ctx, Rect::new(r.x + 276.0, my, 220.0, 24.0));
                            Button::new(
                                FocusId::from_str(&format!("tm_rm_managed:{mi}")),
                                "Remove",
                            )
                            .variant(ButtonVariant::Ghost)
                            .layer(2)
                            .paint_at(ctx, Rect::new(r.x + 504.0, my, 76.0, 24.0));
                        }
                    }
                }

                TextInput {
                    id: FocusId::from_str("tm.manual_input"),
                    value: &self.tool_manual_path,
                    placeholder: "Manual executable path (e.g. /usr/bin/lua5.4)",
                    layer: 2,
                }
                .paint_at(
                    ctx,
                    Rect::new(r.x + 18.0, r.y + r.height - 86.0, r.width - 150.0, 28.0),
                );
                Button::new(FocusId::from_str("tm.use_manual"), "Use Path")
                    .variant(ButtonVariant::Secondary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 124.0, r.y + r.height - 86.0, 106.0, 28.0),
                    );
                Button::new(FocusId::from_str("modal.close"), "Done")
                    .variant(ButtonVariant::Primary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 94.0, r.y + r.height - 42.0, 76.0, 28.0),
                    );
            }
            ActiveModal::ToolSetupPrompt => {
                let r = paint_dialog_frame(ctx, vp_rect, 480.0, 200.0, "External Tool Required");
                let tid = self.pending_tool_setup.as_deref().unwrap_or("tool");
                ctx.draw.push_text_layer(
                    2,
                    TextCommand {
                        pos: Point::new(r.x + 18.0, r.y + 56.0),
                        max_width: Some(r.width - 36.0),
                        text: format!(
                            "This configuration requires '{tid}'. Use an existing installation, select an executable manually, or open Tools & Runtimes."
                        ),
                        size: 13.0,
                        line_height: 19.0,
                        color: self.theme.text_primary,
                        weight: FontWeight::Regular,
                        monospace: false,
                        italic: false,
                        clip: Some(r),
                    },
                );
                Button::new(FocusId::from_str("tsp.dismiss"), "Don't Ask Again")
                    .variant(ButtonVariant::Ghost)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + 18.0, r.y + r.height - 42.0, 136.0, 28.0),
                    );
                Button::new(FocusId::from_str("modal.close"), "Cancel")
                    .variant(ButtonVariant::Secondary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 240.0, r.y + r.height - 42.0, 80.0, 28.0),
                    );
                Button::new(FocusId::from_str("tsp.configure"), "Configure Tool…")
                    .variant(ButtonVariant::Primary)
                    .layer(2)
                    .paint_at(
                        ctx,
                        Rect::new(r.x + r.width - 152.0, r.y + r.height - 42.0, 134.0, 28.0),
                    );
            }
        }
    }
}

impl NativeApp for KorenNativeApp {
    fn title(&self) -> &str {
        "Koren IDE (Native)"
    }

    fn theme(&self) -> &ThemeTokens {
        &self.theme
    }

    fn view(&mut self, ctx: &mut UiContext<'_>, viewport: Size) {
        if let Some((msg, is_error)) = self.run_coord.poll_events(&mut self.output_lines) {
            if is_error && self.run_coord.settings.reveal_on_failure {
                self.bottom_visible = true;
                self.bottom_tab = BottomTab::Output;
            }
            self.toast = Some(ToastMessage {
                title: if is_error {
                    "Run Failed".to_owned()
                } else {
                    "Run Finished".to_owned()
                },
                body: msg,
                is_error,
            });
        }
        self.paint_top_toolbar(ctx, viewport);
        self.paint_status_bar(ctx, viewport);

        let m = self.theme.metrics;
        let top = m.toolbar_height;
        let bottom_reserved = m.statusbar_height;
        let avail_h = (viewport.height - top - bottom_reserved).max(0.0);

        let mut content_h = avail_h;
        if self.bottom_visible {
            let bh = self.bottom_height.min(avail_h * 0.5);
            content_h = (avail_h - bh - m.island_gap).max(0.0);
            let bottom_rect = Rect::new(
                m.outer_margin,
                top + content_h + m.island_gap,
                (viewport.width - 2.0 * m.outer_margin).max(0.0),
                bh,
            );
            self.paint_bottom_island(ctx, bottom_rect);
        }

        let mut editor_x = m.outer_margin;
        let mut editor_w = (viewport.width - 2.0 * m.outer_margin).max(0.0);
        if self.sidebar_visible {
            let sw = self.sidebar_width.min(viewport.width * 0.45);
            let sb_rect = Rect::new(m.outer_margin, top, sw, content_h);
            self.paint_sidebar_island(ctx, sb_rect);
            editor_x += sw + m.island_gap;
            editor_w = (editor_w - sw - m.island_gap).max(0.0);
        }
        let ed_rect = Rect::new(editor_x, top, editor_w, content_h);
        self.paint_editor_island(ctx, ed_rect);
        self.paint_overlays(ctx, viewport);
    }

    fn on_click(
        &mut self,
        target: Option<FocusId>,
        pos: Point,
        button: MouseButton,
        input: &mut InputState,
    ) -> Option<WindowCommand> {
        let Some(id) = target else {
            if pos.y < self.theme.metrics.toolbar_height && button == MouseButton::Left {
                return Some(WindowCommand::DragWindow);
            }
            self.active_modal = ActiveModal::None;
            return None;
        };

        if id == FocusId::from_str("win.close") {
            return Some(WindowCommand::Close);
        }
        if id == FocusId::from_str("win.min") {
            return Some(WindowCommand::Minimize);
        }
        if id == FocusId::from_str("win.max") {
            return Some(WindowCommand::ToggleMaximize);
        }
        if id == FocusId::from_str("tb.left") {
            self.sidebar_visible = !self.sidebar_visible;
            return None;
        }
        if id == FocusId::from_str("tb.bottom") {
            self.bottom_visible = !self.bottom_visible;
            return None;
        }
        if id == FocusId::from_str("tb.right")
            || id == FocusId::from_str("menu.theme")
            || id == FocusId::from_str("set.oled")
        {
            self.theme = match self.theme.kind {
                ThemeKind::Dark => ThemeTokens::oled_black(),
                ThemeKind::OledBlack => ThemeTokens::dark(),
            };
            return None;
        }
        if id == FocusId::from_str("tb.menu") {
            self.active_modal = if self.active_modal == ActiveModal::MainMenu {
                ActiveModal::None
            } else {
                ActiveModal::MainMenu
            };
            return None;
        }
        if id == FocusId::from_str("tb.settings") || id == FocusId::from_str("menu.settings") {
            self.active_modal = ActiveModal::Settings;
            return None;
        }
        if id == FocusId::from_str("set.tab_general") {
            self.settings_tab = SettingsTab::General;
            return None;
        }
        if id == FocusId::from_str("set.tab_run") {
            self.settings_tab = SettingsTab::RunDebug;
            return None;
        }
        if id == FocusId::from_str("set.tab_tools") {
            self.settings_tab = SettingsTab::ToolsRuntimes;
            return None;
        }
        if id == FocusId::from_str("set.tool_system") {
            self.run_coord.tool_settings.show_system_installations =
                !self.run_coord.tool_settings.show_system_installations;
            return None;
        }
        if id == FocusId::from_str("set.tool_confirm") {
            self.run_coord.tool_settings.confirm_before_download =
                !self.run_coord.tool_settings.confirm_before_download;
            return None;
        }
        if id == FocusId::from_str("set.tool_incompat") {
            self.run_coord.tool_settings.show_incompatible_versions =
                !self.run_coord.tool_settings.show_incompatible_versions;
            return None;
        }
        if id == FocusId::from_str("cmd.tools_manager") {
            self.active_modal = ActiveModal::ToolManager;
            input.focused = Some(FocusId::from_str("tm.manual_input"));
            return None;
        }
        if id == FocusId::from_str("tm.cat_all") {
            self.tool_category_filter = None;
            return None;
        }
        if id == FocusId::from_str("tm.cat_lsp") {
            self.tool_category_filter = Some(ToolCategory::LanguageServer);
            return None;
        }
        if id == FocusId::from_str("tm.cat_rt") {
            self.tool_category_filter = Some(ToolCategory::Runtime);
            return None;
        }
        if id == FocusId::from_str("tm.cat_fmt") {
            self.tool_category_filter = Some(ToolCategory::Formatter);
            return None;
        }
        if id == FocusId::from_str("tm.cat_dbg") {
            self.tool_category_filter = Some(ToolCategory::Debugger);
            return None;
        }
        if id == FocusId::from_str("tm.use_detected") {
            if let Some(ref tid) = self.selected_tool_id.clone()
                && let Some(first) = self.run_coord.tools.discover_local(tid).into_iter().next()
            {
                let ws_key = self
                    .workspace_root
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned());
                let _ = self
                    .run_coord
                    .tools
                    .select_system(tid, first.path, ws_key.as_deref());
            }
            return None;
        }
        if id == FocusId::from_str("tm.use_manual") {
            if let Some(ref tid) = self.selected_tool_id.clone() {
                let ws_key = self
                    .workspace_root
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned());
                if let Err(err) = self.run_coord.tools.select_manual(
                    tid,
                    PathBuf::from(self.tool_manual_path.trim()),
                    ws_key.as_deref(),
                ) {
                    self.toast = Some(ToastMessage {
                        title: "Invalid Tool Path".to_owned(),
                        body: err.to_string(),
                        is_error: true,
                    });
                }
            }
            return None;
        }
        for i in 0..self.run_coord.tools.declarations().len() {
            if id == FocusId::from_str(&format!("tm_pick:{i}")) {
                let decls: Vec<String> = self
                    .run_coord
                    .tools
                    .declarations()
                    .into_iter()
                    .filter(|(d, _)| self.tool_category_filter.is_none_or(|c| d.category == c))
                    .map(|(d, _)| d.id.clone())
                    .collect();
                if let Some(tid) = decls.get(i) {
                    self.selected_tool_id = Some(tid.clone());
                }
                return None;
            }
        }
        if let Some(ref tid) = self.selected_tool_id.clone() {
            let versions: Vec<String> = self
                .run_coord
                .tools
                .managed_versions(tid)
                .into_iter()
                .map(|m| m.version.clone())
                .collect();
            for (mi, ver) in versions.iter().enumerate() {
                if id == FocusId::from_str(&format!("tm_use_managed:{mi}")) {
                    let ws_key = self
                        .workspace_root
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned());
                    if let Err(err) =
                        self.run_coord
                            .tools
                            .select_managed(tid, ver, ws_key.as_deref())
                    {
                        self.toast = Some(ToastMessage {
                            title: "Cannot Select Managed Version".to_owned(),
                            body: err.to_string(),
                            is_error: true,
                        });
                    }
                    return None;
                }
                if id == FocusId::from_str(&format!("tm_rm_managed:{mi}")) {
                    if let Err(err) = self.run_coord.tools.remove_managed_version(tid, ver, false) {
                        self.toast = Some(ToastMessage {
                            title: "Version In Use".to_owned(),
                            body: err.to_string(),
                            is_error: true,
                        });
                    }
                    return None;
                }
            }
        }
        if id == FocusId::from_str("tsp.dismiss") {
            if let Some(tid) = self.pending_tool_setup.take() {
                let _ = self.run_coord.tools.dismiss_prompt(&tid);
            }
            self.active_modal = ActiveModal::None;
            return None;
        }
        if id == FocusId::from_str("tsp.configure") {
            self.selected_tool_id = self.pending_tool_setup.take();
            self.active_modal = ActiveModal::ToolManager;
            input.focused = Some(FocusId::from_str("tm.manual_input"));
            return None;
        }
        if id == FocusId::from_str("set.run_output") {
            self.run_coord.settings.default_output = match self.run_coord.settings.default_output {
                OutputDestination::Run => OutputDestination::Terminal,
                OutputDestination::Terminal => OutputDestination::Run,
            };
            return None;
        }
        if id == FocusId::from_str("set.run_save") {
            self.run_coord.settings.default_save = match self.run_coord.settings.default_save {
                SavePolicy::Ask => SavePolicy::Save,
                SavePolicy::Save => SavePolicy::Disk,
                SavePolicy::Disk => SavePolicy::Ask,
            };
            return None;
        }
        if id == FocusId::from_str("set.run_reveal") {
            self.run_coord.settings.reveal_on_run = !self.run_coord.settings.reveal_on_run;
            return None;
        }
        if id == FocusId::from_str("set.run_reveal_fail") {
            self.run_coord.settings.reveal_on_failure = !self.run_coord.settings.reveal_on_failure;
            return None;
        }
        if id == FocusId::from_str("set.run_history") {
            self.run_coord.settings.keep_history = !self.run_coord.settings.keep_history;
            return None;
        }
        if id == FocusId::from_str("set.run_confirm_stop") {
            self.run_coord.settings.confirm_before_stop =
                !self.run_coord.settings.confirm_before_stop;
            return None;
        }
        if id == FocusId::from_str("tb.search") || id == FocusId::from_str("menu.palette") {
            self.active_modal = ActiveModal::CommandPalette;
            input.focused = Some(FocusId::from_str("pal.input"));
            return None;
        }
        if id == FocusId::from_str("tb.run_select") || id == FocusId::from_str("cmd.run_select") {
            if self.run_coord.entries.is_empty() {
                self.run_draft = Some(RunDraft::new_custom(
                    self.run_coord.settings.default_output,
                    self.run_coord.settings.default_save,
                ));
                self.active_modal = ActiveModal::CreateRunConfig;
                input.focused = Some(FocusId::from_str("rc.name"));
            } else {
                self.active_modal = if self.active_modal == ActiveModal::RunSelector {
                    ActiveModal::None
                } else {
                    ActiveModal::RunSelector
                };
            }
            return None;
        }
        if id == FocusId::from_str("tb.run") || id == FocusId::from_str("cmd.run_selected") {
            self.active_modal = ActiveModal::None;
            if let Some(sel) = self.run_coord.selected.clone() {
                self.trigger_run_for_identity(sel, None);
            } else {
                self.run_draft = Some(RunDraft::new_custom(
                    self.run_coord.settings.default_output,
                    self.run_coord.settings.default_save,
                ));
                self.active_modal = ActiveModal::CreateRunConfig;
                input.focused = Some(FocusId::from_str("rc.name"));
            }
            return None;
        }
        if id == FocusId::from_str("tb.stop")
            || id == FocusId::from_str("bp.stop")
            || id == FocusId::from_str("cmd.run_stop")
        {
            self.active_modal = ActiveModal::None;
            self.run_coord.stop_active();
            return None;
        }
        if id == FocusId::from_str("tb.rerun")
            || id == FocusId::from_str("bp.rerun")
            || id == FocusId::from_str("cmd.run_rerun")
        {
            self.active_modal = ActiveModal::None;
            if let Some(last) = self.run_coord.last_run.clone() {
                self.trigger_run_for_identity(last, None);
            }
            return None;
        }
        if id == FocusId::from_str("tb.debug") {
            self.toast = Some(ToastMessage {
                title: "Debug".to_owned(),
                body: "Select a run configuration with a debug adapter in Run Manager.".to_owned(),
                is_error: false,
            });
            return None;
        }
        if id == FocusId::from_str("run_menu.create")
            || id == FocusId::from_str("rm.new")
            || id == FocusId::from_str("cmd.run_create")
        {
            self.run_draft = Some(RunDraft::new_custom(
                self.run_coord.settings.default_output,
                self.run_coord.settings.default_save,
            ));
            self.active_modal = ActiveModal::CreateRunConfig;
            input.focused = Some(FocusId::from_str("rc.name"));
            return None;
        }
        if id == FocusId::from_str("run_menu.manage") || id == FocusId::from_str("cmd.run_manage") {
            self.active_modal = ActiveModal::RunManager;
            return None;
        }
        if id == FocusId::from_str("rc.toggle_output") {
            if let Some(ref mut draft) = self.run_draft {
                draft.output = match draft.output {
                    OutputDestination::Run => OutputDestination::Terminal,
                    OutputDestination::Terminal => OutputDestination::Run,
                };
            }
            return None;
        }
        if id == FocusId::from_str("rc.toggle_scope") {
            if let Some(ref mut draft) = self.run_draft {
                draft.scope = if draft.scope == "user" {
                    "workspace".to_owned()
                } else {
                    "user".to_owned()
                };
            }
            return None;
        }
        if id == FocusId::from_str("rc.toggle_adv") {
            if let Some(ref mut draft) = self.run_draft {
                draft.show_advanced = !draft.show_advanced;
            }
            return None;
        }
        if id == FocusId::from_str("rc.toggle_save") {
            if let Some(ref mut draft) = self.run_draft {
                draft.save = match draft.save {
                    SavePolicy::Ask => SavePolicy::Save,
                    SavePolicy::Save => SavePolicy::Disk,
                    SavePolicy::Disk => SavePolicy::Ask,
                };
            }
            return None;
        }
        if id == FocusId::from_str("rc.save") {
            if let Some(ref mut draft) = self.run_draft {
                match draft.to_configuration() {
                    Ok(config) => {
                        let scope = draft.scope.clone();
                        match self.run_coord.save_configuration(
                            &scope,
                            config,
                            self.workspace_root.as_deref(),
                        ) {
                            Ok(_) => {
                                self.run_draft = None;
                                self.active_modal = ActiveModal::None;
                            }
                            Err(err) => {
                                draft.error = Some(err);
                            }
                        }
                    }
                    Err(err) => {
                        draft.error = Some(err);
                    }
                }
            }
            return None;
        }
        if id == FocusId::from_str("dirty.save") {
            self.active_modal = ActiveModal::None;
            if let Some(identity) = self.pending_dirty_identity.take() {
                self.trigger_run_for_identity(identity, Some("save"));
            }
            return None;
        }
        if id == FocusId::from_str("dirty.disk") {
            self.active_modal = ActiveModal::None;
            if let Some(identity) = self.pending_dirty_identity.take() {
                self.trigger_run_for_identity(identity, Some("disk"));
            }
            return None;
        }
        if id == FocusId::from_str("run.approve") {
            self.active_modal = ActiveModal::None;
            self.execute_approved_run();
            return None;
        }
        if id == FocusId::from_str("run.deny") {
            self.active_modal = ActiveModal::None;
            self.run_coord.deny_pending();
            return None;
        }
        for i in 0..self.run_coord.entries.len() {
            if id == FocusId::from_str(&format!("run_pick:{i}")) {
                let identity = self.run_coord.entries[i].identity.clone();
                self.run_coord
                    .select(Some(identity), self.workspace_root.as_deref());
                self.active_modal = ActiveModal::None;
                return None;
            }
            if id == FocusId::from_str(&format!("rm_select:{i}")) {
                let identity = self.run_coord.entries[i].identity.clone();
                self.run_coord
                    .select(Some(identity), self.workspace_root.as_deref());
                return None;
            }
            if id == FocusId::from_str(&format!("rm_run:{i}")) {
                let identity = self.run_coord.entries[i].identity.clone();
                self.active_modal = ActiveModal::None;
                self.trigger_run_for_identity(identity, None);
                return None;
            }
            if id == FocusId::from_str(&format!("rm_dup:{i}")) {
                self.run_draft = Some(RunDraft::from_entry(&self.run_coord.entries[i], true));
                self.active_modal = ActiveModal::CreateRunConfig;
                input.focused = Some(FocusId::from_str("rc.name"));
                return None;
            }
            if id == FocusId::from_str(&format!("rm_edit:{i}")) {
                self.run_draft = Some(RunDraft::from_entry(&self.run_coord.entries[i], false));
                self.active_modal = ActiveModal::CreateRunConfig;
                input.focused = Some(FocusId::from_str("rc.name"));
                return None;
            }
            if id == FocusId::from_str(&format!("rm_del:{i}")) {
                let identity = self.run_coord.entries[i].identity.clone();
                let _ = self
                    .run_coord
                    .delete_configuration(&identity, self.workspace_root.as_deref());
                return None;
            }
        }
        if id == FocusId::from_str("menu.save") {
            self.active_modal = ActiveModal::None;
            self.save_active_tab();
            return None;
        }
        if id == FocusId::from_str("modal.close") {
            self.active_modal = ActiveModal::None;
            self.run_coord.deny_pending();
            self.pending_dirty_identity = None;
            return None;
        }
        if id == FocusId::from_str("toast.dismiss") || id == FocusId::from_str("tb.bell") {
            self.toast = if self.toast.is_some() {
                None
            } else {
                Some(ToastMessage {
                    title: "Koren Platform".to_owned(),
                    body: format!("Workspace '{}' active.", self.workspace_name),
                    is_error: false,
                })
            };
            return None;
        }
        if id == FocusId::from_str("sb.files") {
            self.sidebar_tab = SidebarTab::Files;
            return None;
        }
        if id == FocusId::from_str("sb.search") {
            self.sidebar_tab = SidebarTab::Search;
            input.focused = Some(FocusId::from_str("sb.search_input"));
            return None;
        }
        if id == FocusId::from_str("bp.output") {
            self.bottom_tab = BottomTab::Output;
            return None;
        }
        if id == FocusId::from_str("bp.problems") {
            self.bottom_tab = BottomTab::Problems;
            return None;
        }
        if id == FocusId::from_str("bp.terminal") {
            self.bottom_tab = BottomTab::Terminal;
            return None;
        }
        if id == FocusId::from_str("set.linenums") {
            self.setting_line_numbers = !self.setting_line_numbers;
            return None;
        }
        if id == FocusId::from_str("set.guides") {
            self.setting_indent_guides = !self.setting_indent_guides;
            return None;
        }

        if id == FocusId::from_str("ext_rec.enable") {
            if let Some((ext_id, ext_name, _)) = self.recommended_extension.take() {
                let ws_root = self.workspace_root.clone();
                match self
                    .run_coord
                    .enable_and_activate_extension(&ext_id, ws_root.as_deref())
                {
                    Ok(Some(missing_tool)) => {
                        self.pending_tool_setup = Some(missing_tool);
                        self.active_modal = ActiveModal::ToolSetupPrompt;
                        self.toast = Some(ToastMessage {
                            title: format!("Enabled {ext_name}"),
                            body: "Extension enabled; configure its tool to continue.".to_owned(),
                            is_error: false,
                        });
                    }
                    Ok(None) => {
                        self.toast = Some(ToastMessage {
                            title: format!("Enabled {ext_name}"),
                            body: "Extension enabled and activated.".to_owned(),
                            is_error: false,
                        });
                    }
                    Err(err) => {
                        self.toast = Some(ToastMessage {
                            title: "Extension Error".to_owned(),
                            body: err,
                            is_error: true,
                        });
                    }
                }
            }
            return None;
        }
        if id == FocusId::from_str("ext_rec.dismiss") {
            if let Some((ext_id, _, _)) = self.recommended_extension.take() {
                self.dismissed_ext_recommendations.insert(ext_id);
            }
            return None;
        }

        // Check tabs
        for i in 0..self.tabs.len() {
            if id == FocusId::from_str(&format!("tab_close:{i}")) {
                self.close_tab(i);
                self.refresh_extension_recommendation();
                return None;
            }
            if id == FocusId::from_str(&format!("tab:{i}")) {
                self.active_tab = i;
                self.refresh_extension_recommendation();
                return None;
            }
        }

        // Check tree nodes / search / palette results
        let mut clicked_path: Option<(PathBuf, bool)> = None;
        for n in &self.tree_nodes {
            if id == FocusId::from_str(&format!("tree:{}", n.rel_path.display()))
                || id == FocusId::from_str(&format!("search:{}", n.rel_path.display()))
                || id == FocusId::from_str(&format!("pal_open:{}", n.rel_path.display()))
            {
                clicked_path = Some((n.rel_path.clone(), n.is_dir));
                break;
            }
        }
        if let Some((path, is_dir)) = clicked_path {
            self.selected_tree_path = Some(path.clone());
            if is_dir {
                if !self.expanded_dirs.remove(&path) {
                    self.expanded_dirs.insert(path);
                }
                self.rebuild_tree();
            } else {
                self.open_file(&path);
                self.active_modal = ActiveModal::None;
            }
            return None;
        }

        // Editor click
        let ed_body = Rect::new(
            self.last_editor_rect.x,
            self.last_editor_rect.y + self.theme.metrics.tabbar_height,
            self.last_editor_rect.width,
            (self.last_editor_rect.height - self.theme.metrics.tabbar_height).max(0.0),
        );
        if let Some(tab) = self.tabs.get_mut(self.active_tab)
            && id == tab.editor.id
        {
            tab.editor
                .click_at(ed_body, pos, self.theme.metrics.line_height_code);
        }
        None
    }

    fn on_scroll(&mut self, pos: Point, _dx: f32, dy: f32) {
        if self.last_tree_rect.contains(pos) {
            let max_scroll = (self.tree_nodes.len() as f32 * self.theme.metrics.row_height
                - self.last_tree_rect.height)
                .max(0.0);
            self.tree_scroll_y = (self.tree_scroll_y - dy).clamp(0.0, max_scroll);
        } else if self.last_editor_rect.contains(pos)
            && let Some(tab) = self.tabs.get_mut(self.active_tab)
        {
            let max_scroll = (tab.editor.lines.len() as f32 * self.theme.metrics.line_height_code
                - self.last_editor_rect.height
                + 60.0)
                .max(0.0);
            tab.editor.scroll_y = (tab.editor.scroll_y - dy).clamp(0.0, max_scroll);
        }
    }

    fn on_key(
        &mut self,
        key: &Key,
        text: Option<&str>,
        mods: ModifiersState,
        input: &mut InputState,
    ) -> bool {
        if matches!(key, Key::Named(NamedKey::Escape)) {
            self.active_modal = ActiveModal::None;
            self.run_coord.deny_pending();
            self.pending_dirty_identity = None;
            self.toast = None;
            return true;
        }
        if matches!(key, Key::Named(NamedKey::F5)) {
            if mods.shift_key() {
                self.run_coord.stop_active();
            } else if mods.control_key() {
                if let Some(last) = self.run_coord.last_run.clone() {
                    self.trigger_run_for_identity(last, None);
                }
            } else if let Some(sel) = self.run_coord.selected.clone() {
                self.trigger_run_for_identity(sel, None);
            }
            return true;
        }
        if mods.control_key()
            && let Key::Character(ch) = key
        {
            if ch.eq_ignore_ascii_case("k") {
                self.active_modal = ActiveModal::CommandPalette;
                input.focused = Some(FocusId::from_str("pal.input"));
                return true;
            }
            if ch == "," {
                self.active_modal = ActiveModal::Settings;
                return true;
            }
            if ch.eq_ignore_ascii_case("j") {
                self.bottom_visible = !self.bottom_visible;
                return true;
            }
            if ch.eq_ignore_ascii_case("s") {
                self.save_active_tab();
                return true;
            }
            if ch.eq_ignore_ascii_case("w") {
                self.close_tab(self.active_tab);
                return true;
            }
        }

        // Route typing to ToolManager manual path input if focused
        if self.active_modal == ActiveModal::ToolManager
            && input.focused == Some(FocusId::from_str("tm.manual_input"))
        {
            if matches!(key, Key::Named(NamedKey::Backspace)) {
                self.tool_manual_path.pop();
                return true;
            }
            if let Some(t) = text
                && !mods.control_key()
                && !t.chars().any(char::is_control)
            {
                self.tool_manual_path.push_str(t);
                return true;
            }
            return false;
        }

        // Route typing to Run Configuration draft inputs if focused
        if self.active_modal == ActiveModal::CreateRunConfig
            && let Some(ref mut draft) = self.run_draft
        {
            let target_field = if input.focused == Some(FocusId::from_str("rc.name")) {
                Some(&mut draft.name)
            } else if input.focused == Some(FocusId::from_str("rc.exec")) {
                Some(&mut draft.executable)
            } else if input.focused == Some(FocusId::from_str("rc.args")) {
                Some(&mut draft.args_simple)
            } else if input.focused == Some(FocusId::from_str("rc.cwd")) {
                Some(&mut draft.cwd)
            } else if input.focused == Some(FocusId::from_str("rc.langs")) {
                Some(&mut draft.languages_csv)
            } else if input.focused == Some(FocusId::from_str("rc.timeout")) {
                Some(&mut draft.timeout_ms_text)
            } else {
                None
            };
            if let Some(field) = target_field {
                if matches!(key, Key::Named(NamedKey::Backspace)) {
                    field.pop();
                    draft.error = None;
                    return true;
                }
                if let Some(t) = text
                    && !mods.control_key()
                    && !t.chars().any(char::is_control)
                {
                    field.push_str(t);
                    draft.error = None;
                    return true;
                }
                return false;
            }
        }

        // Route typing to active search/palette input if focused
        if input.focused == Some(FocusId::from_str("pal.input")) {
            if matches!(key, Key::Named(NamedKey::Backspace)) {
                self.palette_query.pop();
                return true;
            }
            if let Some(t) = text
                && !mods.control_key()
                && !t.chars().any(char::is_control)
            {
                self.palette_query.push_str(t);
                return true;
            }
            return false;
        }
        if input.focused == Some(FocusId::from_str("sb.search_input")) {
            if matches!(key, Key::Named(NamedKey::Backspace)) {
                self.search_query.pop();
                self.run_search();
                return true;
            }
            if let Some(t) = text
                && !mods.control_key()
                && !t.chars().any(char::is_control)
            {
                self.search_query.push_str(t);
                self.run_search();
                return true;
            }
            return false;
        }

        // Active code editor input
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if mods.control_key()
                && let Key::Character(ch) = key
            {
                if ch.eq_ignore_ascii_case("z") {
                    tab.dirty = tab.editor.undo() || tab.dirty;
                    return true;
                }
                if ch.eq_ignore_ascii_case("y") {
                    tab.dirty = tab.editor.redo() || tab.dirty;
                    return true;
                }
                if ch.eq_ignore_ascii_case("a") {
                    tab.editor.select_all();
                    return true;
                }
                if ch.eq_ignore_ascii_case("v")
                    && let Some(clip) = input.clipboard.get_text()
                {
                    tab.editor.insert_str(&clip);
                    tab.dirty = true;
                    return true;
                }
            }
            match key {
                Key::Named(NamedKey::ArrowUp) => {
                    tab.editor.move_cursor(-1, 0, mods.shift_key());
                    return true;
                }
                Key::Named(NamedKey::ArrowDown) => {
                    tab.editor.move_cursor(1, 0, mods.shift_key());
                    return true;
                }
                Key::Named(NamedKey::ArrowLeft) => {
                    tab.editor.move_cursor(0, -1, mods.shift_key());
                    return true;
                }
                Key::Named(NamedKey::ArrowRight) => {
                    tab.editor.move_cursor(0, 1, mods.shift_key());
                    return true;
                }
                Key::Named(NamedKey::Enter) => {
                    tab.editor.insert_newline();
                    tab.dirty = true;
                    return true;
                }
                Key::Named(NamedKey::Backspace) => {
                    if tab.editor.backspace() {
                        tab.dirty = true;
                    }
                    return true;
                }
                Key::Named(NamedKey::Delete) => {
                    if tab.editor.delete_forward() {
                        tab.dirty = true;
                    }
                    return true;
                }
                _ => {
                    if let Some(t) = text
                        && !mods.control_key()
                        && !mods.alt_key()
                        && !t.chars().any(char::is_control)
                    {
                        tab.editor.insert_str(t);
                        tab.dirty = true;
                        return true;
                    }
                }
            }
        }
        false
    }

    fn on_first_frame(&mut self) -> Option<WindowCommand> {
        let elapsed_ms = self.start_instant.elapsed().as_secs_f64() * 1000.0;
        if self.smoke_mode {
            println!("KOREN_NATIVE_FIRST_FRAME_MS={elapsed_ms:.2}");
            Some(WindowCommand::Close)
        } else {
            None
        }
    }
}

fn main() {
    let mut workspace_arg: Option<PathBuf> = None;
    let mut smoke_mode = false;
    for arg in std::env::args().skip(1) {
        if arg == "--smoke" {
            smoke_mode = true;
        } else if workspace_arg.is_none() {
            workspace_arg = Some(PathBuf::from(arg));
        }
    }
    let app = KorenNativeApp::new(workspace_arg, smoke_mode);
    if let Err(err) = run_app(app, false) {
        eprintln!("koren-native exited with error: {err}");
        std::process::exit(1);
    }
}
