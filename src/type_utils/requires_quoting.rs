pub fn requires_quoting(
    name: &str,
    // target: ts.ScriptTarget = ts.ScriptTarget.ESNext,
) -> bool {
    // TODO: actual logic
    name.starts_with('#')
}
