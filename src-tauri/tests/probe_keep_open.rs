//! Spike A 调试探针：诊断 keep-open 场景下子进程 stdio 归属。
#![cfg(windows)]

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
#[ignore = "诊断探针"]
fn probe_keep_open_stdio() {
    let bash = squirl::engine::shell::resolve_git_bash().unwrap();
    let bash2 = bash.clone();
    type Case = (&'static str, Box<dyn FnOnce() -> std::process::Child>);
    let cases: Vec<Case> = vec![
        (
            "P2: direct bash CREATE_NEW_CONSOLE, inherit stdio",
            Box::new(move || {
                let mut c = Command::new(&bash2);
                c.args(["-lic", "( exit 1 ); exec bash"]);
                c.creation_flags(0x10);
                c.spawn().unwrap()
            }),
        ),
        (
            "P1: conhost.exe bash ...",
            Box::new(move || {
                let mut c = Command::new("conhost.exe");
                c.arg(&bash).args(["-lic", "( exit 1 ); exec bash"]);
                c.creation_flags(0x10);
                c.spawn().unwrap()
            }),
        ),
    ];
    for (name, spawn) in cases {
        let mut child = spawn();
        let start = Instant::now();
        let mut outcome = String::from("ALIVE>=4s");
        while start.elapsed() < Duration::from_secs(4) {
            if let Ok(Some(st)) = child.try_wait() {
                outcome = format!("EXITED after {:?}: {st}", start.elapsed());
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        println!("--- {name}: {outcome}");
        let _ = child.kill();
        let _ = child.wait();
    }
}
