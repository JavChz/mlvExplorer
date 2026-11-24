pub enum GridAction {
    Select { index: usize, extend: bool, toggle: bool },
    OpenPlayer(usize),
    TogglePick(i64),
    SetRating(i64, u8),
    OpenInMlvApp(Vec<usize>),
    ShowInExplorer(usize),
    RequestRemoveFromCatalog(Vec<usize>),
    RequestMoveToTrash(Vec<usize>),
    BoxSelect(Vec<usize>),
    DeselectAll,
    SortChanged,
}

pub struct GridView {
    pub card_size: f32,
    pub group_by_day: bool,
    pub drag_start_pos: Option<egui::Pos2>,
    pub drag_current_pos: Option<egui::Pos2>,
    pub scrollbar_tooltip_rect: Option<egui::Rect>,
    pub last_columns: usize,
    pub scroll_to_index: Option<usize>,
}

impl Default for GridView {
    fn default() -> Self {
        Self {
            card_size: 260.0,
            group_by_day: true,
            drag_start_pos: None,
            drag_current_pos: None,
            scrollbar_tooltip_rect: None,
            last_columns: 4,
            scroll_to_index: None,
        }
    }
}
