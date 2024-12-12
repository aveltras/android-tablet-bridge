{ lib, rustPlatform }:

rustPlatform.buildRustPackage rec {
  pname = "android-tablet-bridge";
  version = "0.1.0";
  src = ./.;
  cargoHash = "sha256-zFWOSn9MN3BNqCWdMYB5hkXWA3n4PeIE2Rv8oFgBcY0=";
}
