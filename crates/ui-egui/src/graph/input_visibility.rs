pub(crate) fn count(connections: impl IntoIterator<Item = bool>, minimum: usize) -> usize {
    let mut visible = minimum;
    let mut total = 0;
    for (index, connected) in connections.into_iter().enumerate() {
        total = index + 1;
        if connected {
            visible = index + 2;
        }
    }
    visible.min(total)
}
