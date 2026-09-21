//! Input state machine
//!
//! Defines the states of the IME and transitions between them.

use super::candidate::{CandidateList, CandidateSource};
use super::preedit::Preedit;

/// One 文節 of a conversion: a stretch of the reading with its own
/// candidate list. A conversion starts as a single segment over the whole
/// reading; Shift+←/→ carve it up, and ←/→ then move between the pieces.
#[derive(Debug, Clone)]
pub struct Segment {
    /// The (settled) reading this segment converts
    pub reading: String,
    /// Its candidates — the source view's rows while `filter` is set
    pub candidates: CandidateList,
    /// The Ctrl+R source filter narrowing this segment's window; `None`
    /// shows its full list. Kept per segment so moving the focus away and
    /// back finds the view as it was left
    pub filter: Option<CandidateSource>,
}

impl Segment {
    /// A segment showing its full list.
    pub fn new(reading: impl Into<String>, candidates: CandidateList) -> Self {
        Self {
            reading: reading.into(),
            candidates,
            filter: None,
        }
    }

    /// The text this segment commits: the selected candidate, or the raw
    /// reading when the list is empty (a source view narrowed to nothing
    /// displays the reading, so that is what committing produces — never
    /// an empty commit that would eat the composition).
    pub fn selected_text(&self) -> &str {
        self.candidates.selected_text().unwrap_or(&self.reading)
    }
}

/// The current state of the IME
#[derive(Debug, Clone, Default)]
pub enum InputState {
    /// No input, waiting for user to type
    #[default]
    Empty,

    /// Composing mode - building preedit text (hiragana, katakana, or alphabet)
    Composing {
        /// The preedit string being composed
        preedit: Preedit,
    },

    /// Conversion mode - selecting from candidates
    Conversion {
        /// The preedit string showing conversion result
        preedit: Preedit,
        /// The segments the reading is split into: one over the whole
        /// reading until the user resizes it. Never empty
        segments: Vec<Segment>,
        /// The focused segment — the one the candidate window shows and
        /// the candidate keys act on. Always a valid index
        focus: usize,
    },
}

impl InputState {
    /// Check if the engine is in the Empty (idle) state
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Get the current preedit if any
    pub fn preedit(&self) -> Option<&Preedit> {
        match self {
            Self::Empty => None,
            Self::Composing { preedit, .. } => Some(preedit),
            Self::Conversion { preedit, .. } => Some(preedit),
        }
    }

    /// Get mutable reference to preedit
    pub fn preedit_mut(&mut self) -> Option<&mut Preedit> {
        match self {
            Self::Empty => None,
            Self::Composing { preedit, .. } => Some(preedit),
            Self::Conversion { preedit, .. } => Some(preedit),
        }
    }

    /// The conversion's segments, if in the Conversion state.
    pub fn segments(&self) -> Option<&[Segment]> {
        match self {
            Self::Conversion { segments, .. } => Some(segments),
            _ => None,
        }
    }

    /// Index of the focused segment, if in the Conversion state.
    pub fn focus(&self) -> Option<usize> {
        match self {
            Self::Conversion { focus, .. } => Some(*focus),
            _ => None,
        }
    }

    /// The focused segment, if in the Conversion state.
    pub fn focused_segment(&self) -> Option<&Segment> {
        match self {
            Self::Conversion {
                segments, focus, ..
            } => segments.get(*focus),
            _ => None,
        }
    }

    /// The focused segment, mutable.
    pub fn focused_segment_mut(&mut self) -> Option<&mut Segment> {
        match self {
            Self::Conversion {
                segments, focus, ..
            } => segments.get_mut(*focus),
            _ => None,
        }
    }

    /// Whether the conversion has been split into more than one segment.
    pub fn is_segmented(&self) -> bool {
        self.segments().is_some_and(|s| s.len() > 1)
    }

    /// The active source filter of the focused segment's window
    pub fn filter(&self) -> Option<CandidateSource> {
        self.focused_segment().and_then(|s| s.filter)
    }

    /// The reading the focused segment's list was built from, if in the
    /// Conversion state.
    pub fn reading(&self) -> Option<&str> {
        self.focused_segment().map(|s| s.reading.as_str())
    }

    /// The focused segment's candidates in conversion state
    pub fn candidates(&self) -> Option<&CandidateList> {
        self.focused_segment().map(|s| &s.candidates)
    }

    /// Get mutable reference to the focused segment's candidates
    pub fn candidates_mut(&mut self) -> Option<&mut CandidateList> {
        self.focused_segment_mut().map(|s| &mut s.candidates)
    }
}
