## Tram - 📸 A TUI Btrfs snapshot manager with scheduling support

1. [Screenshots](#screenshots)
2. [Features](#features)
3. [Setup](#setup)
4. [Quick Start](#quick-start)
5. [Shell Completion](#shell-completion)
6. [License](#license)

### Screenshots

<p align="center">
  <img src="./assets/screenshot1.png" width="48%">
  <img src="./assets/screenshot2.png" width="48%">
</p>

### Features

- 🖥️ **TUI** — A fast and keyboard-driven terminal interface.
- 📦 **Snapshot Groups** — Manage any combination of Btrfs subvolumes, including multiple Linux installations on the same filesystem.
- ⏰ **Scheduled Snapshots** — Automatically create snapshots at scheduled intervals.

### Setup

<details>
   <summary><b>Installation</b></summary>

- _Arch Linux_

    ```bash
    yay -S tram_btrfs
    ```

- _Cargo_

    Installing via `cargo` _**doesn't support shell completion and boot service**_.
    See the section below to manually [generate shell completion](#shell-completion) and [install boot service](#manually-boot-service)

    ```bash
    cargo install tram_btrfs
    ```

</details>

<a id="manually-boot-service"></a>
<details>
    <summary><b>Boot Service</b></summary>

- If you want the program to check snapshot schedule every time your system boots, enable the following **_Systemd_** service:

    ```bash
    systemctl enable tram_btrfs.service
    ```

- This service simply execute `tram_btrfs --boot` during system startup. So if you're _**NOT using Systemd**_, configure your init system to run this command at boot instead.

- The program also checks the schedule whenever it starts. However, boot snapshots are only created when running with `--boot` flag.

- If you installed via `cargo` or directly download the GitHub release, you may need to install boot service manually:
  If you're using Systemd, execute this(if not, see above):
    ```bash
    sudo curl -Lo '/usr/lib/systemd/system/tram_btrfs.service' 'https://raw.githubusercontent.com/Xiaomony/tram_btrfs/main/packaging/systemd/tram_btrfs.service'
    ```

</details>

### Quick Start

1. Snapshots are managed in groups. A snapshot group is a collection of subvolumes that are snapshotted together. You can create, rename, delete groups in "Groups" section. And if your Btrfs system contains "@" and "@home" subvolumes, it will automatically create a "default" group the first time you launch it.

2. The program can automatically create scheduled snapshots(daily, weekly, monthly and boot snapshots). In "Settings" section, you can set the maximum count of each type of scheduled snapshots. To enable the boot snapshots, see the [Setup](#setup) section above.

3. The keybindings are **Vim-like** and there's prompts below the menu as well.

4. The program stores its configuration in `~/.config/tram_btrfs/tram.toml`. Since it requires root privileges to perform Btrfs operations, the configuration file will be stored in `/root/.config/tram_btrfs/` when the program is run with `sudo`.

5. By default, the program detects the Btrfs device containing the currently running Linux system and mounts it at `/run/tram_btrfs/`. But you can specify any Btrfs devices using `--device` flag.(`tram_btrfs --device '/dev/nvme0n1p8'` for example)

### Shell Completion

If you installed via `cargo` or directly download the GitHub release, you may need to generate the shell completion manually:

- _**Install system-wide**_
    1. Bash
        ```bash
        tram_btrfs completion bash | sudo tee /usr/share/bash-completion/completions/tram_btrfs >/dev/null
        ```
    2. Zsh
        ```bash
        tram_btrfs completion zsh | sudo tee /usr/share/zsh/site-functions/_tram_btrfs >/dev/null
        ```
    3. Fish
        ```bash
        tram_btrfs completion fish | sudo tee /usr/share/fish/vendor_completions.d/tram_btrfs.fish >/dev/null
        ```
- _**Install for current user**_
    1. Bash
        ```bash
        mkdir -p ~/.local/share/bash-completion/completions
        tram_btrfs completion bash >~/.local/share/bash-completion/completions/tram_btrfs
        ```
    2. Zsh
        ```bash
        mkdir -p ~/.zsh/completions
        tram_btrfs completion zsh >~/.zsh/completions/_tram_btrfs
        ```
        Then add the following to your `.zshrc` if needed:
        ```bash
        fpath=(~/.zsh/completions $fpath)
        autoload -Uz compinit
        compinit
        ```
    3. Fish
        ```bash
        mkdir -p ~/.config/fish/completions
        tram_btrfs completion fish >~/.config/fish/completions/tram_btrfs.fish
        ```

### License

Tram is licensed under the GNU General Public License v3.0 or later (GPL-3.0-or-later).

See the [LICENSE](./LICENSE) file for the full license text.
