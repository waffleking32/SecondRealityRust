//! The part registry: the names `--part` accepts, and what they construct.
//!
//! The demo shell will grow a part per file once Phase 2 lands; until then the
//! test card is the only thing there is to run. Adding a part means adding one
//! entry here and nothing else.

use sr_engine::part::Part;
use sr_engine::testcard::TestCard;

/// Every part this binary can run, as (name, constructor).
pub fn registry() -> &'static [(&'static str, fn() -> Box<dyn Part>)] {
    &[("testcard", || Box::new(TestCard::new()))]
}

/// Whether `name` names a registered part.
pub fn is_known(name: &str) -> bool {
    registry().iter().any(|(registered, _)| *registered == name)
}

/// A fresh instance of `name`, or `None` if it is not registered.
pub fn new_part(name: &str) -> Option<Box<dyn Part>> {
    registry()
        .iter()
        .find(|(registered, _)| *registered == name)
        .map(|(_, construct)| construct())
}
