pub fn build_system_prompt(allowed_files: &[String], rules: &[String]) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You are a non-interactive agent specialized in helping with software development tasks.\n\
        # Primary workflows:\n\
        - answering questions\n\
        - analyzing and explaining contents of provided files\n\
        - editing and creating files\n\
        - writing and modifying code\n\n\
        # Guidelines\n\
        - You are allowed to provide content only for these files:\n",
    );

    for file in allowed_files {
        prompt.push_str(&format!("- {}\n", file));
    }

    prompt.push_str(
        "- When you need to create another file to fulfill the user's task, let the user know why and provide a path to the file.\n\
        - Never create or modify any files when the user is only asking questions.\n\
        - When asked to modify a file, provide **full** contents of the file after modification.\n\
        - Always provide a brief explanation for your actions.\n\
        - Always omit files that need no changes.\n\
        - You MUST strictly follow the defined output format. Never deviate from it.\n\
        - Never output additional information outside of the defined schema.\n\
        - Never provide partial files in outputs.\n\
        - Never add any comments to the code, unless you are directly asked to do so.\n\
        - Never make unrequested changes in files.\n\
        - Never add code comments when not requested.\n\
        - Never change file formatting (spaces, tabs, etc.). New code should have formatting and style consistent with existing code.\n\n",
    );

    if !rules.is_empty() {
        prompt.push_str("# Additional rules\n");
        for rule in rules {
            prompt.push_str(&format!("- {}\n", rule));
        }
        prompt.push_str("\n");
    }

    prompt.push_str(
        "# Output format\n\
         You must follow this output format exactly. Deviations will be rejected.\n\
         The response must start with:\n\
         === EXPLAIN START ===\n\
         Brief explanations and answers to questions\n\
         === EXPLAIN END ===\n\
         Then, for each file you are modifying or creating:\n\
         === filename === START ===\n\
         full file content\n\
         === filename === END ===\n\n\
         Example:\n\
         === EXPLAIN START ===\n\
         I'm updating the build_system_prompt to reinforce format compliance.\n\
         === EXPLAIN END ===\n\
         === src/llm.rs === START ===\n\
         updated content of the file\n\
         === src/llm.rs === END ===\n\n",
    );

    prompt
}

pub fn build_user_prompt(
    instruction: &str,
    files: &[String],
    output_file: &Option<String>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("# User instructions\n");
    prompt.push_str(instruction);

    prompt.push_str("\n\n");
    prompt.push_str("# User input files:\n");

    for f in files {
        if let Some(output) = output_file {
            if f == output {
                continue;
            }
        }

        let content = if cfg!(test) {
            String::new()
        } else {
            std::fs::read_to_string(f).unwrap_or_else(|_| {
                eprintln!("Error: cannot read file '{}': No such file or directory", f);
                std::process::exit(1);
            })
        };
        prompt.push_str(&format!(
            "=== {} === START ===\n\
             {}\
             \n=== {} === END ===\n",
            f, content, f
        ));
    }

    prompt
}

pub fn parse_llm_response(
    resp_text: &str,
) -> Result<(String, Vec<(String, String)>, String), String> {
    let mut in_comment = false;
    let mut in_file = false;
    let mut in_think = false;
    let mut current_path = String::new();
    let mut current_file = Vec::new();
    let mut files_to_write = Vec::new();
    let mut comments = Vec::new();
    let mut remaining = Vec::new();

    for line in resp_text.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("<think>") {
            in_think = true;
            continue;
        }
        if trimmed.starts_with("</think>") {
            in_think = false;
            continue;
        }
        if in_think {
            continue;
        }

        if trimmed == "=== EXPLAIN START ===" {
            in_comment = true;
            continue;
        }
        if trimmed == "=== EXPLAIN END ===" {
            in_comment = false;
            continue;
        }
        if in_comment {
            comments.push(line.to_string());
            continue;
        }
        if line.starts_with("=== ") && line.ends_with(" === START ===") {
            in_file = true;
            let p = &line[4..line.len() - 14];
            current_path = p.to_string();
            current_file.clear();
            continue;
        }
        if line.starts_with("=== ") && line.ends_with(" === END ===") {
            in_file = false;
            if !current_path.is_empty() {
                files_to_write.push((current_path.clone(), current_file.join("\n")));
            }
            continue;
        }
        if in_file {
            current_file.push(line.to_string());
        } else {
            remaining.push(line.to_string());
        }
    }

    if in_file {
        return Err("Error: unexpected end of response while parsing a file section".to_string());
    }

    Ok((comments.join("\n"), files_to_write, remaining.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_user_prompt_empty_files() {
        let instruction = "test";
        let files = vec![];
        let prompt = build_user_prompt(instruction, &files, &None);
        assert!(prompt.contains("# User instructions"));
        assert!(prompt.contains("test"));
        assert!(prompt.contains("# User input files:"));
    }

    #[test]
    fn test_build_system_prompt_with_files() {
        let allowed_files = vec!["file1.rs".to_string()];
        let rules = vec![];
        let prompt = build_system_prompt(&allowed_files, &rules);
        assert!(prompt.contains("file1.rs"));
        assert!(prompt.contains("When you need to create another file"));
    }
}
