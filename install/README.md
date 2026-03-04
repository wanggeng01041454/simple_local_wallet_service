# local-wallet Installation Guide

## Prerequisites

Build the release binary first:

```bash
cargo build --release
# Binary: target/release/local-wallet
```

For Windows cross-compilation:
```bash
cargo build --release --target x86_64-pc-windows-msvc
```

For Android Termux cross-compilation:
```bash
cargo build --release --target aarch64-linux-android
```

---

## Linux (systemd)

```bash
cp target/release/local-wallet .
chmod +x install/install-linux.sh
./install/install-linux.sh
```

- Installs binary to `/usr/local/bin/local-wallet`
- Creates and enables `/etc/systemd/system/local-wallet.service`
- Starts the service immediately

**Manage the service:**
```bash
sudo systemctl status local-wallet
sudo systemctl restart local-wallet
sudo systemctl stop local-wallet
```

---

## macOS (launchd)

```bash
cp target/release/local-wallet .
chmod +x install/install-macos.sh
./install/install-macos.sh
```

- Installs binary to `/usr/local/bin/local-wallet`
- Creates `~/Library/LaunchAgents/com.local-wallet.plist`
- Starts automatically at login

**Manage the service:**
```bash
launchctl stop com.local-wallet
launchctl start com.local-wallet
launchctl unload ~/Library/LaunchAgents/com.local-wallet.plist  # disable
```

---

## Windows (Task Scheduler)

```powershell
Copy-Item target\release\local-wallet.exe .
.\install\install-windows.ps1
```

- Installs binary to `%LOCALAPPDATA%\local-wallet\`
- Registers a Task Scheduler task that runs at logon
- No administrator privileges required

**Manage the task:**
```powershell
Start-ScheduledTask -TaskName LocalWalletService
Stop-ScheduledTask -TaskName LocalWalletService
Unregister-ScheduledTask -TaskName LocalWalletService -Confirm:$false  # uninstall
```

---

## Android Termux

```bash
cp local-wallet .   # copy the aarch64 binary
chmod +x install/install-termux.sh
./install/install-termux.sh
```

- Installs binary to `$PREFIX/bin/local-wallet`
- Adds `wallet-start` / `wallet-stop` aliases to `~/.bashrc`

**Note:** Termux has no persistent service manager. The wallet runs in the foreground or inside a `tmux` session. It will stop when the Termux session ends.

**Start with tmux (recommended for persistence):**
```bash
source ~/.bashrc
wallet-start   # starts in background tmux session named 'wallet'
wallet-stop    # stops it
```

---

## Accessing the Service

After installation on any platform:

| Interface | URL |
|-----------|-----|
| Admin UI | http://localhost:9292 |
| REST API | http://localhost:9293 (localhost only) |

**First run:** The Admin UI will guide you through setting a password and creating or importing wallets.

**After each restart:** Visit http://localhost:9292 and enter your password to unlock the wallets.
