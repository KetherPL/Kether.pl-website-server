// SPDX-License-Identifier: GPL-3.0-only

const MAX_OPTIONS: usize = 10;

#[derive(Debug)]
pub(super) enum PollKind {
    YesNo { question: String },
    Malformed { question: String, option: String },
    Multi { question: String, options: Vec<String> },
}

fn usage_hint() -> &'static str {
    "!poll <question> | !poll <question> -o <opt1> -o <opt2> [...]"
}

pub(super) fn parse_poll_args(args: &str) -> Result<PollKind, String> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return Err(format!("Usage: {}", usage_hint()));
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let mut question_tokens: Vec<&str> = Vec::new();
    let mut options: Vec<String> = Vec::new();
    let mut current_option_tokens: Vec<&str> = Vec::new();
    let mut in_options = false;

    for token in tokens {
        if token == "-o" {
            if !in_options {
                in_options = true;
            } else {
                if current_option_tokens.is_empty() {
                    return Err(format!(
                        "Each -o must be followed by an option. Usage: {}",
                        usage_hint()
                    ));
                }
                options.push(current_option_tokens.join(" "));
                if options.len() > MAX_OPTIONS {
                    return Err(format!(
                        "Too many options (max {}). Usage: {}",
                        MAX_OPTIONS,
                        usage_hint()
                    ));
                }
                current_option_tokens.clear();
            }
            continue;
        }

        if in_options {
            current_option_tokens.push(token);
        } else {
            question_tokens.push(token);
        }
    }

    if question_tokens.is_empty() {
        return Err(format!("Question cannot be empty. Usage: {}", usage_hint()));
    }

    if in_options {
        if current_option_tokens.is_empty() {
            return Err(format!(
                "Each -o must be followed by an option. Usage: {}",
                usage_hint()
            ));
        }
        options.push(current_option_tokens.join(" "));
        if options.len() > MAX_OPTIONS {
            return Err(format!(
                "Too many options (max {}). Usage: {}",
                MAX_OPTIONS,
                usage_hint()
            ));
        }
    }

    let question = question_tokens.join(" ");
    match options.len() {
        0 => Ok(PollKind::YesNo { question }),
        1 => Ok(PollKind::Malformed {
            question,
            option: options.remove(0),
        }),
        _ => Ok(PollKind::Multi { question, options }),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_poll_args, PollKind};

    #[test]
    fn test_parse_yes_no_question() {
        let parsed = parse_poll_args("Should we block friendly fire between survivors?").unwrap();
        match parsed {
            PollKind::YesNo { question } => {
                assert_eq!(question, "Should we block friendly fire between survivors?");
            }
            _ => panic!("Expected yes/no poll"),
        }
    }

    #[test]
    fn test_parse_single_option_malformed() {
        let parsed = parse_poll_args("Tank burn time? -o 150s").unwrap();
        match parsed {
            PollKind::Malformed { question, option } => {
                assert_eq!(question, "Tank burn time?");
                assert_eq!(option, "150s");
            }
            _ => panic!("Expected malformed poll"),
        }
    }

    #[test]
    fn test_parse_multi_option_poll() {
        let parsed = parse_poll_args("Tank burn time? -o 120s -o 150s -o 180s").unwrap();
        match parsed {
            PollKind::Multi { question, options } => {
                assert_eq!(question, "Tank burn time?");
                assert_eq!(options, vec!["120s", "150s", "180s"]);
            }
            _ => panic!("Expected multi-option poll"),
        }
    }

    #[test]
    fn test_parse_option_with_spaces() {
        let parsed = parse_poll_args("What map? -o No Mercy -o The Parish").unwrap();
        match parsed {
            PollKind::Multi { question, options } => {
                assert_eq!(question, "What map?");
                assert_eq!(options, vec!["No Mercy", "The Parish"]);
            }
            _ => panic!("Expected multi-option poll"),
        }
    }

    #[test]
    fn test_parse_empty_question() {
        let err = parse_poll_args("-o yes -o no").unwrap_err();
        assert!(err.contains("Question cannot be empty"));
    }

    #[test]
    fn test_parse_empty_option_segment() {
        let err = parse_poll_args("Question -o yes -o").unwrap_err();
        assert!(err.contains("Each -o must be followed by an option"));
    }

    #[test]
    fn test_parse_too_many_options() {
        let err = parse_poll_args("Q -o 1 -o 2 -o 3 -o 4 -o 5 -o 6 -o 7 -o 8 -o 9 -o 10 -o 11")
            .unwrap_err();
        assert!(err.contains("Too many options"));
    }
}
