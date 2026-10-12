pub enum InspectorAction {
    OpenInMlvApp,
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
