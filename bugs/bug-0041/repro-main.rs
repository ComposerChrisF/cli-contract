// Check-only: nothing here creates anything.  Argument: a scratch dir holding `to_usr`, a symlink
// to /usr.  Each path below resolves, once its missing levels are built, to /Volumes/cli_contract_bb:
// a mount root the floor must refuse in every mode.
use cli_contract::{check_destination, check_output_file, Clobber, CreateMode, DestPolicy};
use std::path::Path;

fn main() {
    std::env::set_current_dir(std::env::args().nth(1).unwrap()).unwrap();
    let all = DestPolicy::Create(CreateMode::All);
    for dir in [
        "to_usr/../Volumes/cli_contract_bb",
        "to_usr/zz/../../Volumes/cli_contract_bb",
    ] {
        println!("check_destination({dir}, All): {:?}", check_destination(Path::new(dir), &all));
    }
    let file = "to_usr/zz/../../Volumes/cli_contract_bb";
    println!(
        "check_output_file({file}, All, Allow): {:?}",
        check_output_file(Path::new(file), CreateMode::All, Clobber::Allow)
    );
}
