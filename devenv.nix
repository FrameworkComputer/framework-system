{ pkgs, lib, config, inputs, ... }:

rec {
  packages = with pkgs; [
    systemd # libudev
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

  languages.rust.enable = true;
  # https://devenv.sh/reference/options/#languagesrustchannel
  languages.rust.channel = "stable";
}
