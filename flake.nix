{
  description = "vk";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;

          OPENSSL_NO_VENDOR = "1";

          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";

          nativeBuildInputs = [
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.openssl
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        vk = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
      {
        checks = {
          inherit vk;

          vk-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          vk-doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          vk-fmt = craneLib.cargoFmt {
            inherit src;
          };
        };

        packages = {
          default = vk;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = vk;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = [
          ];
        };
      }
    )
    // {
      # System-agnostic, so it lives outside eachDefaultSystem — moira reads it
      # as `.#moiraFlake`.
      #
      # `checks.*` is the point here: vk-fmt, vk-clippy, vk-doc and vk are
      # already crane derivations covering exactly what `.moira/test.yaml` runs
      # by hand, and until now nothing built them. As jobs they are hermetic,
      # pushed to the org cache, and skipped entirely on a commit that did not
      # change their inputs.
      moiraFlake = {
        include = [
          "packages.*"
          "checks.*"
        ];
      };
    };
}
