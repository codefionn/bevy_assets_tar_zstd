#!/usr/bin/env nix-shell
{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell rec {
    name = "rustc-build-env";
    nativeBuildInputs = with pkgs; [
      pkg-config
      llvmPackages.bintools
    ];
    buildInputs = with pkgs; [
      rustup cargo # Required for using rust
      # Required for cargo packages
      xlibsWrapper xorg.libXcursor xorg.libXrandr xorg.libXi alsaLib
      vulkan-loader amdvlk
      libxkbcommon alsa-lib udev wayland
      curl python
    ];
    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

    # Install the required toolchain
    shellHook = ''
      rustup toolchain install stable
    '';
  }
