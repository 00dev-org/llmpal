pub fn build_system_prompt(allowed_files: &[String], rules: &[String]) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "Follow user instructions. When asked to make changes, apply changes to given files. \n\
        When asked to create a file, create it. When asked questions, just answer them without creating or modifying files.\n\
        When changing files, output an explanation with brief and blunt information about changes.\n\
        Then output modified files. Always output full contents of changed files.\n\
        Never propose to output files other than the allowed ones:\n",
    );

    for file in allowed_files {
        prompt.push_str(&format!(" {}\n", file));
    }

    prompt.push_str(
        "When the task requires creating files and you are not allowed to create them, mention the issue in the comments section.\n\
        When asked to create a new file, output to a new file or extract something from other files - only use allowed files.\n\
        When asked to explain code, answer questions, suggest changes or improvements - output only in the EXPLAIN section without modifying files. \n\
        Never explain stuff by adding comments to the code unless directly asked to do so.\n\
        Do not make unnecessary changes in files. Do not add code comments when not requested. Omit files that need no changes. \n\
        Always use defined output format. Do not output additional information outside of defined schema. \n\
        Do not change file formatting (spaces, tabs, etc.). New code should have formatting and style consistent with existing code.\n\n",
    );

    prompt.push_str(
        "# Output format - example\n\
         === EXPLAIN START ===\n\
         Brief explanations and answers to questions\n\
         === EXPLAIN END ===\n\
         === file1.txt === START ===\n\
         edited file\n\
         === file1.txt === END ===\n\
         === file2.txt === START ===\n\
         edited file\n\
         === file2.txt === END ===\n\n",
    );

    if !rules.is_empty() {
        prompt.push_str("=== RULES START ===\n");
        for rule in rules {
            prompt.push_str(&format!("{}\n", rule));
        }
        prompt.push_str("=== RULES END ===\n");
    }

    prompt
}

pub fn build_user_prompt(
    instruction: &str,
    files: &[String],
    output_file: &Option<String>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str("=== USER INSTRUCTIONS START\n");
    prompt.push_str(instruction);
    prompt.push_str("\n=== USER INSTRUCTIONS END\n\n");
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
        assert!(prompt.contains("=== USER INSTRUCTIONS START"));
        assert!(prompt.contains("test"));
        assert!(prompt.contains("=== USER INSTRUCTIONS END"));
        assert!(prompt.contains("# User input files:"));
    }

    #[test]
    fn test_build_system_prompt_with_files() {
        let allowed_files = vec!["file1.rs".to_string()];
        let rules = vec![];
        let prompt = build_system_prompt(&allowed_files, &rules);
        assert!(prompt.contains("file1.rs\nWhen the task requires creating files"));
    }
}
