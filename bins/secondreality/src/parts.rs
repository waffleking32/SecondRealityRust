//! The part registry, the names `--part` accepts and what they construct, plus the asset
//! reading every part shares. The demo's run order is `shell::plan::PART_ORDER`; every name in
//! it is registered here.

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
pub mod logo;
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

use sr_assets::Image;
use sr_engine::part::{Part, PartError};
use sr_engine::testcard::TestCard;
use sr_engine::View;

/// The CRTC state a mode set leaves: `width`×`height` from origin 0, no pan,
/// split or double scan, rows `line` pixels apart (`None`: the surface's own
/// stride) and overscan colour `border`.
pub(crate) fn mode_view(width: u16, height: u16, line: Option<u16>, border: Option<u8>) -> View {
    View { y_line_offset: line, border, ..View::new(width, height) }
}

/// The bytes of the embedded asset `rel` ([`crate::assets`]), or `part`'s error naming it.
pub(crate) fn read_asset(part: &'static str, rel: &str) -> Result<Vec<u8>, PartError> {
    crate::assets::read(rel).map(<[u8]>::to_vec).map_err(|error| PartError::new(part, error))
}

/// The picture the embedded asset `rel` holds, or `part`'s error naming it.
pub(crate) fn load_image(part: &'static str, rel: &str) -> Result<Image, PartError> {
    crate::assets::image(rel).map_err(|error| PartError::new(part, error))
}

/// One registry row: a part's name and the constructor that builds it.
pub type PartEntry = (&'static str, fn() -> Box<dyn Part>);

/// Every part this binary can run, as (name, constructor), with its original source.
pub fn registry() -> &'static [PartEntry] {
    &[
        ("testcard", || Box::new(TestCard::new())),
        ("alku", || Box::new(alku::Alku::new())), // ALKU/MAIN.C
        ("u2a", || Box::new(u2a::U2a::new())), // VISU/C/U2A.C
        ("pam", || Box::new(pam::Pam::new())), // PAM/OUTTAA.C
        ("beglogo", || Box::new(beglogo::Beglogo::new())), // BEG/BEG.C
        ("glenz", || Box::new(glenz::Glenz::new())), // GLENZ/MAIN.C
        ("tunneli", || Box::new(tunneli::Tunneli::new())), // TUNNELI/TUN10.PAS
        ("mntscrl", || Box::new(mntscrl::Mntscrl::new())), // FOREST/READ2.PAS
        ("lnszoom", || Box::new(lnszoom::Lnszoom::new())), // LENS/MAIN.C
        ("plzpart", || Box::new(plzpart::Plzpart::new())), // PLZPART/PLZ.C
        ("minvball", || Box::new(minvball::Minvball::new())), // DOTS/MAIN.C
        ("rayscrl", || Box::new(rayscrl::Rayscrl::new())), // WATER/DEMO.PAS
        ("ddstars", || Box::new(ddstars::Ddstars::new())), // DDSTARS/KOE.ASM
        ("endlogo", || Box::new(endlogo::Endlogo::new())), // END/END.C
        ("cred", || Box::new(cred::Cred::new())), // CREDITS/MAIN.C
        ("endscrl", || Box::new(endscrl::Endscrl::new())), // ENDSCRL/MAIN.C
        ("techno", || Box::new(techno::Techno::new())), // TECHNO/KOE.C
        ("panicend", || Box::new(panicend::Panicend::new())), // PANIC/SHUTDOWN.C
        ("3dsinfld", || Box::new(coman::Coman::new())), // COMAN/MAIN.C
        ("jplogo", || Box::new(jplogo::Jplogo::new())), // JPLOGO/JP.C
        ("u2e", || Box::new(u2e::U2e::new())), // VISU/C/U2E.C
        // The vault's unreleased effects: not in the demo's sequence, silent and unanchored,
        // so a run numbers its own frames from zero.
        ("grid", || Box::new(vault::grid::Grid::new())), // GRID/MAIN.C
        ("hard", || Box::new(vault::hard::part::Hard::new())), // HARD/B.C
        ("twist", || Box::new(vault::twist::Twist::new())), // TWIST/MAIN.C
    ]
}

fn entry(name: &str) -> Option<&'static PartEntry> {
    registry().iter().find(|(registered, _)| *registered == name)
}

/// The registry's own `&'static str` for `name`, so a plan can carry it without borrowing the
/// argument list.
pub fn registered(name: &str) -> Option<&'static str> {
    entry(name).map(|(registered, _)| *registered)
}

/// A fresh instance of `name`, or `None` if it is not registered.
pub fn new_part(name: &str) -> Option<Box<dyn Part>> {
    entry(name).map(|(_, construct)| construct())
}
