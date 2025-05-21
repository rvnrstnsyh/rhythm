use std::fmt::{Display, Formatter, Result};

use crate::types::Record;

use hex::encode;

impl Display for Record {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let _event_desc: String = match &self.event {
            Some(data) => format!("Event {} bytes", data.len()),
            None => "No Event".to_string(),
        };
        return write!(
            f,
            "Cycle {}, Phase {}, Rev {}, Timestamp {}ms, Hash 0x{}...",
            self.cycle_index,
            self.phase_index,
            self.rev_index,
            self.timestamp_ms,
            &encode(self.hash)[..17]
        );
    }
}
