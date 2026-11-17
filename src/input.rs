//! Terminal input for the interface.
//!
//! Some terminals send Alt+arrow as the Esc byte followed by the bytes of the
//! plain arrow key: `ESC ESC [ A` for Alt+Up, or `ESC ESC O A` in application
//! cursor mode. macOS Terminal.app does this when its Option key acts as
//! Meta. crossterm does not read those bytes as Alt+Up. It reads them in one
//! of two ways, and [`Input`] joins both into one Alt+arrow key:
//!
//! - All bytes in one read become three keys: Esc, `[` or `O`, and `A`.
//! - The first Esc byte in a read of its own becomes Esc. The other bytes
//!   then become a plain Up.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// How long a pressed Esc waits for an arrow key that the terminal sends in
/// a write of its own. An Esc pressed alone still feels immediate.
pub const ARROW_WAIT: Duration = Duration::from_millis(10);

/// Where terminal events come from. The interface reads the terminal, and
/// tests read a script.
pub trait EventSource {
    /// Reports whether an event is ready, waiting up to `timeout` for one.
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
    /// Returns the next event and waits for it when none is ready.
    fn read(&mut self) -> io::Result<Event>;
}

/// The terminal that Husker runs in, read through crossterm.
pub struct TerminalEvents;

impl EventSource for TerminalEvents {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }
}

/// Terminal events with every Esc-prefixed arrow key joined into Alt+arrow.
///
/// Read every terminal event of the interface through one `Input`. An event
/// read from crossterm directly would overtake the events that `Input` has
/// read ahead.
pub struct Input<S = TerminalEvents> {
    source: S,
    /// Events read from the source ahead of their turn, oldest first.
    ahead: VecDeque<Event>,
}

impl Input {
    /// Reads the terminal that Husker runs in.
    pub fn terminal() -> Self {
        Self::new(TerminalEvents)
    }
}

impl<S: EventSource> Input<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            ahead: VecDeque::new(),
        }
    }

    /// Reports whether an event is ready, waiting up to `timeout` for one.
    pub fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        Ok(!self.ahead.is_empty() || self.source.poll(timeout)?)
    }

    /// Returns the next event and waits for it when none is ready. A pressed
    /// Esc waits up to [`ARROW_WAIT`] for the arrow key that may follow it.
    pub fn read(&mut self) -> io::Result<Event> {
        let event = match self.ahead.pop_front() {
            Some(event) => event,
            None => self.source.read()?,
        };
        if !is_press(&event, KeyCode::Esc, KeyModifiers::NONE) {
            return Ok(event);
        }
        match self.arrow_after_escape() {
            Ok(Some(arrow)) => Ok(Event::Key(KeyEvent::new(arrow, KeyModifiers::ALT))),
            Ok(None) => Ok(event),
            Err(error) => {
                // The Esc stays first in line for the next read.
                self.ahead.push_front(event);
                Err(error)
            }
        }
    }

    /// The arrow of an Alt+arrow key whose Esc was just read. Takes the keys
    /// after the Esc only when they belong to the arrow.
    fn arrow_after_escape(&mut self) -> io::Result<Option<KeyCode>> {
        // The terminal sent the key in one write, so its three keys are ready
        // together. `Input` does not wait for them: a `[` or a letter that
        // comes later is most likely the rest of a plain arrow key whose
        // bytes arrived in two reads.
        if self.waiting(0, Duration::ZERO)?.is_some_and(starts_arrow) {
            let letter = self.waiting(1, Duration::ZERO)?;
            let Some(arrow) = letter.and_then(arrow_of_letter) else {
                return Ok(None);
            };
            self.ahead.drain(..2);
            return Ok(Some(arrow));
        }
        // The terminal sent the Esc in a write of its own. The plain arrow
        // key then follows sooner than a person can press a second key.
        let next = self.waiting(0, ARROW_WAIT)?;
        let Some(arrow) = next.and_then(plain_arrow) else {
            return Ok(None);
        };
        self.ahead.pop_front();
        Ok(Some(arrow))
    }

    /// The event `index` places ahead, when the source has it ready within
    /// `timeout`. An event that was read ahead before is ready at once.
    fn waiting(&mut self, index: usize, timeout: Duration) -> io::Result<Option<&Event>> {
        while self.ahead.len() <= index {
            if !self.source.poll(timeout)? {
                return Ok(None);
            }
            let event = self.source.read()?;
            self.ahead.push_back(event);
        }
        Ok(self.ahead.get(index))
    }
}

fn is_press(event: &Event, code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(
        event,
        Event::Key(key)
            if key.kind == KeyEventKind::Press && key.code == code && key.modifiers == modifiers
    )
}

/// Whether the key is the byte after Esc in a plain arrow key. That byte is
/// `[`, or `O` while the terminal is in application cursor mode.
fn starts_arrow(event: &Event) -> bool {
    is_press(event, KeyCode::Char('['), KeyModifiers::NONE)
        || is_press(event, KeyCode::Char('O'), KeyModifiers::SHIFT)
}

/// The arrow that the last byte of a plain arrow key names. crossterm reads
/// that byte as a capital letter, which carries Shift.
fn arrow_of_letter(event: &Event) -> Option<KeyCode> {
    [
        ('A', KeyCode::Up),
        ('B', KeyCode::Down),
        ('C', KeyCode::Right),
        ('D', KeyCode::Left),
    ]
    .into_iter()
    .find(|(letter, _)| is_press(event, KeyCode::Char(*letter), KeyModifiers::SHIFT))
    .map(|(_, arrow)| arrow)
}

/// The arrow of a pressed arrow key without modifiers.
fn plain_arrow(event: &Event) -> Option<KeyCode> {
    [KeyCode::Up, KeyCode::Down, KeyCode::Right, KeyCode::Left]
        .into_iter()
        .find(|arrow| is_press(event, *arrow, KeyModifiers::NONE))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A group that is ready together with the group before it.
    const AT_ONCE: Duration = Duration::ZERO;
    /// The pause before the next key that a person presses.
    const LATER: Duration = Duration::from_millis(200);

    fn milliseconds(count: u64) -> Duration {
        Duration::from_millis(count)
    }

    /// Events in the groups a terminal delivers them in. The events of one
    /// group are ready together. A group arrives after its delay, counted
    /// from the moment the group before it is used up. Time passes only
    /// while a caller waits in `poll`.
    struct Script {
        ready: VecDeque<Event>,
        later: VecDeque<(Duration, Vec<Event>)>,
        /// The time that callers have waited in `poll`.
        waited: Duration,
        /// The number of polls that still succeed before one fails.
        polls_before_failure: Option<usize>,
    }

    impl Script {
        fn arrive(&mut self) {
            if let Some((_, group)) = self.later.pop_front() {
                self.ready.extend(group);
            }
        }
    }

    impl EventSource for Script {
        fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
            match self.polls_before_failure {
                Some(0) => {
                    self.polls_before_failure = None;
                    return Err(io::Error::other("the terminal is gone"));
                }
                Some(polls) => self.polls_before_failure = Some(polls - 1),
                None => {}
            }
            if !self.ready.is_empty() {
                return Ok(true);
            }
            let Some((delay, _)) = self.later.front_mut() else {
                self.waited += timeout;
                return Ok(false);
            };
            let wait = timeout.min(*delay);
            self.waited += wait;
            *delay -= wait;
            if delay.is_zero() {
                self.arrive();
            }
            Ok(!self.ready.is_empty())
        }

        fn read(&mut self) -> io::Result<Event> {
            // A read waits for the next group, however long that takes.
            if self.ready.is_empty() {
                self.arrive();
            }
            self.ready
                .pop_front()
                .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))
        }
    }

    fn input(groups: &[(Duration, &[Event])]) -> Input<Script> {
        Input::new(Script {
            ready: VecDeque::new(),
            later: groups
                .iter()
                .map(|(delay, group)| (*delay, group.to_vec()))
                .collect(),
            waited: Duration::ZERO,
            polls_before_failure: None,
        })
    }

    /// Reads every event of the script in order.
    fn read_all(groups: &[(Duration, &[Event])]) -> Vec<Event> {
        let mut input = input(groups);
        let mut events = Vec::new();
        while input.poll(Duration::from_secs(3600)).unwrap() {
            events.push(input.read().unwrap());
        }
        events
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn key_of_kind(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Event {
        Event::Key(KeyEvent::new_with_kind(code, modifiers, kind))
    }

    fn plain(code: KeyCode) -> Event {
        key(code, KeyModifiers::NONE)
    }

    fn alt(code: KeyCode) -> Event {
        key(code, KeyModifiers::ALT)
    }

    fn character(character: char) -> Event {
        // crossterm marks a capital letter with Shift.
        let modifiers = if character.is_uppercase() {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        };
        key(KeyCode::Char(character), modifiers)
    }

    const ARROWS: [(char, KeyCode); 4] = [
        ('A', KeyCode::Up),
        ('B', KeyCode::Down),
        ('C', KeyCode::Right),
        ('D', KeyCode::Left),
    ];

    #[test]
    fn an_arrow_key_sent_in_one_write_with_its_escape_becomes_alt_arrow() {
        for (letter, arrow) in ARROWS {
            // `[` in normal cursor mode, `O` in application cursor mode.
            for start in ['[', 'O'] {
                let group = [plain(KeyCode::Esc), character(start), character(letter)];
                assert_eq!(
                    read_all(&[(AT_ONCE, &group)]),
                    [alt(arrow)],
                    "ESC ESC {start} {letter}"
                );
            }
        }
    }

    #[test]
    fn an_arrow_key_sent_right_after_its_escape_becomes_alt_arrow() {
        for (_, arrow) in ARROWS {
            let escape = [plain(KeyCode::Esc)];
            let arrow_key = [plain(arrow)];
            // Ready together, in the middle of the wait, and at its end.
            for delay in [AT_ONCE, milliseconds(5), milliseconds(10)] {
                assert_eq!(
                    read_all(&[(AT_ONCE, &escape), (delay, &arrow_key)]),
                    [alt(arrow)],
                    "{arrow:?} after {delay:?}"
                );
            }
        }
    }

    #[test]
    fn an_arrow_key_pressed_after_an_escape_stays_an_arrow() {
        let escape = [plain(KeyCode::Esc)];
        let up = [plain(KeyCode::Up)];
        for delay in [milliseconds(11), LATER] {
            assert_eq!(
                read_all(&[(AT_ONCE, &escape), (delay, &up)]),
                [plain(KeyCode::Esc), plain(KeyCode::Up)],
                "Up after {delay:?}"
            );
        }
    }

    #[test]
    fn an_escape_pressed_alone_waits_ten_milliseconds() {
        let later_keys = [character('['), character('A')];
        let mut input = input(&[(AT_ONCE, &[plain(KeyCode::Esc)]), (LATER, &later_keys)]);
        assert_eq!(input.read().unwrap(), plain(KeyCode::Esc));
        assert_eq!(input.source.waited, milliseconds(10));
        // The keys typed later were not taken to decide about the Esc.
        assert!(!input.poll(Duration::ZERO).unwrap());
        assert_eq!(input.read().unwrap(), character('['));
        assert_eq!(input.read().unwrap(), character('A'));
    }

    #[test]
    fn the_last_escape_waits_ten_milliseconds() {
        let mut input = input(&[(AT_ONCE, &[plain(KeyCode::Esc)])]);
        assert_eq!(input.read().unwrap(), plain(KeyCode::Esc));
        assert_eq!(input.source.waited, milliseconds(10));
    }

    #[test]
    fn an_arrow_key_in_pieces_does_not_become_alt_arrow() {
        // The bytes `ESC [ A` of a plain Up in two reads: crossterm makes an
        // Esc of the first byte. The rest is not ready together with it.
        let escape = [plain(KeyCode::Esc)];
        let rest = [character('['), character('A')];
        assert_eq!(
            read_all(&[(AT_ONCE, &escape), (milliseconds(5), &rest)]),
            [plain(KeyCode::Esc), character('['), character('A')]
        );
        // The same when only the letter comes later. No key waits for it.
        let mut input = input(&[
            (AT_ONCE, &[plain(KeyCode::Esc), character('[')]),
            (milliseconds(5), &[character('A')]),
        ]);
        assert_eq!(input.read().unwrap(), plain(KeyCode::Esc));
        assert_eq!(input.source.waited, Duration::ZERO);
        assert_eq!(input.read().unwrap(), character('['));
        assert_eq!(input.read().unwrap(), character('A'));
    }

    #[test]
    fn keys_that_are_no_arrow_stay_in_order() {
        let shift = KeyModifiers::SHIFT;
        let release = KeyEventKind::Release;
        let repeat = KeyEventKind::Repeat;
        for rest in [
            vec![character('['), character('H')],
            vec![character('['), character('a')],
            vec![character('['), plain(KeyCode::Up)],
            vec![character('[')],
            vec![character('A')],
            vec![character('o'), character('A')],
            vec![character('O'), character('H')],
            // Only the exact keys that crossterm makes of the bytes join.
            vec![key(KeyCode::Char('['), KeyModifiers::ALT), character('A')],
            vec![key(KeyCode::Char('['), shift), character('A')],
            vec![key_of_kind(KeyCode::Char('['), KeyModifiers::NONE, release)],
            vec![
                character('['),
                key(KeyCode::Char('A'), KeyModifiers::ALT | shift),
            ],
            vec![character('['), key(KeyCode::Char('A'), KeyModifiers::NONE)],
            vec![
                character('['),
                key_of_kind(KeyCode::Char('A'), shift, release),
            ],
            vec![
                character('['),
                key_of_kind(KeyCode::Char('A'), shift, repeat),
            ],
            // Only a pressed arrow without modifiers joins.
            vec![key(KeyCode::Up, shift)],
            vec![alt(KeyCode::Up)],
            vec![key_of_kind(KeyCode::Up, KeyModifiers::NONE, release)],
            vec![key_of_kind(KeyCode::Up, KeyModifiers::NONE, repeat)],
            vec![plain(KeyCode::Home)],
            vec![Event::Paste("[A".into())],
            vec![Event::Resize(80, 24), plain(KeyCode::Up)],
        ] {
            let mut group = vec![plain(KeyCode::Esc)];
            group.extend(rest);
            assert_eq!(read_all(&[(AT_ONCE, &group)]), group);
        }
    }

    #[test]
    fn only_a_pressed_escape_without_modifiers_starts_an_alt_arrow() {
        for escape in [
            key_of_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Release),
            key_of_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Repeat),
            key(KeyCode::Esc, KeyModifiers::SHIFT),
            key(KeyCode::Esc, KeyModifiers::CONTROL),
        ] {
            for rest in [
                vec![character('['), character('A')],
                vec![plain(KeyCode::Up)],
            ] {
                let mut group = vec![escape.clone()];
                group.extend(rest);
                assert_eq!(read_all(&[(AT_ONCE, &group)]), group);
            }
        }
    }

    #[test]
    fn a_held_key_becomes_one_alt_arrow_per_repeat() {
        let one_write = [plain(KeyCode::Esc), character('['), character('B')];
        let two_writes = [plain(KeyCode::Esc), plain(KeyCode::Down)];
        for once in [one_write.to_vec(), two_writes.to_vec()] {
            let twice = [once.clone(), once].concat();
            assert_eq!(
                read_all(&[(AT_ONCE, &twice)]),
                [alt(KeyCode::Down), alt(KeyCode::Down)]
            );
        }
    }

    #[test]
    fn an_escape_that_was_read_ahead_joins_too() {
        // The second Esc is read while the first one is checked.
        for rest in [
            vec![character('['), character('A')],
            vec![plain(KeyCode::Up)],
        ] {
            let mut group = vec![plain(KeyCode::Esc), plain(KeyCode::Esc)];
            group.extend(rest);
            assert_eq!(
                read_all(&[(AT_ONCE, &group)]),
                [plain(KeyCode::Esc), alt(KeyCode::Up)]
            );
        }
    }

    #[test]
    fn events_read_ahead_are_ready_without_the_terminal() {
        let group = [plain(KeyCode::Esc), character('['), character('H')];
        let mut input = input(&[(AT_ONCE, &group)]);
        assert_eq!(input.read().unwrap(), plain(KeyCode::Esc));
        // Both keys left the script while the Esc was checked.
        assert!(input.source.ready.is_empty());
        assert!(input.poll(Duration::ZERO).unwrap());
        assert_eq!(input.read().unwrap(), character('['));
        assert_eq!(input.read().unwrap(), character('H'));
        assert!(!input.poll(Duration::ZERO).unwrap());
    }

    #[test]
    fn a_failed_look_ahead_loses_no_key() {
        let group = [plain(KeyCode::Esc), character('['), character('A')];
        // The poll for `[` fails, or the poll for `A`.
        for polls_before_failure in [0, 1] {
            let mut input = input(&[(AT_ONCE, &group)]);
            input.source.polls_before_failure = Some(polls_before_failure);
            assert!(input.read().is_err());
            // The Esc is first in line again.
            assert!(input.poll(Duration::ZERO).unwrap());
            assert_eq!(input.read().unwrap(), alt(KeyCode::Up));
            assert!(!input.poll(Duration::ZERO).unwrap());
        }
    }

    #[test]
    fn other_events_pass_through_unchanged() {
        let events = [
            alt(KeyCode::Up),
            plain(KeyCode::Up),
            character('K'),
            Event::Paste("text".into()),
            Event::Resize(80, 24),
            plain(KeyCode::Esc),
        ];
        assert_eq!(read_all(&[(AT_ONCE, &events)]), events);
    }
}
