use crate::{core::Node, ui::lang::Language};

pub struct Store {
    pub current: usize,
    pub selected: usize,
    pub selected_category: usize,
    pub only_junk: bool,
    pub junk_only: bool,
    pub largest_files: bool,
    pub trashed: u64,
    pub confirm_delete: bool,
    pub status: String,
    pub focus_right: bool,
    pub menu_open: bool,
    pub menu_selected: usize,
    pub lang: Language,
}
impl Store {
    pub fn new(_: &[Node], lang: Language) -> Self {
        Self {
            current: 0,
            selected: 0,
            selected_category: 0,
            only_junk: false,
            junk_only: false,
            largest_files: false,
            trashed: 0,
            confirm_delete: false,
            status: String::new(),
            focus_right: true,
            menu_open: false,
            menu_selected: 0,
            lang,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Store;
    use crate::ui::lang::Language;

    #[test]
    fn disk_list_is_focused_on_startup() {
        assert!(Store::new(&[], Language::En).focus_right);
    }
}
