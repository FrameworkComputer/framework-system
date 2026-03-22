# Shell Completions

Shell completions for `framework_tool` are auto-generated using `clap_complete`.

## Regenerating

If you modify the CLI arguments, regenerate completions:

```bash
cargo build
./target/debug/framework_tool --generate-completions bash > framework_tool/completions/bash/framework_tool
./target/debug/framework_tool --generate-completions zsh > framework_tool/completions/zsh/_framework_tool
./target/debug/framework_tool --generate-completions fish > framework_tool/completions/fish/framework_tool.fish
```

## Testing

`framework_tool` must be in your PATH for completions to work.

**Bash:**
```bash
export PATH="$PWD/target/debug:$PATH"
source framework_tool/completions/bash/framework_tool
framework_tool --<TAB>
```

**Zsh:**
```zsh
export PATH="$PWD/target/debug:$PATH"
source framework_tool/completions/zsh/_framework_tool
framework_tool --<TAB>
```

**Fish:**
```fish
fish_add_path $PWD/target/debug
source framework_tool/completions/fish/framework_tool.fish
framework_tool --<TAB>
```

**PowerShell:**
```powershell
framework_tool --generate-completions powershell | Invoke-Expression
framework_tool --<TAB>
```

## Persistent Installation

Linux: Should be done by downstream package maintainers.

Windows/PowerShell:
```powershell
framework_tool --generate-completions powershell >> $PROFILE
```
