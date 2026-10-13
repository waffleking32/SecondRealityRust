//! The part registry: the names `--part` accepts, and what they construct.
//!
//! The demo shell will grow a part per file once Phase 2 lands; until then the
//! test card is the only thing there is to run. Adding a part means adding one
//! entry here and nothing else: the registry is what the shell's run order is
//! measured against (`shell::resolve`) and what a `--block` plan is stopped by.
//!
//! The names here are the port's own. Which of them the demo's run order
//! (`shell::PART_ORDER`) has used, and which the port has therefore still to
//! port, is the shell's table to say.

use sr_engine::part::Part;
use sr_engine::testcard::TestCard;

/// Every part this binary can run, as (name, constructor).
pub fn registry() -> &'static [(&'static str, fn() -> Box<dyn Part>)] {
    &[("testcard", || Box::new(TestCard::new()))]
}

/// The registered name `name` names, or `None`.
///
/// Returns the registry's own `&'static str` rather than the caller's, so a
/// name resolved from the command line can be carried in a run's plan without
/// borrowing the argument list.
pub fn registered(name: &str) -> Option<&'static str> {
    registry()
        .iter()
        .find(|(registered, _)| *registered == name)
        .map(|(registered, _)| *registered)
}

/// A fresh instance of `name`, or `None` if it is not registered.
pub fn new_part(name: &str) -> Option<Box<dyn Part>> {
    registry()
        .iter()
        .find(|(registered, _)| *registered == name)
        .map(|(_, construct)| construct())
}
