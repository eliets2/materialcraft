/// A menu entry with an optional submenu and shortcut
#[derive(Clone, Copy, Debug)]
pub struct MenuItem {
    pub menu: &'static str,
    pub sub: Option<&'static str>,
    pub label: &'static str,
    pub id: &'static str,
    pub shortcut: Option<&'static str>,
}
