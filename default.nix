let
  niv-sources = import ./nix/sources.nix;
  mozilla-overlay = import niv-sources.nixpkgs-mozilla;
  pkgs = import niv-sources.nixpkgs { overlays = [ mozilla-overlay ]; };
  src = pkgs.nix-gitignore.gitignoreSource [ ] ./.;
  cargo2nix = pkgs.callPackage niv-sources.cargo2nix {
    lockfile = ./Cargo.lock;
  };
in pkgs.stdenv.mkDerivation {
  inherit src;
  name = "track-cli";
  buildInputs = [ pkgs.latest.rustChannels.nightly.rust ];
  phases = [ "unpackPhase" "buildPhase" ];
  buildPhase = ''
    # Setup dependencies path to satisfy Cargo
    mkdir .cargo/
    ln -s ${cargo2nix.env.cargo-config} .cargo/config
    ln -s ${cargo2nix.env.vendor} vendor

    # Run the tests
    cargo test
    touch $out
  '';
}
