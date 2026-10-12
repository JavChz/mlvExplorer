pub enum GridAction {
    Select(usize),
    OpenPlayer(usize),
    TogglePick(i64),
    SetRating(i64, u8),
    OpenInMlvApp(usize),
    SortChanged,
}

pub struct GridView {
    pub card_size: f32,
    pub group_by_day: bool,
}

impl Default for GridView {
    fn default() -> Self {
        Self {
            card_size: 260.0,
            group_by_day: true,
        }
    }
}
