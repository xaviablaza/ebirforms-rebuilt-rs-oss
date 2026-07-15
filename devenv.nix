{ pkgs, lib, ... }:

let
  linuxDesktopPackages = lib.optionals pkgs.stdenv.isLinux [
    pkgs.glib
    pkgs.gtk3
    pkgs.libayatana-appindicator
    pkgs.librsvg
    pkgs.libsoup_3
    pkgs.webkitgtk_4_1
    pkgs.xdo
  ];
in
{
  languages.rust = {
    enable = true;
    channel = "stable";
    version = "1.88.0";
    targets = [ "wasm32-unknown-unknown" ];
  };

  languages.javascript = {
    enable = true;
    package = pkgs.nodejs_22;
    npm.enable = true;
  };

  packages = [
    pkgs.curl
    pkgs.file
    pkgs.libssh2
    pkgs.openssh
    pkgs.openssl
    pkgs.pkg-config
    pkgs.sshpass
    pkgs.trunk
    pkgs.wget
    pkgs.zlib
  ] ++ linuxDesktopPackages;

  env = lib.optionalAttrs pkgs.stdenv.isLinux {
    LD_LIBRARY_PATH = lib.makeLibraryPath linuxDesktopPackages;
  };

  scripts.desktop-dev = {
    description = "Run the Tauri + Leptos desktop app with Nix-provided tools";
    exec = ''
      npm --prefix apps/desktop install
      npm --prefix apps/desktop/frontend install
      cd apps/desktop
      exec npx tauri dev --config '{"build":{"beforeDevCommand":"cd frontend && NO_COLOR=false trunk serve --address 127.0.0.1 --port 1420"},"bundle":{"active":false,"resources":[],"externalBin":[]}}'
    '';
  };
}
