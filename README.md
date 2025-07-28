# llmpal

A CLI tool for interacting with Large Language Models (LLMs) for code modification and file generation. Built in Rust with support for multiple providers and models.
It is designed to work well only on a small set of files, making small improvements step by step. In such tasks, it should work relatively well on inexpensive, open-weight models like qwen/kimi.

> ⚠️ **llmpal modifies files directly**:  
> This tool overwrites files in place and may corrupt content. Always back up your files or use version control (e.g., git) before running the tool.

> ⚠️ Always review code produced by LLM, it might contain bugs that might lead to data loss. 

> **Recommended workflow**:  
> - Make a commit before using llmpal  
> - Execute llmpal command
> - Test changes and commit them if they are good
> - If changes are bad, revert them, improve the command, and try again

## Features
- Modify existing files (**restricted to those specified with the -f flag**) or create new files (**requires the -o flag**) using LLM instructions
- Supports multiple models and providers via OpenRouter and custom endpoints
- Cost-tracking per request

## Installation
1. Install Rust toolchain: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Clone repository: `git clone https://github.com/00dev-org/llmpal`
3. Build: `cd llmpal && cargo install --path .`

## Configuration
All configurations should be in `.llmpal.json` file placed in the project root or home directory. Configuration includes three main parameters: models, rules, and diagnostic.

### Defaults
- API endpoint: `https://openrouter.ai/api/v1/chat/completions`
- API key from environment variable `OPENROUTER_API_KEY`
- Model: `moonshotai/kimi-k2`
- Max tokens: `16384`

### Configuration Structure
```json
{
  "diagnostic": true,
  "rules": [
    "Never use panic directive."
  ],
  "models": [
    {
      "code": "qwen",
      "model": "qwen/qwen3-235b-a22b-2507",
      "provider": "Fireworks",
      "prompt_cost": 0.22,
      "completion_cost": 0.88
    },
    {
      "code": "cer",
      "model": "qwen-3-235b-a22b",
      "prompt_cost": 0.0,
      "completion_cost": 0.0,
      "api_url": "https://api.cerebras.ai/v1/chat/completions",
      "api_key": "$CEREBRAS_API_KEY",
      "max_tokens": 40000
    }
  ]
}
```

### Global Options
The home directory's `.llmpal.json` can provide global configurations that merge with local project settings. When both files exist, local configuration takes precedence for conflicting fields.

### Model Configuration Fields
- `code`: Short identifier for the model (used with `-m` flag)
- `model`: Full model identifier name from the provider
- `provider`: Vendor name (e.g., "Fireworks", "Cerebras")
- `prompt_cost`: Cost per 1M prompt tokens (in USD)
- `completion_cost`: Cost per 1M completion tokens (in USD)
- `api_url`: Custom API endpoint (defaults to OpenRouter)
- `api_key`: API key reference using `$<ENV_VARIABLE_NAME>` syntax
- `max_tokens`: Maximum token limit for model (set to null for the default limit)

### Advanced Configuration
You can specify environment variables for API keys using the `$<ENV_NAME>` syntax. The tool will resolve these at runtime. For example:
```json
"api_key": "$CEREBRAS_API_KEY"
```

Model selection follows the priority:
1. CLI flag `-m <code>` specified at runtime
2. First model in config file if no `-m` flag
3. Default model `moonshotai/kimi-k2` if no config available

### Parameters Reference
- **rules**: Array of rules that appear in the LLM system prompt, influencing LLM behavior
- **diagnostic**: When true, logs LLM prompts and responses to `$HOME/.llmpal/prompt.log`

## Usage
### Important File Restrictions
The LLM is strictly limited to:
- Modifying files explicitly listed with the `-f` flag
- Creating files only when explicitly specified with the `-o` flag

Any attempt to modify files not listed with the `-f` flag or create files without the `-o` flag will trigger an error and abort the operation. For example:
#### This will fail: attempt to create a file without the `-o` flag
```bash
llmpal 'Write content to README.md'
```

### Modify files with instruction
```bash
llmpal -f src/main.rs 'Implement logging'
```
### Create new file
```bash
llmpal -o poem.md 'Write short poem'
```
### Debug output
```bash
llmpal -v --trace -f src/llm.rs 'Explain this function'
```
### Use custom model
```bash
llmpal -m qwen -o poem.txt 'Write short poem'
```
### Multi-file modification
```bash
llmpal -f src/main.rs -f src/lib.rs 'Refactor core logic'
```