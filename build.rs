fn main() {
    csbindgen::Builder::default()
        .input_extern_file("src/compute_shader_interop.rs")
        .csharp_dll_name("version")
        .csharp_class_name("TrueTraceNative")
        .generate_csharp_file("./dotnet/NativeMethods.g.cs")
        .unwrap();
}