//! Checks the keys that Husker reads from the bytes of a real terminal.
//!
//! crossterm reads keys from standard input when it is a terminal. The test
//! puts a pseudo-terminal in the place of standard input and writes to it
//! what a terminal program sends for each key. The process has one standard
//! input, so every case runs in one test.

#![cfg(unix)]

use std::fs::File;
use std::io::Write;
use std::panic;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::enable_raw_mode;
use husker::input::Input;
use rustix::fs::{Mode, OFlags, open};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::stdio::dup2_stdin;

/// How long one key may take to arrive.
const KEY_LIMIT: Duration = Duration::from_secs(5);
/// How long the whole test may take. A read without a ready key never ends.
const TEST_LIMIT: Duration = Duration::from_secs(30);

fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

#[test]
fn bytes_from_a_terminal_arrive_as_keys() {
    let (done, finished) = mpsc::channel();
    let reader = thread::spawn(move || {
        read_keys_from_a_pseudo_terminal();
        let _ = done.send(());
    });
    match finished.recv_timeout(TEST_LIMIT) {
        Ok(()) => {}
        Err(RecvTimeoutError::Timeout) => panic!("the keys were not read within {TEST_LIMIT:?}"),
        // The thread failed an assertion. Fail with its message.
        Err(RecvTimeoutError::Disconnected) => panic::resume_unwind(reader.join().unwrap_err()),
    }
}

fn read_keys_from_a_pseudo_terminal() {
    // The controller is the side of the terminal program, such as Terminal.app.
    let controller =
        openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("open a pseudo-terminal");
    grantpt(&controller).unwrap();
    unlockpt(&controller).unwrap();
    let name = ptsname(&controller, Vec::new()).unwrap();
    let terminal = open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY,
        Mode::empty(),
    )
    .unwrap();
    dup2_stdin(&terminal).unwrap();
    // Raw mode passes every byte on at once, as it does while Husker runs.
    enable_raw_mode().unwrap();

    let mut controller = File::from(controller);
    let mut input = Input::terminal();
    let none = KeyModifiers::NONE;
    let alt = KeyModifiers::ALT;
    // Each case is one or more writes of the terminal program and the keys
    // that Husker reads from them.
    let cases: &[(&[&[u8]], Vec<Event>)] = &[
        (&[b"\x1b[A"], vec![key(KeyCode::Up, none)]),
        (&[b"\x1b"], vec![key(KeyCode::Esc, none)]),
        // A plain arrow while the terminal is in application cursor mode.
        (&[b"\x1bOA"], vec![key(KeyCode::Up, none)]),
        // Alt+arrow as xterm sends it, and as the Terminal.app key mappings
        // of the usage guide send it.
        (&[b"\x1b[1;3A"], vec![key(KeyCode::Up, alt)]),
        (&[b"\x1b[1;3B"], vec![key(KeyCode::Down, alt)]),
        // Option+arrow as Terminal.app sends it while Option acts as Meta.
        (&[b"\x1b\x1b[A"], vec![key(KeyCode::Up, alt)]),
        (&[b"\x1b\x1b[B"], vec![key(KeyCode::Down, alt)]),
        (&[b"\x1b\x1b[C"], vec![key(KeyCode::Right, alt)]),
        (&[b"\x1b\x1b[D"], vec![key(KeyCode::Left, alt)]),
        // The same key while the terminal is in application cursor mode.
        (&[b"\x1b\x1bOA"], vec![key(KeyCode::Up, alt)]),
        (&[b"\x1b\x1bOB"], vec![key(KeyCode::Down, alt)]),
        // The same key when the terminal sends the Esc in a write of its own.
        (&[b"\x1b", b"\x1b[A"], vec![key(KeyCode::Up, alt)]),
        (&[b"\x1b", b"\x1b[B"], vec![key(KeyCode::Down, alt)]),
        (&[b"\x1b", b"\x1bOA"], vec![key(KeyCode::Up, alt)]),
        // A held key repeats without a gap.
        (
            &[b"\x1b\x1b[B\x1b\x1b[B"],
            vec![key(KeyCode::Down, alt), key(KeyCode::Down, alt)],
        ),
        // An Esc before the key stays an Esc.
        (
            &[b"\x1b\x1b\x1b\x1b[A"],
            vec![key(KeyCode::Esc, none), key(KeyCode::Up, alt)],
        ),
        // Any other key after Esc stays as crossterm reads it.
        (
            &[b"\x1b\x1b[H"],
            vec![
                key(KeyCode::Esc, none),
                key(KeyCode::Char('['), none),
                key(KeyCode::Char('H'), KeyModifiers::SHIFT),
            ],
        ),
    ];
    for (writes, keys) in cases {
        let sent = writes
            .iter()
            .map(|bytes| bytes.escape_ascii().to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        for (index, bytes) in writes.iter().enumerate() {
            if index > 0 {
                // crossterm reads the earlier write before this one arrives.
                assert!(input.poll(KEY_LIMIT).unwrap(), "no key for {sent}");
            }
            controller.write_all(bytes).unwrap();
        }
        for expected in keys {
            assert!(input.poll(KEY_LIMIT).unwrap(), "no key for {sent}");
            assert_eq!(input.read().unwrap(), *expected, "key for {sent}");
        }
        assert!(
            !input.poll(Duration::ZERO).unwrap(),
            "an extra key for {sent}"
        );
    }
}
