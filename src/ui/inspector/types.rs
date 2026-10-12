pub enum InspectorAction {
    OpenInMlvApp,
    BatchSetRating(u8),
    BatchSetFlag(crate::db::PickFlag),
    BatchAddTag(String),
    BatchRemoveTag(String),
    BatchSetNotes(String),
    BatchSetSlate {
        scene: Option<String>,
        shot: Option<String>,
        take: Option<String>,
        location: Option<String>,
        camera_unit: Option<String>,
    },
    BatchSetStretch(f32),
}

pub struct Inspector {
    pub new_tag_input: String,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            new_tag_input: String::new(),
        }
    }
}
