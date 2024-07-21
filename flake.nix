{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          rustToolchain = (pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
            targets = [ "wasm32-unknown-unknown" ];
          };
          nativeBuildInputs = with pkgs; [ rustToolchain pkg-config ];
          buildInputs = with pkgs; [ 
	    openssl 
      xorg.libX11 xorg.libXcursor xorg.libXi xorg.libXrandr
	  ];
        in
        with pkgs;
        {
	  packages = {
            default = rustPlatform.buildRustPackage {
              inherit buildInputs nativeBuildInputs;
              pname = "hectic-key-capture";
              version = "0.1.0";
              src = ./.;
              cargoSha256 = "sha256-iMcVfwmUGSNJfQogAZZMI/7pRvgqLhVx7gvK7bAB5gY=";
            };
          };

          apps = {
            default = {
              type = "app";
              program = "${self.packages.${system}.default}/bin/hectic-key-capture";
            };
          };
	  
          devShells.default = mkShell {
            inherit buildInputs nativeBuildInputs;
            shellHook = ''
              export PATH="''${PATH}:''${HOME}/.cargo/bin"
            '';
          };
        }
      );

}
