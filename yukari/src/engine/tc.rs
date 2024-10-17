use std::str::FromStr;

// Time control represents the current time left on our clock, and the time
#[derive(Clone, Copy, Debug)]
pub struct TimeControl {
    /// Current time remaining on our clock in seconds
    remaining: f32,
    /// Mode in which the clock is operating
    mode: TimeMode,
    /// Number of moves made so far.
    move_number: u32,
}

impl TimeControl {
    /// Construct a new instance with the base time on the clock
    #[must_use]
    pub const fn new(mode: TimeMode) -> Self {
        Self {
            remaining: match mode {
                TimeMode::St(time) => time as f32,
                TimeMode::Incremental { base, .. } | TimeMode::Classical { base, .. } => base,
            },
            mode,
            move_number: 0,
        }
    }

    /// Set the time using a centisecond value
    pub fn set_remaining(&mut self, centiseconds: f32) {
        self.remaining = centiseconds / 100.0;
    }

    /// Increment the move number.
    pub fn increment_moves(&mut self) {
        self.move_number += 1;
    }

    /// Compute the time to search.
    #[must_use]
    pub fn search_time(&self) -> f32 {
        match self.mode {
            TimeMode::St(secs) => (secs as f32) - 0.02,
            TimeMode::Incremental { base: _, increment } => {
                let remaining = self.remaining - 0.02;
                remaining.min((remaining + increment) / 30.0)
            }
            TimeMode::Classical { base: _, mps } => {
                let remaining = self.remaining - 0.02;
                let mps = mps as i32;
                let move_number = self.move_number as i32;
                let mut movesleft = mps - move_number;

                // Add the moves per session to get a positive number.
                while movesleft <= 0 {
                    movesleft += mps;
                }

                remaining / (movesleft as f32)
            }
        }
    }
}

/// Time controls can be operating in several modes which have different interpretations
#[derive(Clone, Copy, Debug)]
pub enum TimeMode {
    /// St mode has a fixed seconds per move
    St(u32),
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
            "st" => {
                // Parse out seconds per move
                let secs = u32::from_str(args[0]).map_err(|_| ())?;
                Ok(Self::St(secs))
            }
            "level" => {
                // Figure out if the mode is incremental or classical
                let mps = u32::from_str(args[0]).map_err(|_| ())?;
                let base = Self::parse_time(args[1]).ok_or(())?;
                if mps == 0 {
                    // Incremental
                    // In incremental we need the increment to add after each move
                    let inc = f32::from_str(args[2]).map_err(|_| ())?;
                    Ok(Self::Incremental {
                        base,
                        increment: inc,
                    })
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
    /// Parses a time that might be in min or min:sec format
    fn parse_time(s: &str) -> Option<f32> {
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
