{ pkgs, lib, config, inputs, ... }:

{
  packages = with pkgs; [
    systemd # libudev
    # For UEFI building and testing
    parted
    gnumake
    qemu
  ];

  languages.rust = {
    enable = true;
    targets = [ "x86_64-unknown-uefi" ];
    # https://devenv.sh/reference/options/#languagesrustchannel
    channel = "stable";
  };
}
