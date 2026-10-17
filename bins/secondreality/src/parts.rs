//! The part registry: the names `--part` accepts, and what they construct.
//!
//! The shell grows a part per file as Phase 2 lands: the test card, and the
//! demo's own parts in the order they were ported. Adding one means adding an
//! entry here and a module beside this file — the registry is what the shell's
//! run order is measured against (`shell::resolve`) and what a `--block` plan
//! is stopped by.
//!
//! The names here are the port's own. Which of them the demo's run order
//! (`shell::PART_ORDER`) has used, and which the port has therefore still to
//! port, is the shell's table to say.

pub mod beglogo;
pub mod cred;
pub mod endlogo;
pub mod endscrl;
pub mod minvball;
pub mod mntscrl;
pub mod poslist;
pub mod rayscrl;
pub mod tunneli;

use sr_engine::part::Part;
use sr_engine::testcard::TestCard;

/// One registry row: a part's name and the constructor that builds it.
pub type PartEntry = (&'static str, fn() -> Box<dyn Part>);

/// Every part this binary can run, as (name, constructor).
pub fn registry() -> &'static [PartEntry] {
    &[
        ("testcard", || Box::new(TestCard::new())),
        // `BEG/BEG.C`, the block-2 entry point (`MAIN/U2.ASM:801-811`).
        ("beglogo", || Box::new(beglogo::Beglogo::new())),
        // `TUNNELI/TUN10.PAS`, block 2's third part (`MAIN/U2.ASM:816-826`).
        ("tunneli", || Box::new(tunneli::Tunneli::new())),
        // `FOREST/READ2.PAS`, block 3's entry (`MAIN/U2.ASM:856-866`).
        ("mntscrl", || Box::new(mntscrl::Mntscrl::new())),
        // `DOTS/MAIN.C`, block 3's fourth (`MAIN/U2.ASM:856-874`).
        ("minvball", || Box::new(minvball::Minvball::new())),
        // `WATER/DEMO.PAS`, block 3's fifth (`MAIN/U2.ASM:856-874`).
        ("rayscrl", || Box::new(rayscrl::Rayscrl::new())),
        // `END/END.C`, block 5's entry (`MAIN/U2.ASM:906-918`).
        ("endlogo", || Box::new(endlogo::Endlogo::new())),
        // `CREDITS/MAIN.C`, block 5's second part (`MAIN/U2.ASM:912-913`).
        ("cred", || Box::new(cred::Cred::new())),
        // `ENDSCRL/MAIN.C`, block 5's last (`MAIN/U2.ASM:914-917`).
        ("endscrl", || Box::new(endscrl::Endscrl::new())),
    ]
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
