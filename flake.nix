{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    android.url = "github:tadfisher/android-nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      android,
      ...
    }:
    let
      inherit (nixpkgs.lib) attrValues genAttrs;
      supportedSystems = [ "x86_64-linux" ];
      forAllSystems = f: genAttrs supportedSystems (system: f system);
    in rec {
      nixpkgsFor = forAllSystems (system: import nixpkgs {
        inherit system;
        overlays = attrValues self.overlays ++ [(import rust-overlay)];
        config.allowUnfree = true;
      });

      overlays.default = final: prev: {
        android-tablet-bridge = final.callPackage ./default.nix {};
      };

      packages = forAllSystems (system: rec {
        inherit (nixpkgsFor.${system}) android-tablet-bridge;
        default = android-tablet-bridge;
      });

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgsFor.${system};
          android-studio = pkgs.androidStudioPackages.stable;
          android-sdk = android.sdk.${system} (sdkPkgs: with sdkPkgs; [
            # Useful packages for building and testing.
            build-tools-34-0-0
            cmdline-tools-latest
            emulator
            platform-tools
            platforms-android-34

            # Other useful packages for a development environment.
            # ndk-26-1-10909125
            # skiaparser-3
            # sources-android-34
          ]);
        in {
          default = pkgs.mkShell {
            ANDROID_HOME = "${android-sdk}/share/android-sdk";
            ANDROID_SDK_ROOT = "${android-sdk}/share/android-sdk";
            JAVA_HOME = pkgs.jdk.home;
            buildInputs = with pkgs; [
              android-sdk
              android-studio
              gradle
              jdk
              (rust-bin.selectLatestNightlyWith (
                toolchain:
                toolchain.default.override {
                  targets = [ "x86_64-unknown-linux-gnu" ];
                  extensions = [
                    "rust-analyzer"
                    "rust-src"
                  ];
                }
              ))
            ];
          };
        });
    };
}
