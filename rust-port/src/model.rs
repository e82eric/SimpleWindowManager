use crate::layout::Layout;
use std::collections::VecDeque;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn from_xywh(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ClientId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkspaceId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MonitorId(pub usize);

#[derive(Clone, Debug, Default)]
pub struct ClientData {
    pub rect: Rect,
    pub hwnd: isize,
    pub process_id: u32,
    pub process_image_name: String,
    pub class_name: String,
    pub title: String,
    pub command_line: Option<String>,
    pub is_elevated: bool,
    pub is_minimized: bool,
    pub use_cloak_to_hide: bool,
}

#[derive(Clone, Debug)]
pub struct Client {
    pub data: ClientData,
    pub is_visible: bool,
    pub workspace: WorkspaceId,
}

#[derive(Clone, Debug, Default)]
pub struct WorkspaceFilterData {
    pub titles: Vec<String>,
    pub not_titles: Vec<String>,
    pub process_image_names: Vec<String>,
    pub not_process_image_names: Vec<String>,
    pub class_names: Vec<String>,
    pub not_class_names: Vec<String>,
}

impl WorkspaceFilterData {
    pub fn matches(&self, client: &Client) -> bool {
        let data = &client.data;
        if self
            .not_titles
            .iter()
            .any(|filter| data.title.contains(filter))
        {
            return false;
        }
        if self.titles.iter().any(|filter| data.title.contains(filter)) {
            return true;
        }
        if self
            .not_class_names
            .iter()
            .any(|filter| data.class_name.contains(filter))
        {
            return false;
        }
        if self
            .class_names
            .iter()
            .any(|filter| data.class_name.contains(filter))
        {
            return true;
        }
        if self
            .not_process_image_names
            .iter()
            .any(|filter| data.process_image_name.contains(filter))
        {
            return false;
        }
        self.process_image_names
            .iter()
            .any(|filter| data.process_image_name.contains(filter))
    }
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub name: String,
    pub tag: String,
    pub is_icon: bool,
    pub clients: Vec<ClientId>,
    pub minimized_clients: Vec<ClientId>,
    pub monitor: Option<MonitorId>,
    pub selected: Option<ClientId>,
    pub main_offset: i32,
    pub layout: Layout,
    pub filter_data: WorkspaceFilterData,
    pub saved_layout: Vec<isize>,
    pub saved_selected: Option<isize>,
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub id: i32,
    pub top: i32,
    pub bottom: i32,
    pub x_offset: i32,
    pub height: i32,
    pub width: i32,
    pub workspace: Option<WorkspaceId>,
    pub is_hidden: bool,
    pub selected: bool,
    pub gap_width: i32,
}

#[derive(Clone, Debug)]
pub struct FloatLogEntry {
    pub timestamp: SystemTime,
    pub hwnd: isize,
    pub process_id: u32,
    pub process_image_name: String,
    pub class_name: String,
    pub title: String,
    pub is_floated: bool,
    pub reason: String,
    pub styles: i32,
    pub ex_styles: i32,
    pub window_width: i32,
    pub window_height: i32,
}

#[derive(Clone, Debug)]
pub struct ClientLogEntry {
    pub timestamp: SystemTime,
    pub hwnd: isize,
    pub process_id: u32,
    pub process_image_name: String,
    pub class_name: String,
    pub title: String,
    pub workspace_name: String,
    pub was_minimized: bool,
    pub styles: i32,
    pub ex_styles: i32,
    pub window_width: i32,
    pub window_height: i32,
}

impl Monitor {
    pub fn new(id: i32, rect: Rect, gap_width: i32) -> Self {
        Self {
            id,
            top: rect.top,
            bottom: rect.bottom,
            x_offset: rect.left,
            height: rect.bottom - rect.top,
            width: rect.right - rect.left,
            workspace: None,
            is_hidden: false,
            selected: false,
            gap_width,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WindowRoutingMode {
    FilteredAndRoutedToWorkspace,
    FilteredCurrentWorkspace,
    NotFilteredCurrentWorkspace,
    #[default]
    FilteredRoutedNonFilteredCurrentWorkspace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Default)]
pub struct WindowManagerState {
    pub clients: Vec<Option<Client>>,
    pub workspaces: Vec<Workspace>,
    pub monitors: Vec<Monitor>,
    pub selected_monitor: Option<MonitorId>,
    pub last_workspace: Option<WorkspaceId>,
    pub current_window_routing_mode: WindowRoutingMode,
    pub float_log_buffer: VecDeque<FloatLogEntry>,
    pub client_log_buffer: VecDeque<ClientLogEntry>,
}

impl WindowManagerState {
    pub fn log_float_decision(&mut self, entry: FloatLogEntry) {
        if self.float_log_buffer.len() == 100 {
            self.float_log_buffer.pop_front();
        }
        self.float_log_buffer.push_back(entry);
    }

    pub fn log_client_addition(&mut self, entry: ClientLogEntry) {
        if self.client_log_buffer.len() == 100 {
            self.client_log_buffer.pop_front();
        }
        self.client_log_buffer.push_back(entry);
    }
    pub fn workspace_register(
        &mut self,
        name: impl Into<String>,
        tag: impl Into<String>,
        is_icon: bool,
        layout: Layout,
    ) -> WorkspaceId {
        let id = WorkspaceId(self.workspaces.len());
        self.workspaces.push(Workspace {
            name: name.into(),
            tag: tag.into(),
            is_icon,
            clients: Vec::new(),
            minimized_clients: Vec::new(),
            monitor: None,
            selected: None,
            main_offset: 0,
            layout,
            filter_data: WorkspaceFilterData::default(),
            saved_layout: Vec::new(),
            saved_selected: None,
        });
        id
    }

    pub fn monitor_register(&mut self, monitor: Monitor) -> MonitorId {
        let id = MonitorId(self.monitors.len());
        self.monitors.push(monitor);
        id
    }

    pub fn client_add(&mut self, workspace: WorkspaceId, mut client: Client) -> ClientId {
        client.workspace = workspace;
        let id = ClientId(self.clients.len());
        let minimized = client.data.is_minimized;
        self.clients.push(Some(client));
        let target = &mut self.workspaces[workspace.0];
        if minimized {
            target.minimized_clients.insert(0, id);
        } else {
            target.clients.insert(0, id);
            target.selected = Some(id);
        }
        id
    }

    pub fn workspace_find_client_by_hwnd(
        &self,
        workspace: WorkspaceId,
        hwnd: isize,
    ) -> Option<ClientId> {
        let workspace = &self.workspaces[workspace.0];
        workspace
            .clients
            .iter()
            .chain(&workspace.minimized_clients)
            .copied()
            .find(|id| {
                self.clients[id.0]
                    .as_ref()
                    .is_some_and(|client| client.data.hwnd == hwnd)
            })
    }

    pub fn window_manager_find_client_in_workspaces_by_hwnd(
        &self,
        hwnd: isize,
    ) -> Option<ClientId> {
        (0..self.workspaces.len())
            .find_map(|index| self.workspace_find_client_by_hwnd(WorkspaceId(index), hwnd))
    }

    pub fn workspace_remove_client(&mut self, client_id: ClientId) -> bool {
        let Some(client) = self.clients[client_id.0].as_ref() else {
            return false;
        };
        let workspace_id = client.workspace;
        let is_minimized = client.data.is_minimized;
        let workspace = &mut self.workspaces[workspace_id.0];
        let list = if is_minimized {
            &mut workspace.minimized_clients
        } else {
            &mut workspace.clients
        };
        let Some(position) = list.iter().position(|id| *id == client_id) else {
            return false;
        };
        list.remove(position);
        workspace.selected = if list.is_empty() {
            None
        } else if position == 0 {
            Some(list[0])
        } else {
            Some(list[position - 1])
        };
        true
    }

    pub fn client_move_from_unminimized_to_minimized(&mut self, client_id: ClientId) {
        if self.clients[client_id.0]
            .as_ref()
            .is_none_or(|client| client.data.is_minimized)
        {
            return;
        }
        let workspace = self.clients[client_id.0].as_ref().unwrap().workspace;
        self.workspace_remove_client(client_id);
        self.clients[client_id.0]
            .as_mut()
            .unwrap()
            .data
            .is_minimized = true;
        self.workspaces[workspace.0]
            .minimized_clients
            .insert(0, client_id);
    }

    pub fn client_move_from_minimized_to_unminimized(&mut self, client_id: ClientId) {
        if self.clients[client_id.0]
            .as_ref()
            .is_none_or(|client| !client.data.is_minimized)
        {
            return;
        }
        let workspace = self.clients[client_id.0].as_ref().unwrap().workspace;
        self.workspace_remove_client(client_id);
        self.clients[client_id.0]
            .as_mut()
            .unwrap()
            .data
            .is_minimized = false;
        let target = &mut self.workspaces[workspace.0];
        target.clients.insert(0, client_id);
        target.selected = Some(client_id);
    }

    pub fn swap_client_nodes_data(&mut self, first: ClientId, second: ClientId) {
        if first == second {
            return;
        }
        let (low, high, reverse) = if first.0 < second.0 {
            (first.0, second.0, false)
        } else {
            (second.0, first.0, true)
        };
        let (left, right) = self.clients.split_at_mut(high);
        let low_client = left[low].as_mut().unwrap();
        let high_client = right[0].as_mut().unwrap();
        if reverse {
            std::mem::swap(&mut high_client.data, &mut low_client.data);
        } else {
            std::mem::swap(&mut low_client.data, &mut high_client.data);
        }
    }

    pub fn tile_layout_select_next_window(&mut self, workspace_id: WorkspaceId) {
        let workspace = &mut self.workspaces[workspace_id.0];
        if workspace.clients.is_empty() {
            return;
        }
        let position = workspace
            .selected
            .and_then(|selected| workspace.clients.iter().position(|id| *id == selected));
        workspace.selected =
            Some(workspace.clients[position.map_or(0, |p| (p + 1) % workspace.clients.len())]);
    }

    pub fn tile_layout_select_previous_window(&mut self, workspace_id: WorkspaceId) {
        let workspace = &mut self.workspaces[workspace_id.0];
        if workspace.clients.is_empty() {
            return;
        }
        let position = workspace
            .selected
            .and_then(|selected| workspace.clients.iter().position(|id| *id == selected));
        workspace.selected = Some(
            workspace.clients[position.map_or(0, |p| {
                if p == 0 {
                    workspace.clients.len() - 1
                } else {
                    p - 1
                }
            })],
        );
    }

    pub fn tilelayout_move_client_next(&mut self, workspace_id: WorkspaceId, client_id: ClientId) {
        let clients = self.workspaces[workspace_id.0].clients.clone();
        if clients.len() <= 1 {
            return;
        }
        let Some(position) = clients.iter().position(|id| *id == client_id) else {
            return;
        };
        if position + 1 < clients.len() {
            self.swap_client_nodes_data(client_id, clients[position + 1]);
            self.workspaces[workspace_id.0].selected = Some(clients[position + 1]);
        } else {
            for position in (1..clients.len()).rev() {
                self.swap_client_nodes_data(clients[position], clients[position - 1]);
            }
            self.workspaces[workspace_id.0].selected = Some(clients[0]);
        }
    }

    pub fn tilelayout_move_client_previous(
        &mut self,
        workspace_id: WorkspaceId,
        client_id: ClientId,
    ) {
        let clients = self.workspaces[workspace_id.0].clients.clone();
        if clients.len() <= 1 {
            return;
        }
        let Some(position) = clients.iter().position(|id| *id == client_id) else {
            return;
        };
        if position > 0 {
            self.swap_client_nodes_data(client_id, clients[position - 1]);
            self.workspaces[workspace_id.0].selected = Some(clients[position - 1]);
        } else {
            for position in 0..clients.len() - 1 {
                self.swap_client_nodes_data(clients[position], clients[position + 1]);
            }
            self.workspaces[workspace_id.0].selected = clients.last().copied();
        }
    }

    pub fn monacle_layout_select_next_client(&mut self, workspace_id: WorkspaceId) {
        let clients = self.workspaces[workspace_id.0].clients.clone();
        if clients.len() <= 1 {
            return;
        }
        for position in 0..clients.len() - 1 {
            self.swap_client_nodes_data(clients[position], clients[position + 1]);
        }
        self.workspaces[workspace_id.0].selected = Some(clients[0]);
    }

    pub fn monacle_layout_select_previous_client(&mut self, workspace_id: WorkspaceId) {
        let clients = self.workspaces[workspace_id.0].clients.clone();
        if clients.len() <= 1 {
            return;
        }
        for position in (1..clients.len()).rev() {
            self.swap_client_nodes_data(clients[position], clients[position - 1]);
        }
        self.workspaces[workspace_id.0].selected = Some(clients[0]);
    }

    pub fn layout_select_next_window(&mut self, workspace: WorkspaceId) {
        match self.workspaces[workspace.0].layout {
            Layout::Deck | Layout::HorizontalDeck => self.deck_layout_select_next_window(workspace),
            Layout::Monocle => self.monacle_layout_select_next_client(workspace),
            Layout::Grid => self.grid_layout_select_next_window(workspace),
            Layout::Tile | Layout::TileReversed => self.tile_layout_select_next_window(workspace),
        }
    }

    pub fn layout_select_previous_window(&mut self, workspace: WorkspaceId) {
        match self.workspaces[workspace.0].layout {
            Layout::Deck | Layout::HorizontalDeck => self.deck_layout_select_next_window(workspace),
            Layout::Monocle => self.monacle_layout_select_previous_client(workspace),
            Layout::Grid => self.grid_layout_select_previous_window(workspace),
            Layout::Tile | Layout::TileReversed => {
                self.tile_layout_select_previous_window(workspace)
            }
        }
    }

    pub fn layout_select_left(&mut self, workspace: WorkspaceId) {
        match self.workspaces[workspace.0].layout {
            Layout::Grid => self.grid_layout_select_horizontal(workspace, false),
            Layout::TileReversed => {
                let item = &mut self.workspaces[workspace.0];
                if item.clients.len() > 1 && item.selected == Some(item.clients[0]) {
                    item.selected = Some(item.clients[1]);
                }
            }
            Layout::Tile | Layout::Deck | Layout::HorizontalDeck => {
                let clients = &self.workspaces[workspace.0].clients;
                if !clients.is_empty()
                    && self.workspaces[workspace.0].selected.is_some()
                    && self.workspaces[workspace.0].selected != Some(clients[0])
                {
                    self.workspaces[workspace.0].selected = Some(clients[0]);
                }
            }
            Layout::Monocle => self.layout_select_previous_window(workspace),
        }
    }

    pub fn layout_select_right(&mut self, workspace: WorkspaceId) {
        match self.workspaces[workspace.0].layout {
            Layout::Grid => self.grid_layout_select_horizontal(workspace, true),
            Layout::TileReversed => {
                let item = &mut self.workspaces[workspace.0];
                if !item.clients.is_empty()
                    && item.selected.is_some()
                    && item.selected != Some(item.clients[0])
                {
                    item.selected = Some(item.clients[0]);
                }
            }
            Layout::Tile | Layout::Deck | Layout::HorizontalDeck => {
                let clients = &self.workspaces[workspace.0].clients;
                if clients.len() > 1 && self.workspaces[workspace.0].selected == Some(clients[0]) {
                    self.workspaces[workspace.0].selected = Some(clients[1]);
                }
            }
            Layout::Monocle => self.layout_select_next_window(workspace),
        }
    }

    pub fn layout_select_up(&mut self, workspace: WorkspaceId) {
        match self.workspaces[workspace.0].layout {
            Layout::Grid => self.grid_layout_select_vertical(workspace, false),
            Layout::Deck | Layout::HorizontalDeck => {
                if let Some(client) = self.workspaces[workspace.0].selected {
                    self.deck_layout_move_client_previous(workspace, client);
                }
            }
            _ => self.layout_select_previous_window(workspace),
        }
    }

    pub fn layout_select_down(&mut self, workspace: WorkspaceId) {
        match self.workspaces[workspace.0].layout {
            Layout::Grid => self.grid_layout_select_vertical(workspace, true),
            Layout::Deck | Layout::HorizontalDeck => {
                if let Some(client) = self.workspaces[workspace.0].selected {
                    self.deck_layout_move_client_next(workspace, client);
                }
            }
            _ => self.layout_select_next_window(workspace),
        }
    }

    pub fn layout_move_client_next(&mut self, workspace: WorkspaceId, client: ClientId) {
        match self.workspaces[workspace.0].layout {
            Layout::Deck | Layout::HorizontalDeck => {
                self.deck_layout_move_client_next(workspace, client)
            }
            Layout::Monocle => self.monacle_layout_select_next_client(workspace),
            Layout::Grid => self.grid_layout_move_client(workspace, client, true),
            _ => self.tilelayout_move_client_next(workspace, client),
        }
    }

    pub fn layout_move_client_previous(&mut self, workspace: WorkspaceId, client: ClientId) {
        match self.workspaces[workspace.0].layout {
            Layout::Deck | Layout::HorizontalDeck => {
                self.deck_layout_move_client_previous(workspace, client)
            }
            Layout::Monocle => self.monacle_layout_select_previous_client(workspace),
            Layout::Grid => self.grid_layout_move_client(workspace, client, false),
            _ => self.tilelayout_move_client_previous(workspace, client),
        }
    }

    pub fn layout_move_client_direction(
        &mut self,
        workspace: WorkspaceId,
        client: ClientId,
        direction: MoveDirection,
    ) {
        let layout = self.workspaces[workspace.0].layout;
        match (layout, direction) {
            (Layout::Grid, _) => self.grid_layout_move_direction(workspace, client, direction),
            (Layout::Monocle, MoveDirection::Left | MoveDirection::Up) => {
                self.monacle_layout_select_previous_client(workspace)
            }
            (Layout::Monocle, MoveDirection::Right | MoveDirection::Down) => {
                self.monacle_layout_select_next_client(workspace)
            }
            (Layout::TileReversed, MoveDirection::Left)
            | (Layout::Tile, MoveDirection::Right)
            | (Layout::Deck | Layout::HorizontalDeck, MoveDirection::Right) => {
                self.tile_layout_move_horizontal(workspace, client, true)
            }
            (Layout::TileReversed, MoveDirection::Right)
            | (Layout::Tile, MoveDirection::Left)
            | (Layout::Deck | Layout::HorizontalDeck, MoveDirection::Left) => {
                self.tile_layout_move_horizontal(workspace, client, false)
            }
            (Layout::Deck | Layout::HorizontalDeck, MoveDirection::Up) => {
                self.deck_layout_move_client_previous(workspace, client)
            }
            (Layout::Deck | Layout::HorizontalDeck, MoveDirection::Down) => {
                self.deck_layout_move_client_next(workspace, client)
            }
            (Layout::Tile | Layout::TileReversed, MoveDirection::Up) => {
                self.tilelayout_move_client_previous(workspace, client)
            }
            (Layout::Tile | Layout::TileReversed, MoveDirection::Down) => {
                self.tilelayout_move_client_next(workspace, client)
            }
        }
    }

    fn tile_layout_move_horizontal(
        &mut self,
        workspace: WorkspaceId,
        client: ClientId,
        right: bool,
    ) {
        let clients = self.workspaces[workspace.0].clients.clone();
        if clients.is_empty() {
            return;
        }
        if right && client == clients[0] && clients.len() > 1 {
            self.swap_client_nodes_data(client, clients[1]);
            self.workspaces[workspace.0].selected = Some(clients[1]);
        } else if !right && client != clients[0] {
            self.swap_client_nodes_data(client, clients[0]);
            self.workspaces[workspace.0].selected = Some(clients[0]);
        }
    }

    fn grid_layout_move_direction(
        &mut self,
        workspace: WorkspaceId,
        client: ClientId,
        direction: MoveDirection,
    ) {
        let clients = self.workspaces[workspace.0].clients.clone();
        let count = clients.len();
        if count <= 1 {
            return;
        }
        let Some(position) = clients.iter().position(|candidate| *candidate == client) else {
            return;
        };
        let left = count / 2;
        let target = match direction {
            MoveDirection::Left if position >= left => Some((position - left).min(left - 1)),
            MoveDirection::Right if position < left => Some((position + left).min(count - 1)),
            MoveDirection::Up if (position < left && position > 0) || position > left => {
                Some(position - 1)
            }
            MoveDirection::Down
                if (position < left && position + 1 < left)
                    || (position >= left && position + 1 < count) =>
            {
                Some(position + 1)
            }
            _ => None,
        };
        if let Some(target) = target {
            self.swap_client_nodes_data(client, clients[target]);
            self.workspaces[workspace.0].selected = Some(clients[target]);
        }
    }

    pub fn layout_move_client_to_main(&mut self, workspace: WorkspaceId, client: ClientId) {
        let clients = self.workspaces[workspace.0].clients.clone();
        if clients.len() < 2 {
            return;
        }
        if self.workspaces[workspace.0].layout == Layout::Grid {
            let Some(position) = clients.iter().position(|candidate| *candidate == client) else {
                return;
            };
            let left = clients.len() / 2;
            let target = if position >= left {
                (position - left).min(left - 1)
            } else {
                (position + left).min(clients.len() - 1)
            };
            if target != position {
                self.swap_client_nodes_data(client, clients[target]);
                self.workspaces[workspace.0].selected = Some(clients[target]);
            }
        } else {
            let source = if client == clients[0] {
                clients[1]
            } else {
                client
            };
            self.swap_client_nodes_data(source, clients[0]);
        }
    }

    pub fn workspace_save_layout(&mut self, workspace: WorkspaceId) {
        let item = &mut self.workspaces[workspace.0];
        item.saved_layout = item
            .clients
            .iter()
            .filter_map(|id| self.clients[id.0].as_ref().map(|client| client.data.hwnd))
            .collect();
        item.saved_selected = item
            .selected
            .and_then(|id| self.clients[id.0].as_ref().map(|client| client.data.hwnd));
    }

    pub fn workspace_restore_saved_layout(&mut self, workspace: WorkspaceId) {
        let saved = self.workspaces[workspace.0].saved_layout.clone();
        if saved.is_empty() {
            return;
        }
        let nodes = self.workspaces[workspace.0].clients.clone();
        for (position, hwnd) in saved.into_iter().enumerate().take(nodes.len()) {
            let found = nodes.iter().copied().find(|id| {
                self.clients[id.0]
                    .as_ref()
                    .is_some_and(|client| client.data.hwnd == hwnd)
            });
            if let Some(found) = found {
                self.swap_client_nodes_data(nodes[position], found);
            }
        }
        if let Some(hwnd) = self.workspaces[workspace.0].saved_selected {
            self.workspaces[workspace.0].selected = nodes.iter().copied().find(|id| {
                self.clients[id.0]
                    .as_ref()
                    .is_some_and(|client| client.data.hwnd == hwnd)
            });
        }
        self.workspaces[workspace.0].saved_layout.clear();
        self.workspaces[workspace.0].saved_selected = None;
    }

    fn deck_layout_select_next_window(&mut self, workspace: WorkspaceId) {
        let item = &mut self.workspaces[workspace.0];
        if item.clients.is_empty() {
            return;
        }
        item.selected = if item.clients.len() == 1 {
            Some(item.clients[0])
        } else if item.selected == Some(item.clients[0]) {
            Some(item.clients[1])
        } else if item.selected == Some(item.clients[1]) {
            Some(item.clients[0])
        } else {
            Some(item.clients[0])
        };
    }

    fn deck_layout_move_client_next(&mut self, workspace: WorkspaceId, client: ClientId) {
        let clients = self.workspaces[workspace.0].clients.clone();
        if clients.len() <= 1 {
            return;
        }
        let Some(position) = clients.iter().position(|id| *id == client) else {
            return;
        };
        if position == 0 {
            for index in 0..clients.len() - 1 {
                self.swap_client_nodes_data(clients[index], clients[index + 1]);
            }
            self.swap_client_nodes_data(clients[0], clients[1]);
        } else if !(position == 1 && clients.len() == 2) {
            for index in 1..clients.len() - 1 {
                self.swap_client_nodes_data(clients[index], clients[index + 1]);
            }
            self.workspaces[workspace.0].selected = Some(clients[1]);
        }
    }

    fn deck_layout_move_client_previous(&mut self, workspace: WorkspaceId, client: ClientId) {
        let clients = self.workspaces[workspace.0].clients.clone();
        if clients.len() <= 1 {
            return;
        }
        let Some(position) = clients.iter().position(|id| *id == client) else {
            return;
        };
        if position == 0 {
            self.swap_client_nodes_data(clients[0], *clients.last().unwrap());
        } else if !(position == 1 && clients.len() == 2) {
            for index in (2..clients.len()).rev() {
                self.swap_client_nodes_data(clients[index], clients[index - 1]);
            }
            self.workspaces[workspace.0].selected = Some(clients[1]);
        }
    }

    fn grid_next_pos(position: usize, left_count: usize, count: usize) -> usize {
        if position == 0 {
            left_count
        } else if position >= left_count && position < count - 1 {
            position + 1
        } else if position == count - 1 {
            left_count - 1
        } else if position > 1 {
            position - 1
        } else {
            0
        }
    }

    fn grid_previous_pos(position: usize, left_count: usize, count: usize) -> usize {
        if position == 0 {
            if left_count > 1 { 1 } else { count - 1 }
        } else if position == left_count {
            0
        } else if position > left_count {
            position - 1
        } else if position == left_count - 1 {
            count - 1
        } else {
            position + 1
        }
    }

    fn grid_layout_select_next_window(&mut self, workspace: WorkspaceId) {
        let item = &mut self.workspaces[workspace.0];
        if item.clients.len() <= 1 {
            item.selected = item.clients.first().copied();
            return;
        }
        let position = item
            .selected
            .and_then(|id| item.clients.iter().position(|candidate| *candidate == id))
            .unwrap_or(0);
        item.selected = Some(
            item.clients[Self::grid_next_pos(position, item.clients.len() / 2, item.clients.len())],
        );
    }

    fn grid_layout_select_previous_window(&mut self, workspace: WorkspaceId) {
        let item = &mut self.workspaces[workspace.0];
        if item.clients.len() <= 1 {
            item.selected = item.clients.first().copied();
            return;
        }
        let position = item
            .selected
            .and_then(|id| item.clients.iter().position(|candidate| *candidate == id))
            .unwrap_or(0);
        item.selected = Some(
            item.clients
                [Self::grid_previous_pos(position, item.clients.len() / 2, item.clients.len())],
        );
    }

    fn grid_layout_select_horizontal(&mut self, workspace: WorkspaceId, right: bool) {
        let item = &mut self.workspaces[workspace.0];
        let count = item.clients.len();
        if count <= 1 {
            return;
        }
        let left = count / 2;
        let Some(position) = item
            .selected
            .and_then(|id| item.clients.iter().position(|candidate| *candidate == id))
        else {
            return;
        };
        let target = if right && position < left {
            Some((position + left).min(count - 1))
        } else if !right && position >= left {
            Some((position - left).min(left - 1))
        } else {
            None
        };
        if let Some(target) = target {
            item.selected = Some(item.clients[target]);
        }
    }

    fn grid_layout_select_vertical(&mut self, workspace: WorkspaceId, down: bool) {
        let item = &mut self.workspaces[workspace.0];
        let count = item.clients.len();
        if count <= 1 {
            return;
        }
        let left = count / 2;
        let Some(position) = item
            .selected
            .and_then(|id| item.clients.iter().position(|candidate| *candidate == id))
        else {
            return;
        };
        let boundary = if position < left {
            if down {
                position + 1 < left
            } else {
                position > 0
            }
        } else if down {
            position + 1 < count
        } else {
            position > left
        };
        if boundary {
            item.selected = Some(item.clients[if down { position + 1 } else { position - 1 }]);
        }
    }

    fn grid_layout_move_client(&mut self, workspace: WorkspaceId, client: ClientId, next: bool) {
        let clients = self.workspaces[workspace.0].clients.clone();
        if clients.len() <= 1 {
            return;
        }
        let Some(position) = clients.iter().position(|id| *id == client) else {
            return;
        };
        let target = if next {
            Self::grid_next_pos(position, clients.len() / 2, clients.len())
        } else {
            Self::grid_previous_pos(position, clients.len() / 2, clients.len())
        };
        self.swap_client_nodes_data(client, clients[target]);
        self.workspaces[workspace.0].selected = Some(clients[target]);
    }

    pub fn workspace_find_for_client(&self, client: &Client) -> Option<WorkspaceId> {
        self.workspaces
            .iter()
            .position(|workspace| workspace.filter_data.matches(client))
            .map(WorkspaceId)
    }

    pub fn arrange_workspace(&mut self, workspace_id: WorkspaceId) {
        let layout = self.workspaces[workspace_id.0].layout;
        let clients = self.workspaces[workspace_id.0].clients.clone();
        if let Some(selected) = self.workspaces[workspace_id.0].selected {
            match layout {
                Layout::Deck | Layout::HorizontalDeck if clients.len() > 1 => {
                    if selected != clients[0] && selected != clients[1] {
                        self.swap_client_nodes_data(selected, clients[1]);
                        self.workspaces[workspace_id.0].selected = Some(clients[1]);
                    }
                }
                Layout::Monocle if !clients.is_empty() => {
                    self.swap_client_nodes_data(selected, clients[0]);
                    self.workspaces[workspace_id.0].selected = Some(clients[0]);
                }
                _ => {}
            }
        }
        let workspace = &self.workspaces[workspace_id.0];
        let Some(monitor_id) = workspace.monitor else {
            return;
        };
        let rects = workspace
            .layout
            .calculate(workspace, &self.monitors[monitor_id.0]);
        let ids = workspace.clients.clone();
        for (index, id) in ids.into_iter().enumerate() {
            if let Some(client) = self.clients[id.0].as_mut() {
                client.data.rect = rects[index].0;
                client.is_visible = rects[index].1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(process: &str, class: &str, title: &str) -> Client {
        Client {
            data: ClientData {
                process_image_name: process.into(),
                class_name: class.into(),
                title: title.into(),
                ..Default::default()
            },
            is_visible: true,
            workspace: WorkspaceId(0),
        }
    }

    fn state_with_clients(count: usize) -> (WindowManagerState, WorkspaceId, Vec<ClientId>) {
        let mut state = WindowManagerState::default();
        let workspace = state.workspace_register("test", "T", false, Layout::Tile);
        let mut clients = Vec::new();
        for index in (0..count).rev() {
            let mut value = client(&format!("{index}.exe"), "Class", &format!("{index}"));
            value.data.hwnd = index as isize + 1;
            clients.push(state.client_add(workspace, value));
        }
        clients.reverse();
        (state, workspace, clients)
    }

    #[test]
    fn filters_match_the_c_ordered_short_circuit_rules() {
        let filters = WorkspaceFilterData {
            process_image_names: vec!["chrome.exe".into(), "firefox.exe".into()],
            not_titles: vec!["Incognito".into()],
            ..Default::default()
        };
        assert!(filters.matches(&client("chrome.exe", "Chrome_WidgetWin", "Docs")));
        assert!(!filters.matches(&client("chrome.exe", "Chrome_WidgetWin", "Incognito")));
        assert!(!filters.matches(&client("notepad.exe", "Notepad", "Docs")));
    }

    #[test]
    fn a_positive_title_match_returns_before_later_process_exclusions() {
        let filters = WorkspaceFilterData {
            titles: vec!["Docs".into()],
            not_process_image_names: vec!["chrome.exe".into()],
            ..Default::default()
        };
        assert!(filters.matches(&client("chrome.exe", "Chrome", "Docs")));
    }

    #[test]
    fn clients_are_added_at_the_list_root_like_c() {
        let (state, workspace, clients) = state_with_clients(3);
        assert_eq!(state.workspaces[workspace.0].clients, clients);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[0]));
    }

    #[test]
    fn tile_move_wraps_by_swapping_data_not_nodes() {
        let (mut state, workspace, clients) = state_with_clients(3);
        let node_order = state.workspaces[workspace.0].clients.clone();
        state.tilelayout_move_client_previous(workspace, clients[0]);
        assert_eq!(state.workspaces[workspace.0].clients, node_order);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[2]));
        let titles: Vec<_> = clients
            .iter()
            .map(|id| state.clients[id.0].as_ref().unwrap().data.title.as_str())
            .collect();
        assert_eq!(titles, ["1", "2", "0"]);
    }

    #[test]
    fn reversed_tile_swaps_left_and_right_selection() {
        let (mut state, workspace, clients) = state_with_clients(3);
        state.workspaces[workspace.0].layout = Layout::TileReversed;
        state.workspaces[workspace.0].selected = Some(clients[0]);
        state.layout_select_left(workspace);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[1]));
        state.layout_select_right(workspace);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[0]));
    }

    #[test]
    fn minimize_and_restore_use_root_insertion() {
        let (mut state, workspace, clients) = state_with_clients(3);
        state.client_move_from_unminimized_to_minimized(clients[1]);
        assert_eq!(
            state.workspaces[workspace.0].minimized_clients,
            [clients[1]]
        );
        state.client_move_from_minimized_to_unminimized(clients[1]);
        assert_eq!(state.workspaces[workspace.0].clients[0], clients[1]);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[1]));
    }

    #[test]
    fn grid_selection_follows_the_c_clockwise_circuit() {
        let (mut state, workspace, clients) = state_with_clients(5);
        state.workspaces[workspace.0].layout = Layout::Grid;
        state.workspaces[workspace.0].selected = Some(clients[0]);
        let mut visited = Vec::new();
        for _ in 0..5 {
            visited.push(state.workspaces[workspace.0].selected.unwrap());
            state.layout_select_next_window(workspace);
        }
        assert_eq!(
            visited,
            [clients[0], clients[2], clients[3], clients[4], clients[1]]
        );
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[0]));
    }

    #[test]
    fn deck_selection_only_toggles_main_and_top_of_deck() {
        let (mut state, workspace, clients) = state_with_clients(4);
        state.workspaces[workspace.0].layout = Layout::Deck;
        state.workspaces[workspace.0].selected = Some(clients[3]);
        state.layout_select_next_window(workspace);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[0]));
        state.layout_select_next_window(workspace);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[1]));
    }

    #[test]
    fn deck_arrangement_promotes_hidden_selection_to_visible_stack_top() {
        let (mut state, workspace, clients) = state_with_clients(4);
        state.workspaces[workspace.0].layout = Layout::Deck;
        state.workspaces[workspace.0].selected = Some(clients[3]);
        let monitor =
            state.monitor_register(Monitor::new(1, Rect::from_xywh(0, 0, 1920, 1080), 13));
        state.workspaces[workspace.0].monitor = Some(monitor);
        state.monitors[monitor.0].workspace = Some(workspace);
        let selected_title = state.clients[clients[3].0]
            .as_ref()
            .unwrap()
            .data
            .title
            .clone();
        state.arrange_workspace(workspace);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[1]));
        assert_eq!(
            state.clients[clients[1].0].as_ref().unwrap().data.title,
            selected_title
        );
        assert!(state.clients[clients[1].0].as_ref().unwrap().is_visible);
    }

    #[test]
    fn monocle_arrangement_promotes_selection_to_visible_root() {
        let (mut state, workspace, clients) = state_with_clients(3);
        state.workspaces[workspace.0].layout = Layout::Monocle;
        state.workspaces[workspace.0].selected = Some(clients[2]);
        let monitor =
            state.monitor_register(Monitor::new(1, Rect::from_xywh(0, 0, 1920, 1080), 13));
        state.workspaces[workspace.0].monitor = Some(monitor);
        state.monitors[monitor.0].workspace = Some(workspace);
        let selected_title = state.clients[clients[2].0]
            .as_ref()
            .unwrap()
            .data
            .title
            .clone();
        state.arrange_workspace(workspace);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[0]));
        assert_eq!(
            state.clients[clients[0].0].as_ref().unwrap().data.title,
            selected_title
        );
        assert!(state.clients[clients[0].0].as_ref().unwrap().is_visible);
    }

    #[test]
    fn saved_layout_tracks_client_data_not_node_identity() {
        let (mut state, workspace, clients) = state_with_clients(3);
        state.workspaces[workspace.0].selected = Some(clients[1]);
        state.workspace_save_layout(workspace);
        state.monacle_layout_select_next_client(workspace);
        assert_eq!(
            state.clients[clients[0].0].as_ref().unwrap().data.title,
            "1"
        );
        state.workspace_restore_saved_layout(workspace);
        let titles: Vec<_> = clients
            .iter()
            .map(|id| state.clients[id.0].as_ref().unwrap().data.title.as_str())
            .collect();
        assert_eq!(titles, ["0", "1", "2"]);
        assert_eq!(state.workspaces[workspace.0].selected, Some(clients[1]));
    }
}
