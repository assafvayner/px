fn main() {
    println!("build script");
    println!("build script");
    prost_build::compile_protos(&["src/protos/lmao.proto"], &["src/"]).unwrap();
}
