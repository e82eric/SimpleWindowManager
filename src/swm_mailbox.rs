//! `swm-mailbox watch`: prints this machine's mailbox folders, and what changes
//! in them, for the SWM of another machine to show. See
//! `extensions::spool::watch` and "Mailboxes of other machines" in the README.
use simple_window_manager_rs::extensions::spool::{remote, watch};
use std::path::PathBuf;

const USAGE: &str = r"usage: swm-mailbox watch [--root FOLDER]
       swm-mailbox show NAME ADDRESS [PROGRAM]
       swm-mailbox version

watch    print the mailbox folders of this machine (%LOCALAPPDATA%\swm\mailbox,
         or FOLDER) and what changes in them, until the output closes
show     connect to the machine at ADDRESS as SWM does and print what its
         mailboxes list under NAME, and why not if it cannot; PROGRAM is
         where swm-mailbox.exe is there, if not %LOCALAPPDATA%\swm\bin
version  print the version of what watch prints";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = &mut std::io::stdout().lock();
    // Each ends when whoever reads the output has gone.
    let ended = match args[..] {
        ["version"] => {
            println!("{}", watch::VERSION);
            return;
        }
        ["watch"] => match watch::root() {
            Some(root) => watch::run(&root, out),
            None => {
                eprintln!("swm-mailbox: LOCALAPPDATA is not set, so there is no mailbox folder");
                std::process::exit(1);
            }
        },
        ["watch", "--root", folder] => watch::run(&PathBuf::from(folder), out),
        ["show", name, address] => remote::show(name, address, None, out),
        ["show", name, address, program] => remote::show(name, address, Some(program), out),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(error) = ended {
        eprintln!("swm-mailbox: {error}");
    }
}
