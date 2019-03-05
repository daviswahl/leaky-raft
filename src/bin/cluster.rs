use std::process::Command;
use std::process::Stdio;
use std::fs::File;
use std::fs::OpenOptions;
use std::{fs,path, fmt};
use std::process::Child;
use std::thread::Thread;
use std::thread::sleep_ms;
use std::io::Write;


fn log_path(s: &str) -> String { format!("logs/{}.log", s) }
fn start_server<P: AsRef<str>+fmt::Display>(port: P, ports: P) -> Child {
    let file = OpenOptions::new().write(true).create(true).truncate(true).open(log_path(port.as_ref())).unwrap();
    let mut cmd = Command::new("target/debug/main");

    let mut pid = format!("logs/{}.pid", port);
    let port = format!("-p={}", port);
    let ports = format!("-P={}", ports);
    let child = cmd.env("RUST_LOG", "info")
        .args(&[port, ports])
        .stdout(Stdio::piped())
        .stderr(Stdio::from(file)).spawn().unwrap();


    println!("pid: {}", pid);
    let mut pidfile = OpenOptions::new().write(true).create(true).truncate(true).open(&pid).unwrap();
    pidfile.write(format!("{}", child.id()).as_ref()).unwrap();
    pidfile.flush().unwrap();
    child

}

fn main() {

    use std::sync;
    let mut children = sync::Arc::new(sync::Mutex::new(vec![]));

    fs::create_dir_all("logs").unwrap();

    children.lock().unwrap().push(start_server("13000", "13001,13002"));
    children.lock().unwrap().push(start_server("13001", "13002,13000"));
    children.lock().unwrap().push(start_server("13002", "13000,13001"));

    let mut children_killer = children.clone();

    let handler = move || {
        let mut children =  children_killer.lock().unwrap();

        while let Some(mut child) = children.pop() {
            println!("killing child");
            child.kill();
        }
        std::process::exit(0);
    };

    ctrlc::set_handler(handler.clone());


    while let Ok(None) = children.clone().lock().unwrap().first_mut().unwrap().try_wait() {
        println!("sleeping...");
        sleep_ms(100)
    }

    handler()
}

