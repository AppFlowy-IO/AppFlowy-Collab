use std::io::Result;
use std::process::Command;
use walkdir::WalkDir;

fn compile_proto_files(proto_files: &[String]) -> Result<()> {
  prost_build::Config::new()
    .protoc_arg("--experimental_allow_proto3_optional")
    .out_dir("src/proto")
    .compile_protos(proto_files, &["proto/"])
}

fn main() -> Result<()> {
  let mut proto_files = Vec::new();
  for e in WalkDir::new("proto").into_iter().filter_map(|e| e.ok()) {
    if e.metadata().unwrap().is_file() {
      proto_files.push(e.path().display().to_string());
    }
  }

  for proto_file in &proto_files {
    println!("cargo:rerun-if-changed={}", proto_file);
  }

  // If the `PROTOC` environment variable is set, don't use vendored `protoc`
  std::env::var("PROTOC").map(|_| ()).unwrap_or_else(|_| {
    let protoc_path = protoc_bin_vendored::protoc_bin_path().expect("protoc bin path");
    let protoc_path_str = protoc_path.to_str().expect("protoc path to str");

    // Set the `PROTOC` environment variable to the path of the `protoc` binary.
    unsafe {
      std::env::set_var("PROTOC", protoc_path_str);
    }
  });

  compile_proto_files(&proto_files).expect("unable to compile proto files");

  let generated_files = std::fs::read_dir("src/proto")?
    .filter_map(Result::ok)
    .filter(|entry| {
      entry
        .path()
        .extension()
        .map(|ext| ext == "rs")
        .unwrap_or(false)
    })
    .map(|entry| entry.path().display().to_string());
  for generated_file in generated_files {
    Command::new("rustfmt").arg(generated_file).status()?;
  }
  Ok(())
}
