pub enum GridAction {
    Select(usize),
    OpenPlayer(usize),
    TogglePick(i64),
    SetRating(i64, u8),
    OpenInMlvApp(usize),
}

pub struct GridView {
    pub card_size: f32,
}

impl Default for GridView {
    fn default() -> Self {
        Self { card_size: 260.0 }
    }
}
