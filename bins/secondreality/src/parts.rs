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

pub mod alku;
pub mod beglogo;
pub mod coman;
pub mod cred;
pub mod ddstars;
pub mod endlogo;
pub mod endscrl;
pub mod glenz;
pub mod jplogo;
pub mod lnszoom;
pub mod minvball;
pub mod mntscrl;
pub mod msrand;
pub mod pam;
pub mod panicend;
pub mod plzpart;
pub mod rayscrl;
pub mod sin1024;
pub mod techno;
pub mod tunneli;
pub mod u2a;
pub mod u2e;
pub mod vault;
pub mod visu;

#[cfg(test)]
pub(crate) mod test_rig;

use sr_engine::part::Part;
use sr_engine::testcard::TestCard;

/// One registry row: a part's name and the constructor that builds it.
pub type PartEntry = (&'static str, fn() -> Box<dyn Part>);

/// Every part this binary can run, as (name, constructor).
pub fn registry() -> &'static [PartEntry] {
    &[
        ("testcard", || Box::new(TestCard::new())),
        // `ALKU/MAIN.C`, block 1's entry (`MAIN/U2.ASM:790-792`).
        ("alku", || Box::new(alku::Alku::new())),
        // `VISU/C/U2A.C`, block 1's second part (`MAIN/U2.ASM:793-799`).
        ("u2a", || Box::new(u2a::U2a::new())),
        // `PAM/OUTTAA.C`, block 1's third part (`MAIN/U2.ASM:793-799`).
        ("pam", || Box::new(pam::Pam::new())),
        // `BEG/BEG.C`, the block-2 entry point (`MAIN/U2.ASM:801-811`).
        ("beglogo", || Box::new(beglogo::Beglogo::new())),
        // `GLENZ/MAIN.C` over its own `VEC.ASM`/`MATH.ASM`/`NEW.ASM` — the
        // zoom-and-bounce opening and the two polyhedra, block 2's second
        // part (`MAIN/U2.ASM:812-815`, `exe4`; the block-1 run reaches it
        // through the `restartmus` at `:813-818`).
        ("glenz", || Box::new(glenz::Glenz::new())),
        // `TUNNELI/TUN10.PAS`, block 2's third part (`MAIN/U2.ASM:816-826`).
        ("tunneli", || Box::new(tunneli::Tunneli::new())),
        // `FOREST/READ2.PAS`, block 3's entry (`MAIN/U2.ASM:856-866`).
        ("mntscrl", || Box::new(mntscrl::Mntscrl::new())),
        // `LENS/MAIN.C` over `ASM.ASM` — the face wipe, the lens and the
        // rotazoomer, block 3's second (`MAIN/U2.ASM:856-874`).
        ("lnszoom", || Box::new(lnszoom::Lnszoom::new())),
        // `PLZPART/PLZ.C` over `VECT.C`, block 3's third (`MAIN/U2.ASM:856-874`).
        ("plzpart", || Box::new(plzpart::Plzpart::new())),
        // `DOTS/MAIN.C`, block 3's fourth (`MAIN/U2.ASM:856-874`).
        ("minvball", || Box::new(minvball::Minvball::new())),
        // `WATER/DEMO.PAS`, block 3's fifth (`MAIN/U2.ASM:856-874`).
        ("rayscrl", || Box::new(rayscrl::Rayscrl::new())),
        // `DDSTARS/KOE.ASM` over `STARS.ASM`, block U's only part — the
        // hidden one, and the only way `SECOND U` reaches it
        // (`MAIN/U2.ASM:889-896`).
        ("ddstars", || Box::new(ddstars::Ddstars::new())),
        // `END/END.C`, block 5's entry (`MAIN/U2.ASM:906-918`).
        ("endlogo", || Box::new(endlogo::Endlogo::new())),
        // `CREDITS/MAIN.C`, block 5's second part (`MAIN/U2.ASM:912-913`).
        ("cred", || Box::new(cred::Cred::new())),
        // `ENDSCRL/MAIN.C`, block 5's last (`MAIN/U2.ASM:914-917`).
        ("endscrl", || Box::new(endscrl::Endscrl::new())),
        // `TECHNO/KOE.C`, the interference, the bars and the troll.
        ("techno", || Box::new(techno::Techno::new())),
        // `PANIC/SHUTDOWN.C`, block 2's fifth part: the fake ending that
        // inherits TECHNO's screen and collapses it to a dot
        // (`MAIN/U2.ASM:75`, `:826-827`).
        ("panicend", || Box::new(panicend::Panicend::new())),
        // `COMAN/MAIN.C` over `ASM.ASM` and the generated `THELOOP.INC` — the
        // 3D sinus field, block 3's sixth part (`MAIN/U2.ASM:856-874`).
        ("3dsinfld", || Box::new(coman::Coman::new())),
        // `JPLOGO/JP.C` over `ASM.ASM`, the generated `ZOOM.INC` and
        // `READP.C` — the jelly logo, block 3's last part
        // (`MAIN/U2.ASM:856-874`).
        ("jplogo", || Box::new(jplogo::Jplogo::new())),
        // `VISU/C/U2E.C` — the city flythrough, block 3's tail after the
        // restart and block 4's entry (`MAIN/U2.ASM:880-887`).
        ("u2e", || Box::new(u2e::U2e::new())),
        // `GRID/MAIN.C` over `ASM.ASM` and the generated `LINEBLIT.INC` — the
        // vault's line rider, one of the three unreleased development effects
        // (`GRID/P.EXE`). Not in the demo's sequence; silent and unanchored,
        // so a `--part grid` run numbers its own frames from zero.
        ("grid", || Box::new(vault::grid::Grid::new())),
        // `HARD/B.C` + `BP.C` over `BA.ASM` and the c-prefixed `VEC.ASM`
        // engine — the vault's dots, grid and glenz cube (`HARD/B.EXE`). Not
        // in the demo's sequence; silent and unanchored, so a `--part hard`
        // run numbers its own frames from zero.
        ("hard", || Box::new(vault::hard::part::Hard::new())),
        // `TWIST/MAIN.C` over `TWIST.ASM`, `ASM.ASM` and the generated
        // `TWSTLOOP.INC` — the vault's planar twister (`TWIST/T.EXE`). Not in
        // the demo's sequence; silent and unanchored, so a `--part twist` run
        // numbers its own frames from zero.
        ("twist", || Box::new(vault::twist::Twist::new())),
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
