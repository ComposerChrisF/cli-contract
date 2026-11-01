use cli_contract::{check_output_file, prepare_output_file, Clobber, CreateMode};
use std::path::Path;
fn main() {
    std::env::set_current_dir(std::env::args().nth(1).unwrap()).unwrap();
    let p = Path::new("a/../a");
    println!("check:   {:?}", check_output_file(p, CreateMode::All, Clobber::Allow));
    println!("prepare: {:?}", prepare_output_file(p, CreateMode::All, Clobber::Allow));
    println!("a is a dir now: {}", Path::new("a").is_dir());
    println!("write: {:?}", std::fs::write(p, b"x").map_err(|e| e.kind()));
    println!("again: {:?}", prepare_output_file(p, CreateMode::All, Clobber::Allow));
}
