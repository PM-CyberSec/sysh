![Typing Effect](https://readme-typing-svg.demolab.com?font=JetBrains+Mono&weight=500&size=18&pause=2000&color=00FFCC&background=1E1E1E00&center=false&vCenter=true&width=1000&lines=user%40host%3A~%24+sysh+--virt+-n+10+%7C+sysht+--virt)

<p align="center"><img width="520" alt="sysh logo" src="docs/logo3.jpeg"></p>

# System Health — sysh / sysht

[![Bash](https://img.shields.io/badge/Bash-5.1%2B-001F4D?style=flat-square&logo=gnu-bash&logoColor=00FFCC)](https://www.gnu.org/software/bash/)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-001F4D?style=flat-square&logo=rust&logoColor=00FFCC)](https://www.rust-lang.org)
[![Linux](https://img.shields.io/badge/Linux-Kali%2FDebian-001F4D?style=flat-square&logo=linux&logoColor=00FFCC)]()
[![TUI](https://img.shields.io/badge/TUI-ratatui%200.29-001F4D?style=flat-square&logoColor=00FFCC)]()
[![Status](https://img.shields.io/badge/Status-Active-001F4D?style=flat-square&logo=target&logoColor=00FFCC)]()
[![License](https://img.shields.io/badge/License-MIT-001F4D?style=flat-square&logoColor=00FFCC)]()

Interactive Linux system health reporting with structured disk analysis and optional thin-provisioning overcommit calculation.

---

## Overview

`sysh` is a Linux-only diagnostic suite consisting of two complementary components:

- **sysh** — Bash-based CLI collector. Generates a comprehensive health report to both standard output and a log file simultaneously, with threshold-based alerts and optional JSON output for automation.
- **sysht** — Interactive terminal viewer. A Bash wrapper prepares data via `sysh` and a Rust application renders it using `ratatui` and `crossterm`. The design follows the `pdf-search` pattern where collection and presentation are separated.

The tools are intended for interactive use on workstations and servers. All potentially long-running operations are bounded by timeouts and lowered priority to ensure the report completes even when subsystems are unresponsive. macOS is not supported.

```bash
sysh                    # full report to stdout and ./sysh-YYYYmmdd_HHMMSS.log
sysh --virt             # include thin-provisioned image scan and overcommit calculation
sysht                   # interactive TUI with tabbed views
```

---

## Features

- **Dual output by default.** Every invocation writes to the terminal and to a timestamped log file. Colors are preserved on screen and stripped for the file.
- **Threshold alerts.** Disk, inode, memory, swap, CPU load, thermal, and virtual overcommit are evaluated against configurable thresholds. Results are summarized and reflected in the exit code.
- **Categorized disk analysis.** Filesystem usage is decomposed into block devices, inode statistics, largest directories by category, largest files, installed packages, and container storage.
- **Thin-provisioning awareness.** Optional `--virt` scan enumerates `qcow2`, `vdi`, `vmdk`, `vhd`, `vhdx`, and `img` images, compares virtual and actual sizes via `qemu-img`, and computes effective free space.
- **Resilient collection.** `timeout` and `ionice -c3` guard `du`, `find`, `snap`, `docker`, and `lvs` operations. Permission or timeout failures are reported inline rather than aborting the run.
- **Machine-readable output.** `--json` appends JSON alongside the human report; `--json-only` emits JSON alone for pipelines.

---

## Technology Stack

| Component | Language | Key Dependencies |
|-----------|----------|------------------|
| `sysh` | Bash 5.1+ | `coreutils` (`df`, `du`, `numfmt`, `stat`, `timeout`), `procps` (`free`, `ps`, `vmstat`), `util-linux` (`lscpu`, `lsblk`), `iproute2` (`ip`, `ss`), `systemd`, `sensors`/`upower` (optional), `qemu-img` + `python3` or `jq` for `--virt`, `lvs` for LVM thin pools |
| `sysht` wrapper | Bash | Invokes `sysh` and launches the TUI binary |
| `sysht-tui` | Rust 1.75+ | `ratatui` 0.29, `crossterm` 0.28, `clap` 4.6, `serde`/`serde_json`, `anyhow` |

The color palette is derived from the GhostTrace theme: deep teal backgrounds (`#001818`, `#001F4D`) with cyan accents (`#70F0F0` / `#00FFCC`) and muted text tones for secondary information.

---

## What It Reports

A single execution covers the following areas. Each section is rendered as a bordered table or structured block with warning and critical highlighting.

| Category | Content | Primary Sources |
|----------|---------|-----------------|
| System | Hostname, kernel, date, uptime, load averages, logged-in users, load per core | `hostname`, `uname -a`, `uptime`, `cat /proc/loadavg`, `who` |
| CPU | Core count, model, scaling frequency, 1-second utilization sample, top CPU consumers | `lscpu`, `/proc/cpuinfo`, `nproc`, `mpstat 1 1`, `ps aux --sort=-%cpu` |
| Memory | Total, used, available, swap, utilization percentage, `vmstat` summary, top memory consumers | `free -h`, `/proc/meminfo`, `vmstat -s` |
| Filesystems | Capacity, type, usage percentage with threshold coloring | `df -hT` |
| Inodes | Inode counts and utilization | `df -i` |
| Block devices | Device, size, type, mount point, filesystem | `lsblk` |
| Top directories | Largest entries at `/` and per-category breakdown for `/home`, `/var`, `/opt`, `/tmp`, and `/usr` sample | `du -sh` with `sort -rh` |
| Top files | Largest regular files under `/home`, `/var`, `/tmp` | `find -printf '%s|%p'` with `sort -nr` |
| Packages | Largest installed packages and snap/flatpak inventories | `dpkg-query` (or `rpm`/`pacman`), `snap`, `flatpak` |
| Containers | Storage usage, largest images and containers, volume list | `docker system df`, `docker images`, `docker ps` |
| I/O | Device utilization or raw disk statistics | `iostat -xz` or `/proc/diskstats` |
| Network | Interface addresses, link statistics, socket summary, listening ports, connectivity checks | `ip`, `ss`, `ping`, `/proc/net/dev` |
| Processes | Process, thread, and zombie counts, top CPU and memory tables | `ps aux`, `ps -eLf` |
| Services | System state, failed units | `systemctl` |
| Logs | Recent priority-3 journal entries, kernel warnings, authentication failures | `journalctl`, `dmesg` |
| Sensors | Hardware temperatures, thermal zones, battery and GPU status where available | `sensors`, `/sys/class/thermal`, `upower`, `nvidia-smi` |
| Virtual images (`--virt`) | Image inventory and overcommit calculation (detailed below) | `find`, `qemu-img info --output=json`, `lvs`, `virsh`, `du --apparent-size` |
| Alerts | Consolidated summary of all threshold evaluations | Internal threshold logic |

Each report concludes with generation metadata and, when `--json` is specified, a JSON document.

---

## Architecture

### Data Flow

```
sysh (Bash)
  ├── stdout (ANSI) ──► terminal (color retained)
  ├── tee ───────────► ./sysh-YYYYmmdd_HHMMSS.log (ANSI stripped)
  └── --json ───────► ./sysh-YYYYmmdd_HHMMSS.json (pretty-printed via jq when available)

sysht (Bash wrapper)
  ├── executes: bash sysh --no-file --no-color -n TOP [--virt] (bounded by timeout)
  │         └── captures /tmp/sysht-raw-<pid>.txt
  └── launches sysht-tui (Rust)
            ├── health::collect_with_virt() spawns sysh and parses output
            │     ├── raw_lines split on delimiter "━━━" into sections map
            │     └── json_overview from sysh --json-only
            └── App::render() presents sections via ratatui
```

Collection is pull-based. The TUI does not run a background daemon; it re-invokes the collector on manual refresh (`r`) or automatic interval (every 5 seconds, toggleable).

### TUI Views

| Key | View | Source Section |
|-----|------|----------------|
| `1` / `F1` | Overview | System header, alerts, and JSON summary |
| `2` / `F2` | CPU | CPU details and top consumers |
| `3` / `F3` | Memory | Memory and swap details |
| `4` / `F4` | Disk: Filesystems | Filesystem, inode, and block device tables |
| `5` / `F5` | Disk: Directories | Categorized largest directories |
| `6` / `F6` | Disk: Files | Largest files |
| `7` / `F7` | Packages | Installed packages and snap/flatpak |
| `8` / `F8` | Docker | Container storage details |
| `9` / `F9` | Network | Interface and socket information |
| `0` / `F10` | Processes | Process counts and top tables |
| `s` / `F11` | Services and Logs | Failed units, journal, and kernel messages |
| `t` / `F12` | Sensors | Temperatures, thermal zones, and power |
| `v` / `F13` | Virt-Thin | Thin-provisioned image inventory and calculation |

---

## Disk Categorization

Rather than reporting a single usage percentage, the suite identifies where capacity is consumed.

- **Top-level scan:** `du -sh /* | sort -rh | head` with a 12-second timeout provides the primary ranking of top-level directories.
- **Category breakdown:** Each of `/home`, `/var`, `/opt`, `/tmp`, and a sample of `/usr` is scanned independently (`du -sh <path>/* | sort -rh | head`, 6-second timeout per category).
- **File scan:** `find /home /var /tmp -xdev -type f -printf '%s|%p'` is executed via `ionice -c3 nice -n 19` with an 8-second timeout and capped to 500 entries, then sorted and formatted with `numfmt --to=iec`.
- **Package scan:** `dpkg-query -Wf '${Installed-Size}\t${Package}'` is sorted numerically. `snap` and `flatpak` inventories are included with a 5-second timeout.
- **Container scan:** All Docker commands are bounded by `timeout 5` to prevent hangs when the daemon is unavailable.

Tables use fixed-width formatting for alignment and include horizontal rules for readability.

---

## Virtual Images and Overcommit Calculation

This feature is disabled by default and enabled with `--virt` on either `sysh` or `sysht`. The additional scan requires approximately 10 seconds.

**Scope of scan:**

- File patterns: `*.qcow2`, `*.qcow`, `*.vdi`, `*.vmdk`, `*.vhd`, `*.vhdx`, `*.img`
- Search roots: `/home`, `/var/lib/libvirt`, `/var/lib/qemu`, `/var/lib/one`, `/opt`, `/srv`, `/var/tmp` (500-file cap, 10-second timeout)
- Per-file inspection: `qemu-img info --output=json` parsed with `python3` (or `jq`), fallback to `stat` and `du -B1` for raw images
- LVM thin pools: `lvs --units b --reportformat json` with `sudo -n` fallback
- Sparse file sample: `find -printf "%S %p"` filtered for sparseness `< 1.0`

**Output table:**

```
Path                                               Fmt     Virtual     Actual     Remain   Thin%
…/gns3server/disks/empty1T.qcow2                    qcow2      1.0T       208K       1.0T    99%
…/Downloads/Metasploitable.vmdk                     vmdk       8.0G       1.8G       6.2G    77%
```

**Calculation:**

```
host_free          = available bytes on / from df -B1
total_virtual      = sum of virtual sizes
total_actual       = sum of actual disk usage
total_remaining    = total_virtual - total_actual
actual_left        = host_free - total_remaining
```

Interpretation:

- `actual_left > 10% of host_free` — OK (green)
- `0 ≤ actual_left ≤ 10% of host_free` — Low (yellow, warning)
- `actual_left < 0` — Overcommitted (red, critical alert `virt_overcommit` and exit code 2)

The same values are exposed in JSON under `.virt`:

```json
{
  "enabled": true,
  "host_free_bytes": 94004559872,
  "total_virtual_bytes": 7726754073088,
  "total_remaining_bytes": 7722713165824,
  "actual_left_bytes": -7628710289408,
  "actual_left_human": "-7.0T",
  "calc": "actual_left = host_free - total_remaining"
}
```

Limits are intentional: per-file `timeout 5`, overall `find` capped, and LVM/docker sections degrade gracefully when permissions are insufficient.

---

## Installation

```bash
chmod +x sysh sysht
cargo build --manifest-path sysht-tui/Cargo.toml --release
# binary produced at sysht-tui/target/release/sysht-tui
```

No installation beyond the repository is required. The Bash component has no build step.

---

## Usage

### CLI

```bash
./sysh                                   # full report, stdout and log, alerts enabled
./sysh -n 20 -o ~/health.log             # increase rows, custom log path
./sysh --no-color --no-file              # plain output for pipes
./sysh --threshold-disk 80               # custom disk threshold
./sysh --no-alert                        # suppress threshold evaluation

./sysh --json                            # append JSON to report and write .json file
./sysh --json-only | jq .                # JSON only
./sysh --virt -n 20                      # include thin-provisioning analysis
./sysh --virt --json | jq .virt          # virt block only
```

Exit codes: `0` healthy, `1` warning threshold exceeded, `2` critical threshold exceeded.

Common options:

| Option | Description | Default |
|--------|-------------|---------|
| `-n, --top N` | Rows for Top-N tables | `10` |
| `-o, --output FILE` | Log file path | `./sysh-YYYYmmdd_HHMMSS.log` |
| `--no-file` | Disable file output | off |
| `--no-color` | Disable ANSI colors | off when piped |
| `--alert` / `--no-alert` | Enable or disable alerts | enabled |
| `--threshold-disk N` | Disk usage threshold (%) | `90` |
| `--threshold-mem N` | Memory usage threshold (%) | `85` |
| `--threshold-inode N` | Inode usage threshold (%) | `90` |
| `--virt` / `--no-virt` | Thin-provisioned scan | disabled |
| `-j, --json` | Emit JSON alongside report | off |
| `--json-only` | Emit JSON only | off |

### TUI

```bash
./sysht                                  # interactive, top 10
./sysht --virt -n 20 --timeout 90        # virt enabled, extended timeout
./sysht --no-tui -n 5                    # capture raw without rendering
./sysht --build                          # force rebuild of Rust binary
```

The TUI reuses all `sysh` options via the wrapper and forwards `--virt` to the collector.

---

## Keyboard Shortcuts (TUI)

| Key | Action |
|-----|--------|
| `1`–`9`, `0`, `s`, `t`, `v` / `F1`–`F13` | Switch view |
| `Tab` / `Shift+Tab` / `n` / `p` | Cycle views |
| `j` / `k` / `Up` / `Down` | Scroll by one line |
| `PageUp` / `PageDown` | Scroll by 15 lines |
| `Home` / `End` | Jump to top or bottom |
| `r` | Refresh (re-run collector) |
| `a` | Toggle automatic refresh (5-second interval) |
| `Ctrl+v` | Toggle virt scan for the next refresh |
| `?` | Toggle help overlay |
| `q` | Quit |

The footer persistently displays available view keys. The help overlay documents all shortcuts and data provenance (`/tmp/sysht-raw-*.txt`).

---

## Project Structure

```
sysh/
├── sysh                     # Bash CLI, source of truth
│   ├── defaults and thresholds
│   ├── helper functions (have, append, strip_ansi, add_alert, human formatting)
│   ├── render functions for each section (header, cpu, memory, disk, network, processes, services, logs, sensors, virt, alerts)
│   └── option parsing and main dispatch
├── sysht                    # Bash wrapper for the TUI
│   └── argument handling, build logic, and launch of sysht-tui binary
└── sysht-tui/               # Rust TUI application
    ├── Cargo.toml           # ratatui 0.29, crossterm 0.28, clap 4, serde_json
    └── src/
        ├── main.rs          # CLI definition and application entry
        ├── app.rs           # View enumeration, state, input handling, rendering
        ├── health.rs        # Snapshot parsing and sysh invocation
        ├── tui.rs           # Terminal initialization and cleanup
        └── theme.rs         # Color definitions
```

---

## Requirements

**CLI (`sysh`):**

- Bash 5.1+, Linux, `coreutils`, `procps`, `util-linux`, `iproute2`
- `timeout` and `ionice` recommended for bounded execution
- `qemu-img` and `python3` (or `jq`) for `--virt`
- `lvs` for LVM thin pool inspection (permission-gated)
- `docker` for container tables (optional)

**TUI (`sysht`):**

- Rust 1.75+ and Cargo
- Compiled binary at `sysht-tui/target/release/sysht-tui` (approximately 1.7 MB)

No automatic privilege escalation is performed. Sections requiring elevated access report the limitation inline and continue.

---

## Examples

```bash
# Basic report and log inspection
./sysh -n 5
grep "actual_left" ./sysh-*.log
./sysh --virt --json | jq '.virt.actual_left_human'

# Stricter thresholds for capacity planning
./sysh --threshold-disk 80 --threshold-mem 70

# Non-interactive file capture
./sysh --no-color --no-alert -o /tmp/health.log && cat /tmp/health.log

# Interactive monitoring
./sysht --virt -n 20
# Inside TUI: 5 for directories, 6 for files, v for Virt-Thin, r to refresh

# Automation: fail CI on critical alerts
./sysh --virt --json-only | jq -e '.alerts[] | select(.level=="crit")' && echo "critical"
```

---

## Notes

- Collection is streaming. The report is written incrementally, so a timeout on a single `du` or `find` still yields a partial but usable result.
- The overcommit figure represents committed capacity, not a forecast. Removing or compacting images (e.g., `qemu-img compact`, guest `fstrim`) restores `actual_left`.
- All scans are capped and timed. Adjust the overall timeout with `sysht --timeout 90` for hosts with unusually large or deep filesystems.

q