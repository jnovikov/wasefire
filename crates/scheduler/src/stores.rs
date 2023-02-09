// Copyright 2022 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use alloc::collections::{BTreeSet, VecDeque};

use board::Event;
use interpreter::Store;

use crate::event::{Handler, Key};
use crate::{Memory, Trap};

#[derive(Debug, Default)]
pub struct Applet {
    store: Store<'static>,

    /// Pending events.
    events: VecDeque<Event>,

    /// Whether we returned from a callback.
    done: bool,

    handlers: BTreeSet<Handler>,
}

impl Applet {
    pub fn store_mut(&mut self) -> &mut Store<'static> {
        &mut self.store
    }

    pub fn memory(&mut self) -> Memory {
        Memory::new(&mut self.store)
    }

    pub fn push(&mut self, event: Event) {
        const MAX_EVENTS: usize = 5;
        if !self.handlers.contains(&Key::from(&event)) {
            // This can happen after an event is disabled and the event queue of the board is
            // flushed.
            logger::trace!("Discarding {}", logger::Debug2Format(&event));
        } else if self.events.contains(&event) {
            logger::trace!("Merging {}", logger::Debug2Format(&event));
        } else if self.events.len() < MAX_EVENTS {
            logger::debug!("Pushing {}", logger::Debug2Format(&event));
            self.events.push_back(event);
        } else {
            logger::warn!("Dropping {}", logger::Debug2Format(&event));
        }
    }

    /// Returns the next event action.
    pub fn pop(&mut self) -> EventAction {
        if core::mem::replace(&mut self.done, false) {
            return EventAction::Reply;
        }
        match self.events.pop_front() {
            Some(event) => EventAction::Handle(event),
            None => EventAction::Wait,
        }
    }

    pub fn done(&mut self) {
        self.done = true;
    }

    pub fn enable(&mut self, handler: Handler) -> Result<(), Trap> {
        match self.handlers.insert(handler) {
            true => Ok(()),
            false => {
                logger::warn!("Tried to overwrite existing handler");
                Err(Trap)
            }
        }
    }

    pub fn disable(&mut self, key: Key) -> Result<(), Trap> {
        self.events.retain(|x| Key::from(x) != key);
        match self.handlers.remove(&key) {
            true => Ok(()),
            false => {
                logger::warn!("Tried to remove non-existing handler");
                Err(Trap)
            }
        }
    }

    pub fn get(&self, key: Key) -> Option<&Handler> {
        self.handlers.get(&key)
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

/// Action when waiting for callbacks.
#[derive(Debug)]
pub enum EventAction {
    /// Should handle the event.
    Handle(Event),

    /// Should resume execution (we handled at least one event).
    Reply,

    /// Should suspend execution until an event is available.
    Wait,
}
