#![cfg(unix)]
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::{Command, Stdio},
};

fn read_line_bounded(file: &mut File) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut pollfd = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(
            unsafe { libc::poll(&mut pollfd, 1, 3000) },
            1,
            "helper output timed out"
        );
        let mut byte = [0];
        file.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            break;
        }
        bytes.push(byte[0]);
        assert!(bytes.len() < 65536);
    }
    String::from_utf8(bytes).unwrap()
}

#[test]
fn pty_disables_input_echo_before_ready() {
    let mut master = -1;
    let mut slave = -1;
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    let mut child = Command::new(env!("CARGO_BIN_EXE_openidea-collab-helper"))
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(slave);
    let ready = read_line_bounded(&mut master);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&ready).unwrap()["type"],
        "ready"
    );
    let secret = "LOCAL_TEST_CREDENTIAL_SHOULD_NOT_ECHO";
    writeln!(master, "{{\"type\":\"invalid\",\"token\":\"{secret}\"}}").unwrap();
    let response = read_line_bounded(&mut master);
    assert!(!response.contains(secret), "PTY echoed sensitive input");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&response).unwrap()["type"],
        "error"
    );
    child.kill().unwrap();
    child.wait().unwrap();
}
