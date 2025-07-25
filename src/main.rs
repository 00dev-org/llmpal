use std::{
    env, fs,
};

const OPEN_ROUTER_URL: &str = "https://api.fireworks.ai/inference/v1/chat/completions";
const MODEL: &str = "accounts/fireworks/models/kimi-k2-instruct";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} file1 file2 ... \"instruction\"", args[0]);
        std::process::exit(1);
    }

    let open_router_key_res = env::var("OPEN_ROUTER_KEY");
    if open_router_key_res.is_err() {
        eprintln!("Missing OPEN_ROUTER_KEY env variable");
        std::process::exit(1);
    }
    let open_router_key = open_router_key_res.clone().unwrap();

    let instr = args.last().unwrap();
    let files = &args[1..args.len() - 1];

    let mut prompt = String::new();
    prompt.push_str("# Instructions\nApply following changes to files: '");
    prompt.push_str(instr);
    prompt.push_str(
        "'\nOutput a comment with brief information about changes.\n\
         Then output modified files.\n\
         Do not make unnecessary changes in files. \
         Do not change file formatting (spaces, tabs, etc.)\n\n",
    );
    prompt.push_str(
        "# Output format - example\n\
         === COMMENT START ===\n\
         comments about changes made\n\
         === COMMENT END ===\n\
         === file1.txt === START ===\n\
         edited file\n\
         === file1.txt === END ===\n\
         === file2.txt === START ===\n\
         edited file\n\
         === file2.txt === END ===\n\n",
    );

    prompt.push_str("# Input files to modify:\n");
    for f in files {
        let content = fs::read_to_string(f).unwrap_or_else(|_| {
            eprintln!("Error: cannot read {}", f);
            std::process::exit(1);
        });
        prompt.push_str("=== ");
        prompt.push_str(f);
        prompt.push_str(" === START ===\n");
        prompt.push_str(&content);
        prompt.push_str("\n=== ");
        prompt.push_str(f);
        prompt.push_str(" === END ===\n");
    }

    let body = format!(
        r#"{{"model":"{MODEL}","messages":[{{"role":"user","content":"{}"}}]}}"#,
        prompt.replace('"', r#"\""#).replace('\n', r#"\n"#)
    );

    println!("Req body::: {}", body);

    let client = reqwest::Client::new();
    let response = client
        .post(OPEN_ROUTER_URL)
        .header("Authorization", format!("Bearer {}", open_router_key))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap();

    let status_code = response.status();
    if !status_code.is_success() {
        let error_text = response.text().await.unwrap();
        panic!("API request failed with status {}: {}", status_code, error_text);
    }

    let res: serde_json::Value = response
        .json()
        .await
        .unwrap_or_else(|e| panic!("Failed to parse JSON response: {}", e));

    println!("Resp body::: {}", res);

    let resp_text = res["choices"][0]["message"]["content"]
        .as_str()
        .unwrap()
        .to_string();

    let mut in_comment = false;
    let mut in_file = false;
    let mut current_path = String::new();
    let mut current_file = Vec::new();

    for lyne in resp_text.lines() {
        if lyne.trim() == "=== COMMENT START ===" {
            in_comment = true;
            continue;
        }
        if lyne.trim() == "=== COMMENT END ===" {
            in_comment = false;
            continue;
        }
        if in_comment {
            println!("{}", lyne);
        }

        if lyne.starts_with("=== ") && lyne.ends_with(" === START ===") {
            in_file = true;
            let p = &lyne[4..lyne.len() - 14];
            current_path = p.to_string();
            current_file.clear();
            continue;
        }
        if lyne.starts_with("=== ") && lyne.ends_with(" === END ===") {
            in_file = false;
            fs::write(&current_path, &current_file.join("\n")).unwrap_or_else(|_| {
                eprintln!("Error: writing {}", current_path);
                std::process::exit(1);
            });
            continue;
        }
        if in_file {
            current_file.push(lyne);
        }
    }
}