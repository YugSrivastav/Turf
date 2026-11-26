//! Herdr-Grade Ratatui Spatial Multiplexer Cockpit for TurfCode CLI.
//!
//! Conforms to [MD-001], [MD-004], [MD-042], and [MD-044]:
//! - Borderless terminal panes blitting directly from `turf-vte::ScreenBuffer`
//! - Inverted high-contrast Electric Lime solid pill tab strip
//! - Borderless left sidebar with single vertical divider (`│`) and 4 sections
//! - Minimal bottom prompt line with subtle `─` divider and right-aligned shortcuts
//! - 60fps flicker-free frame blitting event loop

use std::io::{self, stdout, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::Backend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::Terminal as RatTerminal;
use tokio::sync::broadcast;
use turf_daemon::DaemonClient;
use turf_protocol::intent::{TaskCard, TaskStatus};
use turf_protocol::ipc::DaemonEvent;
use turf_protocol::locks::FileLock;

use crate::input::InputState;
use crate::keys::key_to_bytes;
use crate::layout::CockpitLayout;
use crate::pane::{Pane, PaneBackend, PaneManager, SplitDirection};
use crate::sidebar::{render_sidebar, SidebarState};
use crate::tabs::{
    compute_tab_strip_hits_scrolled, ensure_tab_visible, render_tab_strip_with_status, TabItem,
    TabStripHit,
};
use crate::theme::{
    COLOR_ACCENT, COLOR_ACTIVE_PILL, COLOR_CRUST, COLOR_HEADER_BG, COLOR_MUTED_BORDER,
    COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};

/// Focus areas for spatial navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Terminal,
    Sidebar,
    Input,
}

/// Descriptor for a multiplexer tab containing its own split pane manager.
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub pane_manager: PaneManager,
}

/// Interactive "new tab" / "rename tab" modal state triggered by `+` or tab context menu.
#[derive(Debug, Clone, Default)]
pub struct NewTabModalState {
    pub active: bool,
    pub rename_tab_index: Option<usize>,
    pub input: InputState,
}

/// Tab bar dropdown menu (`New tab`, `Rename`, `Close`) matching Herdr.
#[derive(Debug, Clone, Copy, Default)]
pub struct TabContextMenuState {
    pub active: bool,
    pub tab_index: usize,
    pub anchor_x: u16,
    pub anchor_y: u16,
    pub selected_item: usize,
}

/// Active mouse drag selection scoped strictly to a single pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneSelectionState {
    pub pane_id: usize,
    pub start: (u16, u16),
    pub end: (u16, u16),
    pub dragging: bool,
}

/// The master Ratatui spatial multiplexer cockpit.
pub struct TerminalCockpit {
    pub session_code: String,
    pub role: String,
    pub username: String,
    pub cwd: Option<String>,
    pub show_help: bool,
    pub sidebar_visible: bool,
    pub sidebar_state: SidebarState,
    pub tabs: Vec<Tab>,
    pub active_tab_index: usize,
    pub tab_scroll_offset: usize,
    pub focus: FocusArea,
    pub input_state: InputState,
    pub new_tab_modal: NewTabModalState,
    pub tab_menu: TabContextMenuState,
    pub selection: Option<PaneSelectionState>,
    pub last_area: Rect,
    pub status_toast: Option<(String, Instant)>,
    pub should_exit: bool,
    pub is_detached: bool,
    pub next_tab_id: usize,
    pub daemon_client: Option<DaemonClient>,
    pub daemon_event_rx: Option<broadcast::Receiver<DaemonEvent>>,
}

impl TerminalCockpit {
    /// Initialize a new TerminalCockpit with headless panes (for tests).
    pub fn new_headless(
        session_code: impl Into<String>,
        role: impl Into<String>,
        username: impl Into<String>,
        rows: usize,
        cols: usize,
    ) -> Self {
        let initial_pane = Pane::new_headless(1, "Shell 1", rows, cols);
        let tab = Tab {
            id: 1,
            title: "Shell".to_string(),
            pane_manager: PaneManager::new(initial_pane),
        };

        Self {
            session_code: session_code.into(),
            role: role.into(),
            username: username.into(),
            cwd: None,
            show_help: false,
            sidebar_visible: true,
            sidebar_state: SidebarState::default(),
            tabs: vec![tab],
            active_tab_index: 0,
            tab_scroll_offset: 0,
            focus: FocusArea::Terminal,
            input_state: InputState::default(),
            new_tab_modal: NewTabModalState::default(),
            tab_menu: TabContextMenuState::default(),
            selection: None,
            last_area: Rect::new(0, 0, cols as u16, rows as u16),
            status_toast: None,
            should_exit: false,
            is_detached: false,
            next_tab_id: 2,
            daemon_client: None,
            daemon_event_rx: None,
        }
    }

    /// Initialize a new TerminalCockpit with live in-process PTY shell panes.
    pub fn new_standalone(
        session_code: impl Into<String>,
        role: impl Into<String>,
        username: impl Into<String>,
        rows: u16,
        cols: u16,
    ) -> Result<Self, turf_pty::PtyError> {
        let initial_pane = Pane::spawn_shell(1, "Shell 1", rows, cols)?;
        let tab = Tab {
            id: 1,
            title: "Shell".to_string(),
            pane_manager: PaneManager::new(initial_pane),
        };

        Ok(Self {
            session_code: session_code.into(),
            role: role.into(),
            username: username.into(),
            cwd: None,
            show_help: false,
            sidebar_visible: true,
            sidebar_state: SidebarState::default(),
            tabs: vec![tab],
            active_tab_index: 0,
            tab_scroll_offset: 0,
            focus: FocusArea::Terminal,
            input_state: InputState::default(),
            new_tab_modal: NewTabModalState::default(),
            tab_menu: TabContextMenuState::default(),
            selection: None,
            last_area: Rect::new(0, 0, cols, rows),
            status_toast: None,
            should_exit: false,
            is_detached: false,
            next_tab_id: 2,
            daemon_client: None,
            daemon_event_rx: None,
        })
    }

    /// Initialize a new TerminalCockpit with live PTY shell panes (standalone alias).
    pub fn new_live(
        session_code: impl Into<String>,
        role: impl Into<String>,
        username: impl Into<String>,
        rows: u16,
        cols: u16,
    ) -> Result<Self, turf_pty::PtyError> {
        Self::new_standalone(session_code, role, username, rows, cols)
    }

    /// Initialize a new thin-client TerminalCockpit connected to the background daemon.
    pub async fn new_daemon(
        client: DaemonClient,
        session_code: impl Into<String>,
        role: impl Into<String>,
        username: impl Into<String>,
        rows: u16,
        cols: u16,
    ) -> anyhow::Result<Self> {
        Self::new_daemon_with_cwd(client, session_code, role, username, rows, cols, None).await
    }

    /// Initialize a new thin-client TerminalCockpit connected to the background daemon
    /// with an optional working directory for newly spawned panes.
    pub async fn new_daemon_with_cwd(
        client: DaemonClient,
        session_code: impl Into<String>,
        role: impl Into<String>,
        username: impl Into<String>,
        rows: u16,
        cols: u16,
        cwd: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let session_code_str = session_code.into();
        let cwd_str = cwd.map(|p| p.to_string_lossy().to_string());
        let existing_panes = client.attach_session(&session_code_str).await.unwrap_or_default();
        let event_rx = client.subscribe();

        let mut initial_panes = Vec::new();
        if !existing_panes.is_empty() {
            for (idx, &pane_id) in existing_panes.iter().enumerate() {
                let mut p = Pane::new_daemon(
                    idx + 1,
                    format!("Shell {}", idx + 1),
                    pane_id as u64,
                    client.clone(),
                    rows,
                    cols,
                );
                if let Ok(buffer_bytes) = client.read_pane(pane_id).await {
                    let text = String::from_utf8_lossy(&buffer_bytes);
                    for (y, line) in text.lines().enumerate() {
                        let trimmed = line.trim_end();
                        if !trimmed.is_empty() {
                            p.terminal.process_str(&format!("\x1b[{};1H{}", y + 1, trimmed));
                        }
                    }
                }
                initial_panes.push(p);
            }
        } else {
            let daemon_pane_id = client.spawn_pane(1, cwd_str.clone(), None).await?;
            let p = Pane::new_daemon(
                1,
                "Shell 1",
                daemon_pane_id as u64,
                client.clone(),
                rows,
                cols,
            );
            initial_panes.push(p);
        }

        let first_pane = initial_panes.remove(0);
        let mut pane_manager = PaneManager::new(first_pane);
        for remaining_pane in initial_panes {
            pane_manager.split_active_daemon(
                SplitDirection::Vertical,
                remaining_pane.daemon_pane_id().unwrap_or(1),
                client.clone(),
            );
        }

        let tab = Tab {
            id: 1,
            title: "Shell".to_string(),
            pane_manager,
        };

        Ok(Self {
            session_code: session_code_str,
            role: role.into(),
            username: username.into(),
            cwd: cwd_str,
            show_help: false,
            sidebar_visible: true,
            sidebar_state: SidebarState::default(),
            tabs: vec![tab],
            active_tab_index: 0,
            tab_scroll_offset: 0,
            focus: FocusArea::Terminal,
            input_state: InputState::default(),
            new_tab_modal: NewTabModalState::default(),
            tab_menu: TabContextMenuState::default(),
            selection: None,
            last_area: Rect::new(0, 0, cols, rows),
            status_toast: None,
            should_exit: false,
            is_detached: false,
            next_tab_id: 2,
            daemon_client: Some(client),
            daemon_event_rx: Some(event_rx),
        })
    }

    /// Focus cycling between Sidebar -> Terminal -> Input -> Sidebar.
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusArea::Terminal => FocusArea::Input,
            FocusArea::Input => {
                if self.sidebar_visible {
                    FocusArea::Sidebar
                } else {
                    FocusArea::Terminal
                }
            }
            FocusArea::Sidebar => FocusArea::Terminal,
        };
    }

    /// Set explicit focus area.
    pub fn set_focus(&mut self, focus: FocusArea) {
        self.focus = focus;
    }

    /// Toggle left sidebar visibility ([MD-032] Ctrl+B).
    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        if !self.sidebar_visible && self.focus == FocusArea::Sidebar {
            self.focus = FocusArea::Terminal;
        }
    }

    /// Add a new tab backed by a headless terminal pane.
    pub fn add_tab_headless(&mut self, title: impl Into<String>) -> usize {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let title_str = title.into();

        let initial_pane = Pane::new_headless(1, format!("{title_str} 1"), 24, 80);
        let tab = Tab {
            id: tab_id,
            title: title_str,
            pane_manager: PaneManager::new(initial_pane),
        };
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        tab_id
    }

    /// Add a new tab backed by a live PTY shell.
    pub fn add_tab_shell(&mut self, title: impl Into<String>) -> Result<usize, turf_pty::PtyError> {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;
        let title_str = title.into();

        let initial_pane = Pane::spawn_shell(1, format!("{title_str} 1"), 24, 80)?;
        let tab = Tab {
            id: tab_id,
            title: title_str,
            pane_manager: PaneManager::new(initial_pane),
        };
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        Ok(tab_id)
    }

    /// Switch to tab by index (0-based).
    pub fn switch_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab_index = index;
            true
        } else {
            false
        }
    }

    /// Switch to next tab.
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
        }
    }

    /// Switch to previous tab.
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
        }
    }

    /// Close a specific tab by index (no-op if it is the only remaining tab).
    pub fn close_tab_at(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        let removed_tab = self.tabs.remove(index);
        for pane in removed_tab.pane_manager.all_panes() {
            if let PaneBackend::Daemon { pane_id, client } = &pane.backend {
                let pid = *pane_id as usize;
                let client = client.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let _ = client.close_pane(pid).await;
                    });
                }
            }
        }
        if self.active_tab_index > index {
            self.active_tab_index -= 1;
        } else if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        self.tab_scroll_offset = self.tab_scroll_offset.min(self.tabs.len().saturating_sub(1));
        self.sync_tab_scroll();
        true
    }

    /// Close active tab (unless it is the only remaining tab).
    pub fn close_active_tab(&mut self) -> bool {
        self.close_tab_at(self.active_tab_index)
    }

    fn sync_tab_scroll(&mut self) {
        let layout = CockpitLayout::calculate(self.last_area, self.sidebar_visible);
        if layout.tab_strip_area.width == 0 {
            return;
        }
        let tab_items = self.build_tab_items();
        let status_hint = self.status_toast.as_ref().map(|(s, _)| s.as_str());
        self.tab_scroll_offset = ensure_tab_visible(
            layout.tab_strip_area,
            &tab_items,
            self.active_tab_index,
            self.tab_scroll_offset,
            &self.session_code,
            &self.role,
            status_hint,
        );
    }

    /// Split active pane in the current tab vertically or horizontally (live shell).
    pub fn split_active_pane(&mut self, direction: SplitDirection) -> Result<usize, turf_pty::PtyError> {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.pane_manager.split_active_shell(direction)
        } else {
            Err(turf_pty::PtyError::Allocation("No active tab".to_string()))
        }
    }

    /// Split active pane in the current tab (headless).
    pub fn split_active_pane_headless(&mut self, direction: SplitDirection) -> usize {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.pane_manager.split_active_headless(direction)
        } else {
            0
        }
    }

    /// Close active pane in current tab.
    pub fn close_active_pane(&mut self) -> bool {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.pane_manager.close_active()
        } else {
            false
        }
    }

    /// Cycle focus between split panes in the current tab.
    pub fn cycle_pane_focus(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.pane_manager.focus_next();
        }
    }

    /// Poll all PTY readers across all tabs.
    pub fn poll_ptys(&mut self) -> usize {
        let mut total = 0;
        for tab in &mut self.tabs {
            total += tab.pane_manager.poll_all_ptys();
        }
        total
    }

    /// Poll and drain asynchronous broadcast events from the daemon.
    pub fn poll_daemon_events(&mut self) -> usize {
        let mut events = Vec::new();
        if let Some(rx) = &mut self.daemon_event_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }

        let mut total_bytes = 0;
        for event in events {
            match event {
                DaemonEvent::PaneOutput { pane_id, bytes } => {
                    total_bytes += bytes.len();
                    self.feed_pane_output(pane_id, &bytes);
                }
                DaemonEvent::PaneStatusChanged { pane_id, foreground_cmd } => {
                    for tab in &mut self.tabs {
                        for pane in tab.pane_manager.all_panes_mut() {
                            let matches = match &pane.backend {
                                PaneBackend::Daemon { pane_id: pid, .. } => *pid as usize == pane_id,
                                _ => pane.id == pane_id,
                            };
                            if matches {
                                pane.foreground_cmd = foreground_cmd.clone();
                                if let Some(cmd) = &foreground_cmd {
                                    pane.title = cmd.clone();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        total_bytes
    }

    /// Feed incoming streaming bytes directly into the target pane's VTE parser.
    pub fn feed_pane_output(&mut self, target_pane_id: usize, bytes: &[u8]) {
        for tab in &mut self.tabs {
            for pane in tab.pane_manager.all_panes_mut() {
                let matches = match &pane.backend {
                    PaneBackend::Daemon { pane_id, .. } => *pane_id as usize == target_pane_id,
                    _ => pane.id == target_pane_id,
                };
                if matches {
                    pane.terminal.process(bytes);
                    return;
                }
            }
        }
    }

    /// Gracefully detach from the daemon session without killing child processes or stopping the daemon.
    pub fn detach(&mut self) {
        self.is_detached = true;
        self.should_exit = true;
        if let Some(client) = &self.daemon_client {
            let client = client.clone();
            let session_id = self.session_code.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let _ = client.detach_session(&session_id).await;
                });
            }
        }
    }

    /// Refresh process titles across all tabs and panes.
    pub fn refresh_process_titles(&mut self) {
        for tab in &mut self.tabs {
            tab.pane_manager.refresh_process_titles();
        }
    }

    /// Add a tab automatically using daemon if available, live shell, or headless fallback.
    pub fn add_tab_auto(&mut self, title: &str) -> usize {
        if let Some(client) = self.daemon_client.clone() {
            let tab_id = self.next_tab_id;
            self.next_tab_id += 1;
            let title_str = title.to_string();

            let pid_opt = if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let client_c = client.clone();
                let cwd_c = self.cwd.clone();
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        client_c.spawn_pane(tab_id, cwd_c, None).await.ok()
                    })
                })
            } else {
                None
            };

            let daemon_pid = pid_opt.unwrap_or(tab_id) as u64;
            let initial_pane = Pane::new_daemon(
                1,
                format!("{title_str} 1"),
                daemon_pid,
                client,
                24,
                80,
            );
            let tab = Tab {
                id: tab_id,
                title: title_str,
                pane_manager: PaneManager::new(initial_pane),
            };
            self.tabs.push(tab);
            self.active_tab_index = self.tabs.len() - 1;
            return tab_id;
        }

        let is_headless = self
            .tabs
            .first()
            .and_then(|t| t.pane_manager.active_pane())
            .is_none_or(|p| p.is_headless());

        if is_headless {
            self.add_tab_headless(title)
        } else {
            self.add_tab_shell(title).unwrap_or_else(|_| self.add_tab_headless(title))
        }
    }

    /// Split active pane automatically using daemon if available, live shell, or headless fallback.
    pub fn split_active_pane_auto(&mut self, direction: SplitDirection) -> usize {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            if let Some(client) = self.daemon_client.clone() {
                let active_pane = tab.pane_manager.active_pane();
                let parent_daemon_pid = active_pane.and_then(|p| p.daemon_pane_id()).unwrap_or(1) as usize;
                let dir_str = match direction {
                    SplitDirection::Vertical => "vertical",
                    SplitDirection::Horizontal => "horizontal",
                };

                let pid_opt = if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let client_c = client.clone();
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            client_c.split_pane(parent_daemon_pid, dir_str).await.ok()
                        })
                    })
                } else {
                    None
                };

                let new_daemon_pid = pid_opt.unwrap_or(parent_daemon_pid + 1) as u64;
                return tab.pane_manager.split_active_daemon(direction, new_daemon_pid, client);
            }

            let is_headless = tab.pane_manager.active_pane().is_none_or(|p| p.is_headless());
            if is_headless {
                tab.pane_manager.split_active_headless(direction)
            } else {
                tab.pane_manager.split_active_shell(direction).unwrap_or_else(|_| {
                    tab.pane_manager.split_active_headless(direction)
                })
            }
        } else {
            0
        }
    }

    /// Execute a built-in `/slash` command from the bottom input prompt.
    pub fn execute_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        match parts[0] {
            "/quit" | "/exit" | "/q" => {
                self.should_exit = true;
            }
            "/split" | "/split-v" | "/vsplit" => {
                self.split_active_pane_auto(SplitDirection::Vertical);
                self.focus = FocusArea::Terminal;
            }
            "/split-h" | "/hsplit" => {
                self.split_active_pane_auto(SplitDirection::Horizontal);
                self.focus = FocusArea::Terminal;
            }
            "/close" | "/pane-close" => {
                self.close_active_pane();
                self.focus = FocusArea::Terminal;
            }
            "/next" | "/pane-next" => {
                self.cycle_pane_focus();
                self.focus = FocusArea::Terminal;
            }
            "/tab" | "/new" => {
                let title = parts.get(1).copied().unwrap_or("Shell");
                self.add_tab_auto(title);
                self.focus = FocusArea::Terminal;
            }
            "/close-tab" => {
                self.close_active_tab();
                self.focus = FocusArea::Terminal;
            }
            "/task" => {
                if parts.len() > 1 {
                    let title = parts[1..].join(" ");
                    let task = TaskCard::new(
                        format!("task-{}", self.sidebar_state.tasks.len() + 1),
                        title,
                        &self.username,
                        TaskStatus::InProgress,
                        vec![],
                    );
                    self.sidebar_state.tasks.push(task);
                }
            }
            "/lock" => {
                if parts.len() > 1 {
                    let file_path = parts[1];
                    if !self
                        .sidebar_state
                        .locks
                        .iter()
                        .any(|l| l.file_path == file_path)
                    {
                        let lock = FileLock::new(
                            file_path,
                            &self.username,
                            "task-1",
                            30_000,
                            None,
                        );
                        self.sidebar_state.locks.push(lock);
                    }
                }
            }
            "/unlock" => {
                if parts.len() > 1 {
                    let file_path = parts[1];
                    self.sidebar_state.locks.retain(|l| l.file_path != file_path);
                }
            }
            "/sidebar" => {
                self.toggle_sidebar();
            }
            "/detach" => {
                self.detach();
            }
            "/help" | "/?" => {
                self.show_help = true;
            }
            _ => {}
        }
    }

    /// Open the Herdr-style centered "new tab" modal pre-filled with the next tab number.
    pub fn open_new_tab_modal(&mut self) {
        let default_label = self.next_tab_id.to_string();
        self.new_tab_modal.active = true;
        self.new_tab_modal.rename_tab_index = None;
        self.new_tab_modal.input.cursor_pos = default_label.chars().count();
        self.new_tab_modal.input.value = default_label;
        self.tab_menu.active = false;
        self.show_help = false;
    }

    /// Open the centered "rename tab" modal pre-filled with the target tab's label.
    pub fn open_rename_tab_modal(&mut self, tab_idx: usize) {
        if let Some(tab) = self.tabs.get(tab_idx) {
            let current = if tab.title.trim().is_empty() || tab.title.eq_ignore_ascii_case("shell") {
                tab.id.to_string()
            } else {
                tab.title.clone()
            };
            self.new_tab_modal.active = true;
            self.new_tab_modal.rename_tab_index = Some(tab_idx);
            self.new_tab_modal.input.cursor_pos = current.chars().count();
            self.new_tab_modal.input.value = current;
            self.tab_menu.active = false;
            self.show_help = false;
        }
    }

    fn submit_new_tab_modal(&mut self) {
        let trimmed = self.new_tab_modal.input.value.trim().to_string();
        let rename_idx = self.new_tab_modal.rename_tab_index.take();
        self.new_tab_modal.active = false;
        self.new_tab_modal.input.value.clear();
        self.new_tab_modal.input.cursor_pos = 0;

        if let Some(idx) = rename_idx {
            if let Some(tab) = self.tabs.get_mut(idx) {
                tab.title = if trimmed.is_empty() {
                    tab.id.to_string()
                } else {
                    trimmed
                };
            }
        } else {
            let title = if trimmed.is_empty() {
                self.next_tab_id.to_string()
            } else {
                trimmed
            };
            self.add_tab_auto(&title);
            self.sync_tab_scroll();
        }
        self.focus = FocusArea::Terminal;
    }

    fn execute_tab_menu_action(&mut self, action_idx: usize) {
        let target_idx = self.tab_menu.tab_index;
        self.tab_menu.active = false;
        match action_idx {
            0 => self.open_new_tab_modal(),
            1 => self.open_rename_tab_modal(target_idx),
            2 => {
                self.close_tab_at(target_idx);
            }
            _ => {}
        }
    }

    /// Process a Crossterm mouse event (tab clicks, tab context menu, scrollable tab strip, `+` new tab modal, pane focus, bounded copy selection, OSC 8 links, wheel scroll).
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> bool {
        let col = mouse.column;
        let row = mouse.row;

        // 1. Handle clicks when centered modals are active
        if self.new_tab_modal.active {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let geom = new_tab_modal_geometry(area);
                if rect_contains(geom.save_btn, col, row) {
                    self.submit_new_tab_modal();
                    return true;
                }
                if rect_contains(geom.clear_btn, col, row) {
                    self.new_tab_modal.input.value.clear();
                    self.new_tab_modal.input.cursor_pos = 0;
                    return true;
                }
                if rect_contains(geom.cancel_btn, col, row) || !rect_contains(geom.modal, col, row) {
                    self.new_tab_modal.active = false;
                    return true;
                }
            }
            return true;
        }

        if self.show_help {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.show_help = false;
            }
            return true;
        }

        // 2. Handle active tab context dropdown menu (`New tab`, `Rename`, `Close`)
        if self.tab_menu.active {
            let menu_rect = tab_context_menu_rect(self.tab_menu.anchor_x, self.tab_menu.anchor_y, area);
            match mouse.kind {
                MouseEventKind::Moved => {
                    if rect_contains(menu_rect, col, row)
                        && row > menu_rect.y
                        && row < menu_rect.y + menu_rect.height.saturating_sub(1)
                    {
                        self.tab_menu.selected_item = (row - menu_rect.y - 1) as usize;
                    }
                    return true;
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if rect_contains(menu_rect, col, row)
                        && row > menu_rect.y
                        && row < menu_rect.y + menu_rect.height.saturating_sub(1)
                    {
                        let action = (row - menu_rect.y - 1) as usize;
                        self.execute_tab_menu_action(action);
                        return true;
                    }
                    self.tab_menu.active = false;
                    // Fall through if user clicked directly on tab strip
                }
                _ => {}
            }
        }

        let layout = CockpitLayout::calculate(area, self.sidebar_visible);
        let status_hint = self.status_toast.as_ref().map(|(s, _)| s.as_str()).map(str::to_string);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Right) => {
                if rect_contains(layout.tab_strip_area, col, row) {
                    let tab_items = self.build_tab_items();
                    for (rect, hit) in compute_tab_strip_hits_scrolled(
                        layout.tab_strip_area,
                        &tab_items,
                        self.tab_scroll_offset,
                        &self.session_code,
                        &self.role,
                        status_hint.as_deref(),
                    ) {
                        if rect_contains(rect, col, row) {
                            if let TabStripHit::Tab(idx) = hit {
                                self.selection = None;
                                self.tab_menu = TabContextMenuState {
                                    active: true,
                                    tab_index: idx,
                                    anchor_x: rect.x,
                                    anchor_y: layout.tab_strip_area.y + 1,
                                    selected_item: 0,
                                };
                                return true;
                            }
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Tab strip hits (tabs, `<`, `>`, & `+` button)
                if rect_contains(layout.tab_strip_area, col, row) {
                    let tab_items = self.build_tab_items();
                    for (rect, hit) in compute_tab_strip_hits_scrolled(
                        layout.tab_strip_area,
                        &tab_items,
                        self.tab_scroll_offset,
                        &self.session_code,
                        &self.role,
                        status_hint.as_deref(),
                    ) {
                        if rect_contains(rect, col, row) {
                            match hit {
                                TabStripHit::ScrollLeft => {
                                    self.tab_scroll_offset = self.tab_scroll_offset.saturating_sub(1);
                                }
                                TabStripHit::ScrollRight => {
                                    if self.tab_scroll_offset + 1 < self.tabs.len() {
                                        self.tab_scroll_offset += 1;
                                    }
                                }
                                TabStripHit::Tab(idx) => {
                                    self.selection = None;
                                    if idx == self.active_tab_index {
                                        // Clicking active tab toggles Herdr tab context menu (`New tab`, `Rename`, `Close`)
                                        self.tab_menu = TabContextMenuState {
                                            active: true,
                                            tab_index: idx,
                                            anchor_x: rect.x,
                                            anchor_y: layout.tab_strip_area.y + 1,
                                            selected_item: 0,
                                        };
                                    } else {
                                        self.tab_menu.active = false;
                                        self.switch_tab(idx);
                                        self.sync_tab_scroll();
                                    }
                                    self.focus = FocusArea::Terminal;
                                }
                                TabStripHit::AddButton => {
                                    self.selection = None;
                                    self.open_new_tab_modal();
                                }
                            }
                            return true;
                        }
                    }
                    return true;
                }

                // Terminal pane hit -> focus pane + begin pane-bounded selection anchor
                if rect_contains(layout.terminal_area, col, row) {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        let pane_rects = tab.pane_manager.layout_rects(layout.terminal_area);
                        if let Some((pane_id, rect)) =
                            pane_rects.into_iter().find(|(_, r)| rect_contains(*r, col, row))
                        {
                            tab.pane_manager.set_active_pane(pane_id);
                            self.focus = FocusArea::Terminal;
                            let rel = (col - rect.x, row - rect.y);
                            self.selection = Some(PaneSelectionState {
                                pane_id,
                                start: rel,
                                end: rel,
                                dragging: true,
                            });
                            return true;
                        }
                    }
                }

                // Sidebar click toggles focus to sidebar (or clicks `? help` footer)
                if layout.sidebar_visible && rect_contains(layout.sidebar_area, col, row) {
                    if let Some(sections) = layout.calculate_sidebar_sections() {
                        let p = sections.peers_area;
                        if p.height >= 3 && row == p.y + p.height.saturating_sub(1) {
                            self.show_help = true;
                            return true;
                        }
                    }
                    self.selection = None;
                    self.focus = FocusArea::Sidebar;
                    return true;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(sel) = &mut self.selection {
                    if sel.dragging {
                        if let Some(tab) = self.tabs.get(self.active_tab_index) {
                            let pane_rects = tab.pane_manager.layout_rects(layout.terminal_area);
                            if let Some((_, rect)) =
                                pane_rects.into_iter().find(|(id, _)| *id == sel.pane_id)
                            {
                                if rect.width > 0 && rect.height > 0 {
                                    // Strictly clamp drag coordinates inside this single pane
                                    let clamped_x = col.clamp(rect.x, rect.x + rect.width - 1);
                                    let clamped_y = row.clamp(rect.y, rect.y + rect.height - 1);
                                    sel.end = (clamped_x - rect.x, clamped_y - rect.y);
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some(mut sel) = self.selection.take() {
                    if sel.dragging {
                        sel.dragging = false;
                        if sel.start != sel.end {
                            let copied = self
                                .tabs
                                .get(self.active_tab_index)
                                .and_then(|t| t.pane_manager.get_pane(sel.pane_id))
                                .map(|p| p.extract_selected_text((sel.start, sel.end)))
                                .unwrap_or_default();
                            self.selection = Some(sel);
                            if !copied.is_empty() {
                                copy_to_clipboard(&copied);
                                self.status_toast = Some((
                                    format!("copied {} chars", copied.chars().count()),
                                    Instant::now(),
                                ));
                            }
                            return true;
                        } else {
                            // Single click without drag -> open OSC 8 / wrapped URL if present
                            let link = self
                                .tabs
                                .get(self.active_tab_index)
                                .and_then(|t| t.pane_manager.get_pane(sel.pane_id))
                                .and_then(|p| p.link_at(sel.start.0, sel.start.1));
                            if let Some(url) = link {
                                open_external_url(&url);
                                self.status_toast =
                                    Some(("opened link".to_string(), Instant::now()));
                            }
                            return true;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(layout.tab_strip_area, col, row) {
                    self.tab_scroll_offset = self.tab_scroll_offset.saturating_sub(1);
                    return true;
                }
                if rect_contains(layout.terminal_area, col, row) {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        let pane_rects = tab.pane_manager.layout_rects(layout.terminal_area);
                        if let Some((pane_id, _)) =
                            pane_rects.into_iter().find(|(_, r)| rect_contains(*r, col, row))
                        {
                            if let Some(pane) = tab.pane_manager.get_pane_mut(pane_id) {
                                if !pane.terminal.is_alt_buffer() {
                                    pane.terminal.scroll_up_viewport(3);
                                }
                                return true;
                            }
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if rect_contains(layout.tab_strip_area, col, row) {
                    if self.tab_scroll_offset + 1 < self.tabs.len() {
                        self.tab_scroll_offset += 1;
                    }
                    return true;
                }
                if rect_contains(layout.terminal_area, col, row) {
                    if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                        let pane_rects = tab.pane_manager.layout_rects(layout.terminal_area);
                        if let Some((pane_id, _)) =
                            pane_rects.into_iter().find(|(_, r)| rect_contains(*r, col, row))
                        {
                            if let Some(pane) = tab.pane_manager.get_pane_mut(pane_id) {
                                if !pane.terminal.is_alt_buffer() {
                                    pane.terminal.scroll_down_viewport(3);
                                }
                                return true;
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        false
    }

    /// Process a Crossterm keyboard event.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        // --- 0. Help overlay dismissal ---
        if self.show_help {
            if matches!(
                key.code,
                KeyCode::Esc
                    | KeyCode::Enter
                    | KeyCode::F(1)
                    | KeyCode::Char('?')
                    | KeyCode::Char('q')
                    | KeyCode::Char('Q')
            ) {
                self.show_help = false;
                return true;
            }
            return true;
        }

        // --- 0b. Centered New Tab modal (matching Herdr) ---
        if self.new_tab_modal.active {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(
                    key.code,
                    KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('u') | KeyCode::Char('U')
                )
            {
                self.new_tab_modal.input.value.clear();
                self.new_tab_modal.input.cursor_pos = 0;
                return true;
            }
            match key.code {
                KeyCode::Enter => {
                    self.submit_new_tab_modal();
                    return true;
                }
                KeyCode::Esc => {
                    self.new_tab_modal.active = false;
                    return true;
                }
                KeyCode::Backspace => {
                    self.new_tab_modal.input.backspace();
                    return true;
                }
                KeyCode::Delete => {
                    self.new_tab_modal.input.delete();
                    return true;
                }
                KeyCode::Left => {
                    self.new_tab_modal.input.move_cursor_left();
                    return true;
                }
                KeyCode::Right => {
                    self.new_tab_modal.input.move_cursor_right();
                    return true;
                }
                KeyCode::Home => {
                    self.new_tab_modal.input.move_cursor_home();
                    return true;
                }
                KeyCode::End => {
                    self.new_tab_modal.input.move_cursor_end();
                    return true;
                }
                KeyCode::Char(c) => {
                    self.new_tab_modal.input.insert_char(c);
                    return true;
                }
                _ => return true,
            }
        }

        // --- 0c. Tab context menu keyboard navigation ---
        if self.tab_menu.active {
            match key.code {
                KeyCode::Esc => {
                    self.tab_menu.active = false;
                    return true;
                }
                KeyCode::Up => {
                    self.tab_menu.selected_item = if self.tab_menu.selected_item == 0 {
                        2
                    } else {
                        self.tab_menu.selected_item - 1
                    };
                    return true;
                }
                KeyCode::Down => {
                    self.tab_menu.selected_item = (self.tab_menu.selected_item + 1) % 3;
                    return true;
                }
                KeyCode::Enter => {
                    self.execute_tab_menu_action(self.tab_menu.selected_item);
                    return true;
                }
                _ => {
                    self.tab_menu.active = false;
                }
            }
        }

        // --- 1. Exit shortcut (Ctrl+Q or Alt+Q) ---
        if (key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT))
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        {
            self.should_exit = true;
            return true;
        }

        // --- 1b. Help overlay toggle (F1, Alt+?, or ? in Input/Sidebar) ---
        if key.code == KeyCode::F(1)
            || (key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('?'))
            || (key.code == KeyCode::Char('?')
                && (self.focus == FocusArea::Sidebar
                    || (self.focus == FocusArea::Input && self.input_state.value.is_empty())))
        {
            self.show_help = true;
            return true;
        }

        // --- 2. Detach shortcut (Alt+D or F4) ---
        if key.code == KeyCode::F(4)
            || (key.modifiers.contains(KeyModifiers::ALT)
                && matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D')))
        {
            self.detach();
            return true;
        }

        // --- 3. Toggle prompt focus (F2, Alt+P, or Alt+Enter) ---
        if key.code == KeyCode::F(2)
            || (key.modifiers.contains(KeyModifiers::ALT)
                && (key.code == KeyCode::Enter || matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))))
        {
            self.focus = if self.focus == FocusArea::Input {
                FocusArea::Terminal
            } else {
                FocusArea::Input
            };
            return true;
        }

        // --- 3b. F3 cycles split panes (safe on all keyboard layouts) ---
        if key.code == KeyCode::F(3) {
            self.cycle_pane_focus();
            return true;
        }

        // --- 3. Viewport Scrolling Hotkeys ---
        // Shift+PageUp, Alt+Up, or Alt+[: scroll up by 10 lines
        // Shift+PageDown or Alt+Down: scroll down toward live stream
        // Note: bare PageUp/PageDown pass directly through to vim/less/htop inside the PTY.
        let is_scroll_up = (key.modifiers.contains(KeyModifiers::ALT) && (key.code == KeyCode::Up || key.code == KeyCode::Char('[')))
            || (key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::PageUp);
        let is_scroll_down = (key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Down)
            || (key.modifiers.contains(KeyModifiers::SHIFT) && key.code == KeyCode::PageDown);

        if is_scroll_up {
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(pane) = tab.pane_manager.active_pane_mut() {
                    pane.terminal.scroll_up_viewport(10);
                    return true;
                }
            }
        }

        if is_scroll_down {
            if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                if let Some(pane) = tab.pane_manager.active_pane_mut() {
                    pane.terminal.scroll_down_viewport(10);
                    return true;
                }
            }
        }

        // --- 4. Non-conflicting Alt+Key Multiplexer Hotkeys ---
        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    self.add_tab_auto("Shell");
                    self.sync_tab_scroll();
                    return true;
                }
                KeyCode::Char('w') | KeyCode::Char('W') => {
                    self.close_active_tab();
                    return true;
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    self.toggle_sidebar();
                    return true;
                }
                KeyCode::Char('v') | KeyCode::Char('V') => {
                    self.split_active_pane_auto(SplitDirection::Vertical);
                    return true;
                }
                KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('h') | KeyCode::Char('H') => {
                    self.split_active_pane_auto(SplitDirection::Horizontal);
                    return true;
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    self.cycle_pane_focus();
                    return true;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    self.close_active_pane();
                    return true;
                }
                KeyCode::Char(c @ '1'..='9') => {
                    let idx = (c as usize) - ('1' as usize);
                    self.switch_tab(idx);
                    return true;
                }
                _ => {}
            }
        }

        // --- 4. Tab cycles focus ONLY when NOT in Terminal ---
        if self.focus != FocusArea::Terminal
            && key.code == KeyCode::Tab
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.cycle_focus();
            return true;
        }

        // --- 6. Focus-Specific Key Handling ---
        match self.focus {
            FocusArea::Terminal => {
                // Ctrl+Shift+C copies active pane selection if present
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
                {
                    if let Some(sel) = self.selection {
                        let copied = self
                            .tabs
                            .get(self.active_tab_index)
                            .and_then(|t| t.pane_manager.get_pane(sel.pane_id))
                            .map(|p| p.extract_selected_text((sel.start, sel.end)))
                            .unwrap_or_default();
                        if !copied.is_empty() {
                            copy_to_clipboard(&copied);
                            self.status_toast = Some((
                                format!("copied {} chars", copied.chars().count()),
                                Instant::now(),
                            ));
                        }
                        return true;
                    }
                }

                // Clear active mouse selection highlight when typing into PTY
                if self.selection.is_some() {
                    self.selection = None;
                }

                // Keystrokes in terminal focus forward directly to the active PTY master
                if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
                    if let Some(pane) = tab.pane_manager.active_pane_mut() {
                        // Esc immediately snaps scroll_offset back to 0 if scrolled up
                        if key.code == KeyCode::Esc && pane.terminal.scroll_offset() > 0 {
                            pane.terminal.reset_viewport_scroll();
                            return true;
                        }

                        if let Some(bytes) = key_to_bytes(&key) {
                            // On typing user input, immediately snap scroll_offset back to live view
                            pane.terminal.reset_viewport_scroll();
                            pane.write_pty(&bytes);
                            return true;
                        }
                    }
                }
            }
            FocusArea::Input => {
                match key.code {
                    KeyCode::Enter => {
                        if let Some(cmd) = self.input_state.submit() {
                            self.execute_command(&cmd);
                        }
                        if self.focus == FocusArea::Input {
                            self.focus = FocusArea::Terminal;
                        }
                        return true;
                    }
                    KeyCode::Esc => {
                        self.focus = FocusArea::Terminal;
                        return true;
                    }
                    KeyCode::Backspace => {
                        self.input_state.backspace();
                        return true;
                    }
                    KeyCode::Delete => {
                        self.input_state.delete();
                        return true;
                    }
                    KeyCode::Left => {
                        self.input_state.move_cursor_left();
                        return true;
                    }
                    KeyCode::Right => {
                        self.input_state.move_cursor_right();
                        return true;
                    }
                    KeyCode::Home => {
                        self.input_state.move_cursor_home();
                        return true;
                    }
                    KeyCode::End => {
                        self.input_state.move_cursor_end();
                        return true;
                    }
                    KeyCode::Up => {
                        self.input_state.history_prev();
                        return true;
                    }
                    KeyCode::Down => {
                        self.input_state.history_next();
                        return true;
                    }
                    KeyCode::Char(c) => {
                        // Filter Latin-1 mouse escape leak artifacts as per [MD-044]
                        if (c as u32) >= 0x80 && (c as u32) <= 0xFF {
                            return true;
                        }
                        self.input_state.insert_char(c);
                        return true;
                    }
                    _ => {}
                }
            }
            FocusArea::Sidebar => {
                match key.code {
                    KeyCode::Esc => {
                        self.focus = FocusArea::Terminal;
                        return true;
                    }
                    KeyCode::Up => {
                        if self.sidebar_state.selected_task_index > 0 {
                            self.sidebar_state.selected_task_index -= 1;
                        }
                        return true;
                    }
                    KeyCode::Down => {
                        if !self.sidebar_state.tasks.is_empty()
                            && self.sidebar_state.selected_task_index + 1
                                < self.sidebar_state.tasks.len()
                        {
                            self.sidebar_state.selected_task_index += 1;
                        }
                        return true;
                    }
                    _ => {}
                }
            }
        }

        false
    }

    fn build_tab_items(&self) -> Vec<TabItem> {
        self.tabs
            .iter()
            .map(|t| {
                let badge = t.pane_manager.active_pane().and_then(|pane| {
                    let offset = pane.terminal.scroll_offset();
                    if offset > 0 {
                        Some(format!("[+{} lines]", offset))
                    } else {
                        None
                    }
                });
                TabItem::new(t.id, t.title.clone()).with_scroll_badge(badge)
            })
            .collect()
    }

    /// Render the full spatial multiplexer cockpit into Ratatui buffer.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.last_area = area;
        if let Some((_, ts)) = &self.status_toast {
            if ts.elapsed() > Duration::from_secs(2) {
                self.status_toast = None;
            }
        }
        let layout = CockpitLayout::calculate(area, self.sidebar_visible);

        // 1. Render Left Sidebar if visible
        if layout.sidebar_visible {
            if let Some(sections) = layout.calculate_sidebar_sections() {
                let is_sidebar_focused = self.focus == FocusArea::Sidebar;
                render_sidebar(
                    &sections,
                    layout.sidebar_divider,
                    &self.sidebar_state,
                    buf,
                    is_sidebar_focused,
                );
            }
        }

        // 2. Render Tab Strip (Top Row - Herdr scrollable tabs + pinned `+`)
        let tab_items = self.build_tab_items();
        let status_hint = self.status_toast.as_ref().map(|(s, _)| s.as_str());
        self.tab_scroll_offset = ensure_tab_visible(
            layout.tab_strip_area,
            &tab_items,
            self.active_tab_index,
            self.tab_scroll_offset,
            &self.session_code,
            &self.role,
            status_hint,
        );

        render_tab_strip_with_status(
            layout.tab_strip_area,
            &tab_items,
            self.active_tab_index,
            self.tab_scroll_offset,
            &self.session_code,
            &self.role,
            status_hint,
            buf,
        );

        // 3. Render Terminal Surface (borderless split panes, full height down to bottom)
        let active_selection = self.selection.map(|s| (s.pane_id, (s.start, s.end)));
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            let is_term_focused = self.focus == FocusArea::Terminal
                && !self.new_tab_modal.active
                && !self.tab_menu.active
                && !self.show_help;
            tab.pane_manager.render_with_selection(
                layout.terminal_area,
                is_term_focused,
                active_selection,
                buf,
            );
        }

        // 4. Render dropdowns & centered modals on demand
        if self.tab_menu.active {
            self.render_tab_context_menu(area, buf);
        }
        if self.new_tab_modal.active {
            self.render_new_tab_modal(area, buf);
        } else if self.focus == FocusArea::Input {
            self.render_command_modal(area, buf);
        }

        // 5. Render Shortcuts Help Overlay if toggled
        if self.show_help {
            self.render_help_overlay(area, buf);
        }
    }

    fn render_new_tab_modal(&self, area: Rect, buf: &mut Buffer) {
        let geom = new_tab_modal_geometry(area);
        if geom.modal.width < 20 || geom.modal.height < 6 {
            return;
        }
        draw_modal_box(geom.modal, buf, COLOR_ACCENT);

        let title = if self.new_tab_modal.rename_tab_index.is_some() {
            "rename tab"
        } else {
            "new tab"
        };
        for (i, ch) in title.chars().enumerate() {
            let x = geom.modal.x + 2 + i as u16;
            if x + 1 < geom.modal.x + geom.modal.width {
                let cell = &mut buf[(x, geom.modal.y + 1)];
                cell.set_char(ch);
                cell.set_style(Style::default().bg(COLOR_HEADER_BG).fg(COLOR_TEXT_PRIMARY));
            }
        }

        // Highlighted single-line input bar inside modal
        let input_row_y = geom.modal.y + 3;
        for x in (geom.modal.x + 2)..(geom.modal.x + geom.modal.width.saturating_sub(2)) {
            let cell = &mut buf[(x, input_row_y)];
            cell.set_char(' ');
            cell.set_style(Style::default().bg(COLOR_MUTED_BORDER).fg(COLOR_TEXT_PRIMARY));
        }
        let max_chars = geom.modal.width.saturating_sub(5) as usize;
        for (i, ch) in self.new_tab_modal.input.value.chars().take(max_chars).enumerate() {
            let x = geom.modal.x + 2 + i as u16;
            let cell = &mut buf[(x, input_row_y)];
            cell.set_char(ch);
            cell.set_style(Style::default().bg(COLOR_MUTED_BORDER).fg(COLOR_TEXT_PRIMARY));
        }
        let cursor_offset = (self.new_tab_modal.input.cursor_pos.min(max_chars)) as u16;
        let cursor_x = geom.modal.x + 2 + cursor_offset;
        if cursor_x + 1 < geom.modal.x + geom.modal.width {
            buf[(cursor_x, input_row_y)]
                .set_style(Style::default().bg(COLOR_ACCENT).fg(COLOR_CRUST));
        }

        // Action pills: [ ↵ save ]  [ ^c clear ]  [ esc cancel ]
        draw_pill_button(geom.save_btn, " ↵ save ", true, buf);
        draw_pill_button(geom.clear_btn, " ^c clear ", false, buf);
        draw_pill_button(geom.cancel_btn, " esc cancel ", false, buf);
    }

    fn render_tab_context_menu(&self, area: Rect, buf: &mut Buffer) {
        let menu_rect = tab_context_menu_rect(self.tab_menu.anchor_x, self.tab_menu.anchor_y, area);
        if menu_rect.width < 12 || menu_rect.height < 5 {
            return;
        }
        draw_modal_box(menu_rect, buf, COLOR_ACCENT);

        let items = [" New tab", " Rename", " Close"];
        for (i, label) in items.iter().enumerate() {
            let y = menu_rect.y + 1 + i as u16;
            let selected = i == self.tab_menu.selected_item;
            let style = if selected {
                Style::default()
                    .bg(COLOR_ACCENT)
                    .fg(COLOR_CRUST)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(COLOR_HEADER_BG).fg(COLOR_TEXT_PRIMARY)
            };
            for dx in 1..(menu_rect.width - 1) {
                let x = menu_rect.x + dx;
                let cell = &mut buf[(x, y)];
                cell.set_char(' ');
                cell.set_style(style);
            }
            for (ci, ch) in label.chars().take((menu_rect.width - 2) as usize).enumerate() {
                let x = menu_rect.x + 1 + ci as u16;
                let cell = &mut buf[(x, y)];
                cell.set_char(ch);
                cell.set_style(style);
            }
        }
    }

    fn render_command_modal(&self, area: Rect, buf: &mut Buffer) {
        let box_w = 56u16.min(area.width.saturating_sub(4));
        let box_h = 5u16.min(area.height.saturating_sub(2));
        if box_w < 20 || box_h < 4 {
            return;
        }
        let modal = Rect {
            x: area.x + (area.width - box_w) / 2,
            y: area.y + (area.height - box_h) / 2,
            width: box_w,
            height: box_h,
        };
        draw_modal_box(modal, buf, COLOR_ACCENT);

        let header = "command (/task, /lock, /split-h, /tab, /quit)";
        for (i, ch) in header.chars().take((box_w - 4) as usize).enumerate() {
            let cell = &mut buf[(modal.x + 2 + i as u16, modal.y + 1)];
            cell.set_char(ch);
            cell.set_style(Style::default().bg(COLOR_HEADER_BG).fg(COLOR_TEXT_MUTED));
        }
        let input_y = modal.y + 2;
        for x in (modal.x + 2)..(modal.x + box_w - 2) {
            let cell = &mut buf[(x, input_y)];
            cell.set_char(' ');
            cell.set_style(Style::default().bg(COLOR_MUTED_BORDER).fg(COLOR_TEXT_PRIMARY));
        }
        let prompt = format!("> {}", self.input_state.value);
        for (i, ch) in prompt.chars().take((box_w - 4) as usize).enumerate() {
            let cell = &mut buf[(modal.x + 2 + i as u16, input_y)];
            cell.set_char(ch);
            cell.set_style(Style::default().bg(COLOR_MUTED_BORDER).fg(COLOR_TEXT_PRIMARY));
        }
    }

    fn render_help_overlay(&self, area: Rect, buf: &mut Buffer) {
        let lines = [
            " Turf Cockpit — Shortcuts & Commands ",
            "",
            " Alt+T / Alt+W      New tab / Close tab",
            " Alt+1..9           Switch active tab",
            " Alt+V / Alt+H      Vertical / Horizontal split",
            " F3 / Alt+O         Cycle split pane (Alt+X close)",
            " Alt+B              Toggle Intent Board sidebar",
            " F2 / Alt+P         Toggle prompt focus (Tab cycles)",
            " Alt+Up / Alt+Down  Scroll history (Esc snaps back)",
            " F4 / Alt+D         Detach daemon (Ctrl+Q quits)",
            "",
            " Prompt: /task <title>  /lock <file>  /unlock <file>",
            "         /split  /split-h  /next  /close  /tab  /quit",
            "",
            " Press Esc, F1, or ? to close ",
        ];

        let box_w = 56u16.min(area.width.saturating_sub(2));
        let box_h = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
        if box_w < 20 || box_h < 6 {
            return;
        }
        let start_x = area.x + (area.width - box_w) / 2;
        let start_y = area.y + (area.height - box_h) / 2;

        let bg_style = Style::default().bg(COLOR_HEADER_BG).fg(COLOR_TEXT_PRIMARY);
        let border_style = Style::default().bg(COLOR_HEADER_BG).fg(COLOR_ACCENT);

        for dy in 0..box_h {
            for dx in 0..box_w {
                let x = start_x + dx;
                let y = start_y + dy;
                let cell = &mut buf[(x, y)];
                let is_top = dy == 0;
                let is_bot = dy == box_h - 1;
                let is_left = dx == 0;
                let is_right = dx == box_w - 1;
                let ch = if is_top && is_left {
                    '┌'
                } else if is_top && is_right {
                    '┐'
                } else if is_bot && is_left {
                    '└'
                } else if is_bot && is_right {
                    '┘'
                } else if is_top || is_bot {
                    '─'
                } else if is_left || is_right {
                    '│'
                } else {
                    ' '
                };
                cell.set_char(ch);
                cell.set_style(if is_top || is_bot || is_left || is_right {
                    border_style
                } else {
                    bg_style
                });
            }
        }

        for (i, line) in lines.iter().enumerate() {
            if (i as u16) + 2 >= box_h {
                break;
            }
            let y = start_y + 1 + i as u16;
            let style = if i == 0 {
                Style::default()
                    .bg(COLOR_ACTIVE_PILL)
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if i == lines.len() - 1 {
                Style::default().bg(COLOR_HEADER_BG).fg(COLOR_TEXT_MUTED)
            } else {
                bg_style
            };
            for (cx, ch) in line.chars().take((box_w - 2) as usize).enumerate() {
                let cell = &mut buf[(start_x + 1 + cx as u16, y)];
                cell.set_char(ch);
                cell.set_style(style);
            }
        }
    }

    /// Main 60fps frame blitting and event loop with a per-frame tick callback.
    pub fn run_event_loop_with<B: Backend, F>(
        &mut self,
        terminal: &mut RatTerminal<B>,
        mut on_tick: F,
    ) -> Result<(), io::Error>
    where
        F: FnMut(&mut Self),
    {
        let mut out = stdout();
        enable_raw_mode()?;
        crossterm::execute!(out, EnterAlternateScreen, EnableMouseCapture)?;

        // 60fps target frame time = ~16.6ms
        let frame_duration = Duration::from_micros(16_666);

        while !self.should_exit {
            let frame_start = Instant::now();

            // Run caller-supplied tick hook (e.g. state synchronization)
            on_tick(self);

            // 1. Drain live PTY chunks into terminal emulators
            self.poll_ptys();
            self.poll_daemon_events();

            // 2. Blit Ratatui frame (diff-based 60fps render with zero flicker)
            terminal.draw(|f| {
                self.render(f.area(), f.buffer_mut());
            })?;

            // 3. Calculate remaining frame time for event polling
            let elapsed = frame_start.elapsed();
            let timeout = if elapsed < frame_duration {
                frame_duration - elapsed
            } else {
                Duration::from_millis(1)
            };

            // 4. Poll keyboard, mouse & resize events
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        self.handle_key(key);
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse(mouse, self.last_area);
                    }
                    Event::Resize(_, _) => {
                        // Panes are resized on the next draw pass to their exact layout rects.
                    }
                    _ => {}
                }
            }
        }

        // Cleanup terminal state upon exit
        disable_raw_mode()?;
        crossterm::execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        Ok(())
    }

    /// Main 60fps frame blitting and event loop.
    pub fn run_event_loop<B: Backend>(&mut self, terminal: &mut RatTerminal<B>) -> Result<(), io::Error> {
        self.run_event_loop_with(terminal, |_| {})
    }
}

struct NewTabModalGeometry {
    modal: Rect,
    save_btn: Rect,
    clear_btn: Rect,
    cancel_btn: Rect,
}

fn new_tab_modal_geometry(area: Rect) -> NewTabModalGeometry {
    let box_w = 46u16.min(area.width.saturating_sub(2));
    let box_h = 7u16.min(area.height.saturating_sub(2));
    let modal = Rect {
        x: area.x + area.width.saturating_sub(box_w) / 2,
        y: area.y + area.height.saturating_sub(box_h) / 2,
        width: box_w,
        height: box_h,
    };
    let btn_y = modal.y + modal.height.saturating_sub(2);
    let start_x = modal.x + (modal.width.saturating_sub(32)) / 2;
    let save_btn = Rect { x: start_x, y: btn_y, width: 8, height: 1 };
    let clear_btn = Rect { x: start_x + 10, y: btn_y, width: 10, height: 1 };
    let cancel_btn = Rect { x: start_x + 22, y: btn_y, width: 12, height: 1 };
    NewTabModalGeometry {
        modal,
        save_btn,
        clear_btn,
        cancel_btn,
    }
}

fn tab_context_menu_rect(anchor_x: u16, anchor_y: u16, area: Rect) -> Rect {
    let w = 15u16.min(area.width);
    let h = 5u16.min(area.height);
    let max_x = (area.x + area.width).saturating_sub(w);
    let max_y = (area.y + area.height).saturating_sub(h);
    Rect {
        x: anchor_x.min(max_x),
        y: anchor_y.min(max_y),
        width: w,
        height: h,
    }
}

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && x >= rect.x
        && x < rect.x + rect.width
        && y >= rect.y
        && y < rect.y + rect.height
}

fn draw_modal_box(rect: Rect, buf: &mut Buffer, border_fg: ratatui::style::Color) {
    let bg_style = Style::default().bg(COLOR_HEADER_BG).fg(COLOR_TEXT_PRIMARY);
    let border_style = Style::default().bg(COLOR_HEADER_BG).fg(border_fg);
    for dy in 0..rect.height {
        for dx in 0..rect.width {
            let x = rect.x + dx;
            let y = rect.y + dy;
            if x >= buf.area().width || y >= buf.area().height {
                continue;
            }
            let is_top = dy == 0;
            let is_bot = dy == rect.height - 1;
            let is_left = dx == 0;
            let is_right = dx == rect.width - 1;
            let ch = if is_top && is_left {
                '┌'
            } else if is_top && is_right {
                '┐'
            } else if is_bot && is_left {
                '└'
            } else if is_bot && is_right {
                '┘'
            } else if is_top || is_bot {
                '─'
            } else if is_left || is_right {
                '│'
            } else {
                ' '
            };
            let cell = &mut buf[(x, y)];
            cell.set_char(ch);
            cell.set_style(if is_top || is_bot || is_left || is_right {
                border_style
            } else {
                bg_style
            });
        }
    }
}

fn draw_pill_button(rect: Rect, label: &str, primary: bool, buf: &mut Buffer) {
    let style = if primary {
        Style::default()
            .bg(COLOR_ACCENT)
            .fg(COLOR_CRUST)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(COLOR_ACTIVE_PILL).fg(COLOR_TEXT_PRIMARY)
    };
    for (i, ch) in label.chars().take(rect.width as usize).enumerate() {
        let x = rect.x + i as u16;
        if x < buf.area().width && rect.y < buf.area().height {
            let cell = &mut buf[(x, rect.y)];
            cell.set_char(ch);
            cell.set_style(style);
        }
    }
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = *chunk.get(1).unwrap_or(&0) as usize;
        let b2 = *chunk.get(2).unwrap_or(&0) as usize;
        out.push(TABLE[(b0 >> 2) & 0x3f] as char);
        out.push(TABLE[((b0 & 0x03) << 4) | ((b1 >> 4) & 0x0f)] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((b1 & 0x0f) << 2) | ((b2 >> 6) & 0x03)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[b2 & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

fn copy_to_clipboard(text: &str) {
    let b64 = encode_base64(text.as_bytes());
    let mut out = stdout();
    let _ = write!(out, "\x1b]52;c;{b64}\x07");
    let _ = out.flush();

    let is_wsl = std::env::var_os("WSL_DISTRO_NAME").is_some()
        || Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists();
    if cfg!(target_os = "windows") || is_wsl {
        if let Ok(mut child) = std::process::Command::new("clip.exe")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
        }
    }
}

fn open_external_url(url: &str) {
    let is_wsl = std::env::var_os("WSL_DISTRO_NAME").is_some()
        || Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists();
    if cfg!(target_os = "windows") || is_wsl {
        // rundll32 passes URL query parameters (&, ?, =) intact without cmd.exe shell parsing
        let _ = std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    } else if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    } else {
        let _ = std::process::Command::new("xdg-open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}
