{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = [ pkgs.rustup pkgs.cargo pkgs.openssl pkgs.pkg-config ];
}
