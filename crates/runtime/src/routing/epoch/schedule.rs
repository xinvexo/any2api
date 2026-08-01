use std::collections::{BTreeMap, HashMap, HashSet};

use tokio::time::Instant;

#[derive(Debug, Default)]
pub(super) struct WakeSchedule {
    by_slot: HashMap<u64, Instant>,
    by_deadline: BTreeMap<Instant, HashSet<u64>>,
}

impl WakeSchedule {
    pub(super) fn schedule(&mut self, slot: u64, deadline: Instant) -> bool {
        if self.by_slot.get(&slot) == Some(&deadline) {
            return false;
        }
        if let Some(previous) = self.by_slot.insert(slot, deadline) {
            self.remove_reverse(previous, slot);
        }
        self.by_deadline.entry(deadline).or_default().insert(slot);
        true
    }

    pub(super) fn cancel(&mut self, slot: u64) -> bool {
        let Some(deadline) = self.by_slot.remove(&slot) else {
            return false;
        };
        self.remove_reverse(deadline, slot);
        true
    }

    pub(super) fn next_deadline(&self) -> Option<Instant> {
        self.by_deadline
            .first_key_value()
            .map(|(deadline, _)| *deadline)
    }

    pub(super) fn remove_due(&mut self, now: Instant) -> bool {
        let mut removed = false;
        while self
            .by_deadline
            .first_key_value()
            .is_some_and(|(deadline, _)| *deadline <= now)
        {
            let (_, slots) = self
                .by_deadline
                .pop_first()
                .expect("checked first scheduler wake deadline");
            for slot in slots {
                self.by_slot.remove(&slot);
            }
            removed = true;
        }
        removed
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.by_slot.len()
    }

    fn remove_reverse(&mut self, deadline: Instant, slot: u64) {
        let remove_deadline = self.by_deadline.get_mut(&deadline).is_some_and(|slots| {
            slots.remove(&slot);
            slots.is_empty()
        });
        if remove_deadline {
            self.by_deadline.remove(&deadline);
        }
    }
}
