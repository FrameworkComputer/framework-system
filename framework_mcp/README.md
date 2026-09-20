# framework_mcp

An [MCP](https://modelcontextprotocol.io) server that gives AI assistants
read-only access to the hardware state of a Framework Computer system:
battery and charger, USB-C PD ports, temperatures and fans, sensors,
switches, firmware versions and Embedded Controller diagnostics.

It is built on the same `framework_lib` as `framework_tool`, so every tool
returns the data the matching `framework_tool` command shows, as JSON.

## Tools

| Tool                 | framework_tool equivalent |
|----------------------|---------------------------|
| `system_snapshot`    | everything below at once, for diagnosing a problem |
| `platform`           | `--info` (which system this is) |
| `versions`           | `--versions` (BIOS, EC, PD controllers) |
| `power_status`       | `--power` |
| `charge_limit`       | `--charge-limit` |
| `thermal`            | `--thermal` |
| `thermal_thresholds` | `--thermalget` |
| `sensors`            | `--sensors` |
| `switches`           | `--switches` |
| `chassis_intrusion`  | `--intrusion` |
| `privacy_switches`   | `--privacy` |
| `keyboard_backlight` | `--kblight` |
| `pd_ports`           | `--pdports` |
| `pd_power_info`      | `--pdports-chromebook` |
| `ec_sysinfo`         | `--sysinfo` |
| `ec_uptime`          | `--uptimeinfo` |
| `ec_features`        | `--features` |
| `inputdeck`          | `--inputdeck` |
| `expansion_bay`      | `--expansion-bay` (Laptop 16) |
| `ec_panic_info`      | `--panicinfo` |

All tools are read-only and take no arguments. Nothing here changes any
setting on the system.

## Running

The server talks to the Embedded Controller, which needs root, and speaks
MCP over stdin/stdout. It refuses to start if it is not root or cannot reach
the EC, so the client shows a clear error. Logs go to stderr, controlled by
`RUST_LOG` (default `warn`).

```
cargo build -p framework_mcp
sudo ./target/debug/framework_mcp
```

Because MCP clients start the server themselves, `sudo` must not ask for a
password. Either run the client as root, or allow the binary in sudoers:

```
# /etc/sudoers.d/framework_mcp
yourname ALL=(root) NOPASSWD: /usr/bin/framework_mcp
```

### Claude Code

```
claude mcp add framework -- sudo /usr/bin/framework_mcp
```

### Claude Desktop and other clients

```json
{
  "mcpServers": {
    "framework": {
      "command": "sudo",
      "args": ["/usr/bin/framework_mcp"]
    }
  }
}
```

On Windows, run the client elevated and point it at `framework_mcp.exe`
directly.

## Options

```
--driver <portio|cros-ec|windows>   Force a specific EC driver
--skip-checks                       Start even if not root or the EC does not respond
```
