let 
	pkgs = import <nixpkgs> {crossSystem = {
		config = "x86_64-w64-mingw32";
		libc = "ucrt";
		useLLVM = true;
		rust.rustcTarget = "x86_64-pc-windows-gnullvm";
};};
in
	pkgs.mkShell.override {stdenv = pkgs.llvmPackages.stdenv;} {
		name = "truetrace-build-shell";
#		packages = [
#			pkgs.windows.mcfgthreads
#			pkgs.libgcc
#		];
		buildInputs = [
			pkgs.windows.mingw_w64
			pkgs.llvmPackages.libunwind
		#	pkgs.buildPackages.windows.mingw_w64_headers 
    			(pkgs.windows.mcfgthreads.overrideAttrs {
				dontDisableStatic = true;
})
		];
		shellHook = ''
#			export CXXFLAGS="-I ${pkgs.windows.mcfgthreads.dev}/include"
#			
#			export RUSTFLAGS="-L ${pkgs.buildPackages.libgcc}/lib -C linker=$CC -C link-args=-L${pkgs.buildPackages.libgcc}/lib"
			
#			export CXX=clang++
#			export CC=clang
#		'';
	}

