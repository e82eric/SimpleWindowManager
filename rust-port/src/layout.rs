use crate::model::{Monitor, Rect, Workspace};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layout {
    Grid,
    Deck,
    HorizontalDeck,
    Monocle,
    TileReversed,
    #[default]
    Tile,
}

impl Layout {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Grid => "Q",
            Self::Deck => "D",
            Self::HorizontalDeck => "D",
            Self::Monocle => "M",
            Self::TileReversed => "RT",
            Self::Tile => "T",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Tile => Self::TileReversed,
            Self::TileReversed => Self::Monocle,
            Self::Monocle => Self::Deck,
            Self::Deck => Self::Grid,
            Self::Grid | Self::HorizontalDeck => Self::Tile,
        }
    }

    pub fn calculate(self, workspace: &Workspace, monitor: &Monitor) -> Vec<(Rect, bool)> {
        let count = workspace.clients.len();
        match self {
            Self::Tile => tile_layout_calculate(monitor, workspace.main_offset, count, false),
            Self::TileReversed => {
                tile_layout_calculate(monitor, workspace.main_offset, count, true)
            }
            Self::Deck => deck_layout_calculate(monitor, workspace.main_offset, count, false),
            Self::HorizontalDeck => {
                deck_layout_calculate(monitor, workspace.main_offset, count, true)
            }
            Self::Monocle => monocle_layout_calculate(monitor, count),
            Self::Grid => grid_layout_calculate(monitor, workspace.main_offset, count),
        }
    }
}

fn tile_layout_calculate(
    monitor: &Monitor,
    main_offset: i32,
    count: usize,
    reversed: bool,
) -> Vec<(Rect, bool)> {
    if count == 0 {
        return Vec::new();
    }
    let gap = monitor.gap_width;
    let screen_height = monitor.bottom - monitor.top;
    let main_width = if count == 1 {
        monitor.width - gap * 2
    } else {
        monitor.width / 2 - gap - gap / 2 + main_offset
    };
    let tile_width = if count == 1 {
        0
    } else {
        monitor.width / 2 - gap - gap / 2 - main_offset
    };
    let main_height = screen_height - gap * 2;
    let tile_height = if count < 3 {
        main_height
    } else {
        (main_height - (count as i32 - 2) * gap) / (count as i32 - 1)
    };
    let main_y = monitor.top + gap;
    let (main_x, tile_x) = if reversed && count > 1 {
        (
            monitor.x_offset + tile_width + gap * 2,
            monitor.x_offset + gap,
        )
    } else {
        (
            monitor.x_offset + gap,
            monitor.x_offset + main_width + gap * 2,
        )
    };
    let mut tile_y = main_y;
    (0..count)
        .map(|index| {
            if index == 0 {
                (
                    Rect::from_xywh(main_x, main_y, main_width, main_height),
                    true,
                )
            } else {
                let rect = Rect::from_xywh(tile_x, tile_y, tile_width, tile_height);
                tile_y += tile_height + gap;
                (rect, true)
            }
        })
        .collect()
}

fn deck_layout_calculate(
    monitor: &Monitor,
    main_offset: i32,
    count: usize,
    horizontal: bool,
) -> Vec<(Rect, bool)> {
    if count == 0 {
        return Vec::new();
    }
    let gap = monitor.gap_width;
    let (main, secondary) = if horizontal {
        let available = monitor.bottom - monitor.top - gap * 2;
        let main_height = if count == 1 {
            available
        } else {
            available / 2 - gap / 2 + main_offset
        };
        let secondary_height = if count == 1 {
            0
        } else {
            available / 2 - gap / 2 - main_offset
        };
        let x = monitor.x_offset + gap;
        (
            Rect::from_xywh(x, monitor.top + gap, monitor.width - gap * 2, main_height),
            Rect::from_xywh(
                x,
                monitor.top + gap + main_height + gap,
                monitor.width - gap * 2,
                secondary_height,
            ),
        )
    } else {
        let height = monitor.bottom - monitor.top - gap * 2;
        let main_width = if count == 1 {
            monitor.width - gap * 2
        } else {
            monitor.width / 2 - gap - gap / 2 + main_offset
        };
        let secondary_width = if count == 1 {
            0
        } else {
            monitor.width / 2 - gap - gap / 2 - main_offset
        };
        (
            Rect::from_xywh(
                monitor.x_offset + gap,
                monitor.top + gap,
                main_width,
                height,
            ),
            Rect::from_xywh(
                monitor.x_offset + main_width + gap * 2,
                monitor.top + gap,
                secondary_width,
                height,
            ),
        )
    };
    (0..count)
        .map(|index| (if index == 0 { main } else { secondary }, index < 2))
        .collect()
}

fn monocle_layout_calculate(monitor: &Monitor, count: usize) -> Vec<(Rect, bool)> {
    let gap = monitor.gap_width;
    let rect = Rect::from_xywh(
        monitor.x_offset + gap,
        monitor.top + gap,
        monitor.width - gap * 2,
        monitor.bottom - monitor.top - gap * 2,
    );
    (0..count).map(|index| (rect, index == 0)).collect()
}

fn grid_layout_calculate(monitor: &Monitor, main_offset: i32, count: usize) -> Vec<(Rect, bool)> {
    if count == 0 {
        return Vec::new();
    }
    let gap = monitor.gap_width;
    let all_y = monitor.top + gap;
    let all_height = monitor.bottom - monitor.top - gap * 2;
    if count == 1 {
        return vec![(
            Rect::from_xywh(
                monitor.x_offset + gap,
                all_y,
                monitor.width - gap * 2,
                all_height,
            ),
            true,
        )];
    }
    let left_count = count / 2;
    let right_count = count - left_count;
    let left_width = monitor.width / 2 - gap - gap / 2 + main_offset;
    let right_width = monitor.width / 2 - gap - gap / 2 - main_offset;
    let right_x = monitor.x_offset + left_width + gap * 2;
    let left_height = (all_height - (left_count as i32 - 1) * gap) / left_count as i32;
    let right_height = (all_height - (right_count as i32 - 1) * gap) / right_count as i32;
    (0..count)
        .map(|index| {
            if index < left_count {
                (
                    Rect::from_xywh(
                        monitor.x_offset + gap,
                        all_y + index as i32 * (left_height + gap),
                        left_width,
                        left_height,
                    ),
                    true,
                )
            } else {
                let row = index - left_count;
                (
                    Rect::from_xywh(
                        right_x,
                        all_y + row as i32 * (right_height + gap),
                        right_width,
                        right_height,
                    ),
                    true,
                )
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ClientId, WorkspaceFilterData};

    fn monitor() -> Monitor {
        Monitor::new(0, Rect::from_xywh(0, 0, 1920, 1080), 10)
    }
    fn workspace(count: usize, layout: Layout) -> Workspace {
        Workspace {
            name: "test".into(),
            tag: "T".into(),
            is_icon: false,
            clients: (0..count).map(ClientId).collect(),
            minimized_clients: vec![],
            monitor: None,
            selected: None,
            main_offset: 0,
            layout,
            filter_data: WorkspaceFilterData::default(),
            saved_layout: vec![],
            saved_selected: None,
        }
    }

    #[test]
    fn tile_matches_c_integer_geometry() {
        let rects = Layout::Tile.calculate(&workspace(3, Layout::Tile), &monitor());
        assert_eq!(rects[0].0, Rect::from_xywh(10, 10, 945, 1060));
        assert_eq!(rects[1].0, Rect::from_xywh(965, 10, 945, 525));
        assert_eq!(rects[2].0, Rect::from_xywh(965, 545, 945, 525));
    }
    #[test]
    fn deck_only_exposes_main_and_top_of_stack() {
        let rects = Layout::Deck.calculate(&workspace(4, Layout::Deck), &monitor());
        assert_eq!(
            rects.iter().map(|item| item.1).collect::<Vec<_>>(),
            vec![true, true, false, false]
        );
    }
    #[test]
    fn monocle_only_exposes_main() {
        let rects = Layout::Monocle.calculate(&workspace(3, Layout::Monocle), &monitor());
        assert_eq!(
            rects.iter().map(|item| item.1).collect::<Vec<_>>(),
            vec![true, false, false]
        );
    }
}
