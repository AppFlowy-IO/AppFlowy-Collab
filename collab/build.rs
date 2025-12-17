use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // If the `PROTOC` environment variable is set, don't use vendored `protoc`
  std::env::var("PROTOC").map(|_| ()).unwrap_or_else(|_| {
    let protoc_path = protoc_bin_vendored::protoc_bin_path().expect("protoc bin path");
    let protoc_path_str = protoc_path.to_str().expect("protoc path to str");

    // Set the `PROTOC` environment variable to the path of the `protoc` binary.
    unsafe {
      std::env::set_var("PROTOC", protoc_path_str);
    }
  });

  let proto_files = vec![
    "proto/entity/common.proto",
    "proto/entity/encoding.proto",
    "proto/entity/params.proto",
  ];
  for proto_file in &proto_files {
    println!("cargo:rerun-if-changed={}", proto_file);
  }

  let out_dir = std::env::var("OUT_DIR")?;
  prost_build::Config::new()
    .out_dir(out_dir)
    .compile_protos(&proto_files, &["proto/entity/"])?;

  // Optional: keep generated sources formatted when building locally.
  // Ignore errors to avoid failing builds when `rustfmt` isn't available.
  let _ = Command::new("rustfmt")
    .arg(format!("{}/collab.rs", std::env::var("OUT_DIR")?))
    .status();
  Ok(())
}
