{ pkgs ? import <nixpkgs> {}}:

let
  # Fetch and build the llvm-mingw toolchain environment
  llvm-mingw-toolchain = pkgs.stdenv.mkDerivation rec {
    pname = "llvm-mingw-ucrt";
    version = "20250812";

    # 1. Use fetchurl to download the compressed tarball.
    #    stdenv.mkDerivation will automatically unpack it.
    src = pkgs.fetchurl {
      url = "https://github.com/mstorsjo/llvm-mingw/releases/download/${version}/llvm-mingw-${version}-ucrt-ubuntu-22.04-x86_64.tar.xz";
      # I have pre-calculated the hash for you.
      sha256 = "0j8i7ch0sv0z6rac9irpgqr9vjn9w053dk0myw7hflmdxvykwnsn";
    };

    # 2. We need to patch the binaries to find their libraries in the Nix store.
    nativeBuildInputs = [ pkgs.autoPatchelfHook ];

    # 3. The toolchain requires the standard C library to run.
    buildInputs = [ pkgs.stdenv.cc.libc pkgs.libcxx pkgs.zstd pkgs.xz pkgs.libxml2 pkgs.ncurses pkgs.libz  ];

    # 4. The installation phase copies the toolchain into the Nix store path.
    #    The default behavior is to cd into the extracted directory,
    #    so this copies its contents.
    installPhase = ''
      runHook preInstall
      mkdir -p $out
      cp -r ./* $out/
      runHook postInstall
    '';

    # 5. This flag is necessary to activate the autoPatchelfHook.
    autoPatchelf = true;
  };

in
# 6. Create the final shell environment
pkgs.mkShell {
  # Add the new toolchain to the environment's PATH.
  buildInputs = [
    llvm-mingw-toolchain
    # You can add other tools you need here.
    pkgs.bashInteractive
  ];

  # This message will be displayed when the shell loads.
  shellHook = ''
    echo "✅ llvm-mingw toolchain is now in your PATH"
    echo "   You can now use commands like: x86_64-w64-mingw32-gcc"
    export CXXFLAGS="-I${llvm-mingw-toolchain}/x86_64-w64-mingw32/include/c++/v1"
    export CC=x86_64-w64-mingw32-clang
    export CXX=x86_64-w64-mingw32-clang++
  '';
}
