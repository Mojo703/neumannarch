pub fn titled(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_shown_with_its_first_letter_upper_case() {
        assert_eq!(titled("shipyard"), "Shipyard");
        assert_eq!(titled("Turtle"), "Turtle");
        assert_eq!(titled(""), "");
    }
}
