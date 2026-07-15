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

  scripts.web-create-operator = {
    description = "Create the local web encryption key and an operator account";
    exec = ''
      if [ "$#" -ne 1 ]; then
        echo "Usage: web-create-operator EMAIL" >&2
        exit 2
      fi

      email="$1"
      state_dir="$PWD/.devenv/state"
      key_file="$state_dir/web-intake.key"
      export EBIRFORMS_WEB_DB="''${EBIRFORMS_WEB_DB:-$state_dir/web-intake.sqlite3}"

      mkdir -p "$state_dir"
      if [ ! -f "$key_file" ]; then
        umask 077
        openssl rand -base64 32 > "$key_file"
      fi
      chmod 600 "$key_file"
      export EBIRFORMS_WEB_ENCRYPTION_KEY="$(cat "$key_file")"

      if [ -z "''${EBIRFORMS_NEW_USER_PASSWORD:-}" ]; then
        printf "Operator password (at least 12 characters): "
        IFS= read -r -s EBIRFORMS_NEW_USER_PASSWORD
        printf "\nConfirm password: "
        IFS= read -r -s password_confirmation
        printf "\n"
        if [ "$EBIRFORMS_NEW_USER_PASSWORD" != "$password_confirmation" ]; then
          echo "Passwords do not match." >&2
          exit 2
        fi
        export EBIRFORMS_NEW_USER_PASSWORD
      fi

      cargo run -p ebirforms-web -- create-user "$email" operator
    '';
  };

  scripts.web-dev = {
    description = "Run the hosted intake API and Leptos frontend with hot reload";
    exec = ''
      export EBIRFORMS_WEB_INSECURE_COOKIE=1
      export EBIRFORMS_WEB_DB="''${EBIRFORMS_WEB_DB:-$PWD/.devenv/state/web-intake.sqlite3}"
      export EBIRFORMS_WEB_FRONTEND_PORT="''${EBIRFORMS_WEB_FRONTEND_PORT:-1421}"
      export EBIRFORMS_WEB_API_PORT="''${EBIRFORMS_WEB_API_PORT:-3001}"
      export EBIRFORMS_WEB_BIND="127.0.0.1:$EBIRFORMS_WEB_API_PORT"
      key_file="$PWD/.devenv/state/web-intake.key"
      if [ ! -f "$key_file" ]; then
        umask 077
        mkdir -p "$(dirname "$key_file")"
        openssl rand -base64 32 > "$key_file"
      fi
      export EBIRFORMS_WEB_ENCRYPTION_KEY="$(cat "$key_file")"
      cargo run -p ebirforms-web &
      api_pid=$!
      trap 'kill "$api_pid" 2>/dev/null || true' EXIT INT TERM
      cd apps/web/frontend
      NO_COLOR=false trunk serve --address 127.0.0.1 --port "$EBIRFORMS_WEB_FRONTEND_PORT" --proxy-backend="http://127.0.0.1:$EBIRFORMS_WEB_API_PORT/api/"
    '';
  };
}
