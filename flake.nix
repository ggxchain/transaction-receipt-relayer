{
  description = "Build transaction-receipt-relayer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.05";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # rust version
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        inherit (pkgs) lib;

        rust-toolchain =
          pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rust-toolchain;
        sqlFilter = path: _type: builtins.match ".*\.sql$" path != null;
        testJsonFilter = path: _type: builtins.match ".*\/tests\/.*\.json$" path != null;
        scaleFilter = path: _type: builtins.match ".*\.scale$" path != null;
        src = lib.cleanSourceWith {
          src = (craneLib.path ./.);
          filter = path: type:
            builtins.any (filter: filter path type) [
              sqlFilter
              testJsonFilter
              scaleFilter
              craneLib.filterCargoSources
            ];
        };

        # but many build.rs do - so we add little bit slowness for simplificaiton and reproduceability
        rustNativeBuildInputs = with pkgs; [ clang pkg-config gnumake ];

        # reusable env for shell and builds
        rustEnv = with pkgs; {
          LD_LIBRARY_PATH = pkgs.lib.strings.makeLibraryPath [
            pkgs.stdenv.cc.cc.lib
            pkgs.llvmPackages.libclang.lib
          ];
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          RUSTUP_TOOLCHAIN = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain.channel; # for dylint
        };

        # Common arguments can be set here to avoid repeating them later
        commonArgs = rustEnv // {
          inherit src;
          pname = "workspace";

          nativeBuildInputs = with pkgs; rustNativeBuildInputs ++ [ openssl ];
          buildInputs = with pkgs; [
            openssl
            perl
            sqlite
            zstd
            # Add additional build inputs here
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];

          # Additional environment variables can be set directly
          # MY_CUSTOM_VAR = "some value";
          doCheck = false;
          cargoCheckCommand = "true";
        };

        # Check that the pallets built in WASM modes
        commonWasmArgs = commonArgs // {
          pname = "workspace-wasm";
          cargoExtraArgs =
                "-p pallet-receipt-registry -p pallet-chain-extension-receipt-registry -p pallet-evm-eth-receipt-provider --target wasm32-unknown-unknown --no-default-features";
        };

        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        wasmCargoArtifacts = craneLib.buildDepsOnly commonWasmArgs;


        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        workspace = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
        wasmWorkspace = craneLib.buildPackage (commonWasmArgs // {
          cargoArtifacts = wasmCargoArtifacts;
        });
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit workspace wasmWorkspace;

          # Run clippy (and deny all warnings) on the crate source,
          # again, resuing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          # Check formatting
          fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Run tests with cargo-nextest
          # Consider setting `doCheck = false` on `transaction-receipt-relayer` if you do not want
          # the tests to run twice
          nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });
        };

        packages = {
          default = workspace;
          workspace-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        apps.default = flake-utils.lib.mkApp {
          drv = workspace;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

          # Extra inputs can be added here
          nativeBuildInputs = with pkgs; [
            sqlite
            lldb
            # Mold Linker for faster builds (only on Linux)
            (lib.optionals pkgs.stdenv.isLinux pkgs.mold)
            (lib.optionals pkgs.stdenv.isDarwin pkgs.darwin.apple_sdk.frameworks.Security)
            (lib.optionals pkgs.stdenv.isDarwin pkgs.darwin.apple_sdk.frameworks.SystemConfiguration)
          ];

          buildInputs = [
            # We want the unwrapped version, wrapped comes with nixpkgs' toolchain
            pkgs.rust-analyzer-unwrapped
            # Finally the toolchain
            rust-toolchain
          ];
        };
      });
}
