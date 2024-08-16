My first Rust application, `reboot-to` is a convenience wrapper around `efibootmgr` and `shutdown` that helps you quickly reboot to another OS on your machine. No more spamming F8 on the BIOS screen, just select and forget!

## How it works
`reboot-to` uses the `efibootmgr` executable on your system to enumerate all possible UEFI boot entries. These include other operating systems on your computer, for example when you are dual-booting Windows. It then shows these in a menu, and lets you pick one to reboot to. Behind the scenes, `reboot-to` then uses `efibootmgr` again to set the selected boot entry as a one-time boot target. Finally, the `shutdown` executable is called in order to trigger a reboot.

`reboot-to` comes with command-line switches to skip the TUI part completely, and directly reboot to another UEFI boot entry based on ID or name.

## Requirements

- **UEFI**: Since this uses `efibootmgr` in the background;
- **A system with** `efibootmgr` **and** `shutdown` **available**: `reboot-to` uses these two commands in the background, so they have to be in path. These are available on most modern linux distros;
- **Permissions**: On most systems, using `shutdown` to reboot and `efibootmgr` to set a one-time boot target requires root access. `reboot-to` will tell you if it lacks permissions.

## Acknowledgements

- Rust for being a fun brain-teaser to learn, and a fresh breath from C/C++;
- clap for parsing command line args;
- ratatui for making TUI's easy.

## License
`reboot-to` is licensed under the MIT license, for more information see `LICENSE` in the project root.
