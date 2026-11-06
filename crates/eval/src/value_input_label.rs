pub fn value_input_label(identifier: &str) -> String {
    let mut label = String::with_capacity(identifier.len());
    let mut previous = None;

    for character in identifier.chars() {
        if character == '_' || character == '-' {
            if !label.is_empty() && !label.ends_with(' ') {
                label.push(' ');
            }
            previous = None;
            continue;
        }

        if previous
            .is_some_and(|previous: char| (previous.is_alphabetic() && character.is_ascii_digit()) || (previous.is_lowercase() && character.is_uppercase()))
        {
            label.push(' ');
        }

        if label.is_empty() || label.ends_with(' ') {
            label.extend(character.to_uppercase());
        } else {
            label.push(character);
        }
        previous = Some(character);
    }

    label.truncate(label.trim_end().len());
    label
}
