{ pkgs, lib, config, inputs, ... }:

rec {
  packages = with pkgs; [
    systemd # libudev
    # For UEFI building and testing
    parted
    gnumake
    qemu
    # For framework_gui (iced)
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    xorg.libxcb
    libxkbcommon
    vulkan-loader
    wayland
  ];

  enterShell = ''
    export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${builtins.toString (pkgs.lib.makeLibraryPath packages)}";
  '';

  languages.rust = {
    enable = true;
    targets = [ "x86_64-unknown-uefi" ];
    # https://devenv.sh/reference/options/#languagesrustchannel
    channel = "stable";
  };
}
