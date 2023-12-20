use unicode_segmentation::UnicodeSegmentation;

pub fn get_string_length(value: &str) -> usize {
    if value.is_ascii() {
        return value.len();
    }

    value.graphemes(true).count()
}
