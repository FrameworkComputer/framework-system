{ pkgs, lib, config, inputs, ... }:

{
  packages = with pkgs; [
    systemd # libudev
  ];

  languages.rust.enable = true;
  # https://devenv.sh/reference/options/#languagesrustchannel
  languages.rust.channel = "stable";
}
