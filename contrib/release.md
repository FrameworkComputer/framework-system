# New Release

1. Make sure to update all versions in Cargo.toml and Cargo.lock files
2. Tag
3. Download binaries from github action run on tag
4. Sign windows exe with EV cert
5. Create release on GitHub with release notes and upload binaries
6. Notify distribution maintainers (See README)
7. Do winget release (See below)
8. Do crates.io release (See below)
9. Do freebsd release (See below)

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

## FreeBSD

```
# One time
git clone https://github.com/freebsd/freebsd-ports
cd freebsd-ports/sysutils/framework-system
sudo chown -R $(whoami) /var/db/ports

cd sysutils/framework-system

git checkout -b framework-system-v0.5.0

# Edit DISTVERSION=0.5.0 and remove PORTREVISION
nvim sysutils/framework-system/Makefile

# Generate the hash of the package source
make makesum

# Regenerate the dependency information
make cargo-crates > Makefile.crates

# Generate the hash of the package dependencies
make makesum

# Build
make -V BUILD_DEPENDS
make clean
make BATCH=yes build

# Checking if everything is ok
make package
make stage-qa
make check-plist
```
