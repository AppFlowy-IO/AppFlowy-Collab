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

  if std::panic::catch_unwind(|| compile_proto_files(&proto_files)).is_err() {
    std::env::set_var(
      "PROTOC",
      protoc_bin_vendored::protoc_bin_path()
        .expect("vendored protoc binary not found")
        .to_str()
        .expect("vendored protoc binary path is not valid string"),
    );
    compile_proto_files(&proto_files)?
  }

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
