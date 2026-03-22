complete -c framework_tool -l flash-gpu-descriptor -r
complete -c framework_tool -l device -r -f -a "bios\t''
ec\t''
pd0\t''
pd1\t''
rtm01\t''
rtm23\t''
ac-left\t''
ac-right\t''"
complete -c framework_tool -l compare-version -r
complete -c framework_tool -l fansetduty -d 'Set fan duty cycle (0-100%)' -r
complete -c framework_tool -l fansetrpm -d 'Set fan RPM (limited by EC fan table max RPM)' -r
complete -c framework_tool -l autofanctrl -d 'Turn on automatic fan speed control' -r
complete -c framework_tool -l meinfo -d 'Show Intel ME information (from SMBIOS type 0xDB). Optionally provide a dmidecode binary dump file path' -r
complete -c framework_tool -l pd-reset -d 'Reset a specific PD controller (for debugging only)' -r
complete -c framework_tool -l pd-disable -d 'Disable all ports on a specific PD controller (for debugging only)' -r
complete -c framework_tool -l pd-enable -d 'Enable all ports on a specific PD controller (for debugging only)' -r
complete -c framework_tool -l dp-hdmi-update -d 'Update the DisplayPort or HDMI Expansion Card' -r -F
complete -c framework_tool -l pd-bin -d 'Parse versions from PD firmware binary file' -r -F
complete -c framework_tool -l ec-bin -d 'Parse versions from EC firmware binary file' -r -F
complete -c framework_tool -l capsule -d 'Parse UEFI Capsule information from binary file' -r -F
complete -c framework_tool -l dump -d 'Dump extracted UX capsule bitmap image to a file' -r -F
complete -c framework_tool -l h2o-capsule -d 'Parse UEFI Capsule information from binary file' -r -F
complete -c framework_tool -l dump-ec-flash -d 'Dump EC flash contents' -r -F
complete -c framework_tool -l flash-ec -d 'Flash EC (RO+RW) with new firmware from file - may render your hardware unbootable!' -r -F
complete -c framework_tool -l flash-ro-ec -d 'Flash EC with new RO firmware from file - may render your hardware unbootable!' -r -F
complete -c framework_tool -l flash-rw-ec -d 'Flash EC with new RW firmware from file' -r -F
complete -c framework_tool -l inputdeck-mode -d 'Set input deck power mode [possible values: auto, off, on] (Framework 16 only)' -r -f -a "auto\t''
off\t''
on\t''"
complete -c framework_tool -l charge-limit -d 'Get or set max charge limit' -r
complete -c framework_tool -l charge-current-limit -d 'Set max charge current limit' -r
complete -c framework_tool -l charge-rate-limit -d 'Set max charge current limit' -r
complete -c framework_tool -l get-gpio -d 'Get GPIO value by name or all, if no name provided' -r
complete -c framework_tool -l fp-led-level -d 'Get or set fingerprint LED brightness level' -r -f -a "high\t''
medium\t''
low\t''
ultra-low\t''
auto\t''"
complete -c framework_tool -l fp-brightness -d 'Get or set fingerprint LED brightness percentage' -r
complete -c framework_tool -l kblight -d 'Set keyboard backlight percentage or get, if no value provided' -r
complete -c framework_tool -l remap-key -d 'Remap a key by changing the scancode' -r
complete -c framework_tool -l rgbkbd -d 'Set the color of <key> to <RGB>. Multiple colors for adjacent keys can be set at once. <key> <RGB> [<RGB> ...] Example: 0 0xFF000 0x00FF00 0x0000FF' -r
complete -c framework_tool -l ps2-enable -d 'Control PS2 touchpad emulation (DEBUG COMMAND, if touchpad not working, reboot system)' -r -f -a "true\t''
false\t''"
complete -c framework_tool -l tablet-mode -d 'Set tablet mode override' -r -f -a "auto\t''
tablet\t''
laptop\t''"
complete -c framework_tool -l touchscreen-enable -d 'Enable/disable touchscreen' -r -f -a "true\t''
false\t''"
complete -c framework_tool -l console -d 'Get EC console, choose whether recent or to follow the output' -r -f -a "recent\t''
follow\t''"
complete -c framework_tool -l reboot-ec -d 'Control EC RO/RW jump' -r -f -a "reboot\t''
jump-ro\t''
jump-rw\t''
cancel-jump\t''
disable-jump\t''"
complete -c framework_tool -l ec-hib-delay -d 'Get or set EC hibernate delay (S5 to G3)' -r
complete -c framework_tool -l hash -d 'Hash a file of arbitrary data' -r -F
complete -c framework_tool -l driver -d 'Select which driver is used. By default portio is used' -r -f -a "portio\t''
cros-ec\t''
windows\t''"
complete -c framework_tool -l pd-addrs -d 'Specify I2C addresses of the PD chips (Advanced)' -r
complete -c framework_tool -l pd-ports -d 'Specify I2C ports of the PD chips (Advanced)' -r
complete -c framework_tool -l flash-gpu-descriptor-file -d 'File to write to the gpu EEPROM' -r -F
complete -c framework_tool -l dump-gpu-descriptor-file -d 'File to dump the gpu EEPROM to' -r -F
complete -c framework_tool -l host-command -d 'Send an EC host command. Args: <CMD_ID> <VERSION> [DATA...]' -r
complete -c framework_tool -l generate-completions -d 'Generate shell completions and print to stdout' -r -f -a "bash\t''
elvish\t''
fish\t''
powershell\t''
zsh\t''"
complete -c framework_tool -s v -l verbose -d 'Increase logging verbosity'
complete -c framework_tool -s q -l quiet -d 'Decrease logging verbosity'
complete -c framework_tool -l versions -d 'List current firmware versions'
complete -c framework_tool -l version -d 'Show tool version information (Add -vv for more details)'
complete -c framework_tool -l features -d 'Show features support by the firmware'
complete -c framework_tool -l esrt -d 'Display the UEFI ESRT table'
complete -c framework_tool -l power -d 'Show current power status of battery and AC (Add -vv for more details)'
complete -c framework_tool -l thermal -d 'Print thermal information (Temperatures and Fan speed)'
complete -c framework_tool -l sensors -d 'Print sensor information (ALS, G-Sensor)'
complete -c framework_tool -l pdports -d 'Show information about USB-C PD ports'
complete -c framework_tool -l info -d 'Show info from SMBIOS (Only on UEFI)'
complete -c framework_tool -l pd-info -d 'Show details about the PD controllers'
complete -c framework_tool -l dp-hdmi-info -d 'Show details about connected DP or HDMI Expansion Cards'
complete -c framework_tool -l audio-card-info -d 'Show details about connected Audio Expansion Cards (Needs root privileges)'
complete -c framework_tool -l privacy -d 'Show privacy switch statuses (camera and microphone)'
complete -c framework_tool -l intrusion -d 'Show status of intrusion switch'
complete -c framework_tool -l inputdeck -d 'Show status of the input modules (Framework 16 only)'
complete -c framework_tool -l expansion-bay -d 'Show status of the expansion bay (Framework 16 only)'
complete -c framework_tool -l stylus-battery -d 'Check stylus battery level (USI 2.0 stylus only)'
complete -c framework_tool -l uptimeinfo
complete -c framework_tool -l s0ix-counter
complete -c framework_tool -s t -l test -d 'Run self-test to check if interaction with EC is possible'
complete -c framework_tool -l test-retimer -d 'Run self-test to check if interaction with retimers is possible'
complete -c framework_tool -l boardid -d 'Print all board IDs'
complete -c framework_tool -s f -l force -d 'Force execution of an unsafe command - may render your hardware unbootable!'
complete -c framework_tool -l dry-run -d 'Simulate execution of a command (e.g. --flash-ec)'
complete -c framework_tool -l nvidia -d 'Show NVIDIA GPU information (Framework 16 only)'
complete -c framework_tool -s h -l help -d 'Print help'
