# New Release

1. Make sure to update all versions in Cargo.toml and Cargo.lock files
2. Tag
3. Download binaries from github action run on tag
4. Sign windows exe with EV cert
5. Create release on GitHub with release notes and upload binaries
6. Notify distribution maintainers (See README)
7. Do winget release (See below)
8. Do crates.io release (See below)

## winget

See: https://learn.microsoft.com/en-us/windows/package-manager/package/repository

```
# Example with version 0.5.0

# Create new manifest
wingetcreate update FrameworkComputer.framework_tool -u https://github.com/FrameworkComputer/framework-system/releases/download/v0.5.0/framework_tool.exe -v 0.5.0

winget validate .\manifests\f\FrameworkComputer\framework_tool\0.5.0

# Launches a new window where it installed the command
# Try out some things that don't need hardware access
powershell .\Tools\SandboxTest.ps1 .\manifests\f\FrameworkComputer\framework_tool\0.5.0

winget install --manifest .\manifests\f\FrameworkComputer\framework_tool\0.5.0

git add manifests/f/FrameworkComputer/framework_tool/0.5.0
git commit -sm "New version: FrameworkComputer.framework_tool version 0.5.0"
```

## crates.io

See: https://doc.rust-lang.org/cargo/reference/publishing.html

Dry run and review included files

```
cargo publish -p framework_lib --dry-run
cargo publish -p framework_tool --dry-run
cargo package list -p framework_lib
cargo package list -p framework_tool
```

Publish

```
cargo publish -p framework_lib
cargo publish -p framework_tool
```
