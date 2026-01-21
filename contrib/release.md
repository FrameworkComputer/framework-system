# New Release

1. Tag
2. Download binaries from github action run on tag
3. Sign windows exe with EV cert
4. Create release with release notes and upload binaries
5. Notify distribution maintainers (See README)

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
