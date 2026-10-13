pub enum InspectorAction {
    OpenInMlvApp,
    BatchSetRating(u8),
    BatchSetFlag(crate::db::PickFlag),
    BatchAddTag(String),
    BatchRemoveTag(String),
    CreateKeywordWithParent { name: String, parent_id: Option<i64> },
    OpenKeywordManager,
    FilterByTag(String),
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
    pub new_tag_parent_id: Option<i64>,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            new_tag_input: String::new(),
            new_tag_parent_id: None,
        }
    }
}
