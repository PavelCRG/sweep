use crate::core::Node;

pub struct Store {
    pub current: usize,
    pub selected: usize,
    pub only_junk: bool,
    pub confirm_delete: bool,
    pub status: String,
    pub focus_right: bool,
}
impl Store {
    pub fn new(_: &[Node]) -> Self {
        Self {
            current: 0,
            selected: 0,
            only_junk: false,
            confirm_delete: false,
            status: String::new(),
            focus_right: false,
        }
    }
}
