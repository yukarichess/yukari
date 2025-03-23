use std::str::FromStr;

// Time control represents the current time left on our clock, and the time
#[derive(Clone, Copy, Debug)]
pub struct TimeControl {
    /// Current time remaining on our clock in seconds
    remaining: f32,
    /// Mode in which the clock is operating
    pub mode: TimeMode,
    /// Number of moves made so far.
    move_number: u32,
}

impl TimeControl {
    /// Construct a new instance with the base time on the clock
    #[must_use]
    pub const fn new(mode: TimeMode) -> Self {
        Self {
            remaining: match mode {
                TimeMode::MoveTime(time) => time as f32 / 1000.0,
                TimeMode::Incremental { base, .. } | TimeMode::Classical { base, .. } => base,
            },
            mode,
            move_number: 0,
        }
    }

    /// Set the time using a millisecond value
    pub fn set_remaining(&mut self, milliseconds: f32) {
        self.remaining = milliseconds / 1000.0;
    }

    /// Reset the move number.
    pub fn reset_moves(&mut self) {
        self.move_number = 0;
    }

    /// Increment the move number.
    pub fn increment_moves(&mut self) {
        self.move_number += 1;
    }

    /// Compute the soft and hard time limits to search.
    #[must_use]
    pub fn search_time(&self) -> (f32, f32) {
        match self.mode {
            TimeMode::MoveTime(millisecs) => {
                let secs = (millisecs as f32 / 1000.0) - 0.02;
                (secs, secs)
            }
            TimeMode::Incremental { base: _, increment } => {
                let move_number = self.move_number as f32;
                let expected_length = (59.3 + move_number.mul_add(-2330.0, 72830.0) / (move_number.mul_add(move_number, move_number * 10.0) + 2644.0)) / 2.0;
                let remaining = self.remaining - 0.02;
                let soft = remaining.min((remaining - increment) / expected_length + increment);
                let hard = remaining / 3.0;
                (soft, hard)
            }
            TimeMode::Classical { base: _, mps } => {
                let remaining = self.remaining - 0.02;
                let mps = mps as i32;
                let move_number = (self.move_number / 2) as i32;
                let mut movesleft = mps - move_number;

                // Add the moves per session to get a positive number.
                while movesleft <= 0 {
                    movesleft += mps;
                }

                let remaining = remaining / (movesleft as f32);
                (remaining, remaining)
            }
        }
    }
}

/// Time controls can be operating in several modes which have different interpretations
#[derive(Clone, Copy, Debug)]
pub enum TimeMode {
    /// MoveTime mode has a fixed number of milliseconds per move
    MoveTime(u32),
    /// Incremental mode gives us the whole game's clock, plus time to be added after each move
    Incremental {
        /// Base time for the game in seconds
        base: f32,
        /// Increment in seconds after each move
        increment: f32,
    },
    /// Classical time control has a base time, which is added after a certain number of moves
    Classical {
        /// Base time for the game in seconds
        base: f32,
        /// Moves per session (number of moves before time is bumped again)
        mps: u32,
    },
}

// TODO: this is probably not a great way to handle things since UCI will have it's own setup
/// This implementation parses a command line from the GUI and parses it into the correct format
impl FromStr for TimeMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        let mut parts = s.split(' ');
        // First part of the strin
        let cmd = parts.next().unwrap();
        let args = parts.collect::<Vec<_>>();
        match cmd {
            "level" => {
                // Figure out if the mode is incremental or classical
                let mps = u32::from_str(args[0]).map_err(|_| ())?;
                let base = Self::parse_xboard_level(args[1]).ok_or(())?;
                if mps == 0 {
                    // Incremental
                    // In incremental we need the increment to add after each move
                    let inc = f32::from_str(args[2]).map_err(|_| ())?;
                    Ok(Self::Incremental { base, increment: inc })
                } else {
                    // Classical
                    // In classical we already know the increment is zero
                    Ok(Self::Classical { base, mps })
                }
            }
            _ => Err(()),
        }
    }
}

impl TimeMode {
    pub fn fixed_time_per_move(&mut self, secs: f32) {
        *self = Self::MoveTime((secs * 1000.0) as u32);
    }

    pub fn base(&mut self, base_time: u32) {
        println!("info debug setting base time to {}s", (base_time as f32) / 1000.0);
        match self {
            Self::Incremental { base, increment: _ } => *base = (base_time as f32) / 1000.0,
            _ => *self = Self::Incremental { base: (base_time as f32) / 1000.0, increment: 0.0 },
        }
    }

    pub fn increment(&mut self, inc: u32) {
        match self {
            Self::Incremental { base: _, increment } => *increment = (inc as f32) / 1000.0,
            _ => *self = Self::Incremental { base: 0.0, increment: (inc as f32) / 1000.0 },
        }
    }

    /// Parses a time that might be in min or min:sec format
    fn parse_xboard_level(s: &str) -> Option<f32> {
        if let Some(sep) = s.find(':') {
            let min_part = f32::from_str(&s[0..sep]).ok()?;
            let sec_part = f32::from_str(&s[sep + 1..]).ok()?;
            Some(60.0f32.mul_add(min_part, sec_part))
        } else {
            let min = f32::from_str(s).ok()?;
            Some(60.0 * min)
        }
    }
}
