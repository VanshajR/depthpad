//! Key-blocking decisions, separate from the Windows hook so they can be tested.
//!
//! Rules of HallJoy's Block Bound Keys (app.cpp KeyboardBlockHookProc, block_keys_policy.h):
//! only physical input is considered; the down decides the whole press (repeats and the up
//! follow it), so toggling mid-press never swallows an up that should arrive; keys already
//! held when blocking starts pass through; Alt and Tab can be exempted; a hotkey press itself
//! never reaches other applications.
//!
//! Hotkeys are combinations of usages, modifiers first and the trigger key last. Index 0 is the
//! pause hotkey (the router tracks the paused state, since blocking depends on it); the others
//! are reported by index for the engine to act on.

use std::collections::{HashMap, HashSet};

const ALT_TAB: [u16; 3] = [43, 226, 230]; // Tab, Left Alt, Right Alt

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Passed,
    Blocked,
    Hotkey,
}

/// What to do with one key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// Swallow the event so other applications never see it.
    pub swallow: bool,
    /// The event is a blocked key the engine must hear about (Raw Input won't see it).
    pub report: bool,
    /// The pause hotkey fired (new paused state).
    pub toggled: Option<bool>,
    /// Another hotkey fired (its index).
    pub fired: Option<usize>,
}

#[derive(Debug, Default)]
pub struct Router {
    pub bound: HashSet<u16>,
    pub block: bool,
    /// Block every key, not only bound ones (Wootility's "Enable keyboard keys" off).
    pub block_all: bool,
    pub allow_alt_tab: bool,
    /// Hotkeys; index 0 is pause (empty when it has none).
    pub hotkeys: Vec<Vec<u16>>,
    pub paused: bool,
    routes: HashMap<u16, Route>,
    held: HashSet<u16>,
}

impl Router {
    pub fn new(bound: HashSet<u16>, block: bool, allow_alt_tab: bool, hotkeys: Vec<Vec<u16>>) -> Self {
        Self { bound, block, allow_alt_tab, hotkeys, ..Default::default() }
    }

    /// The hotkey this key completes, given what is held: the longest match, so Ctrl+Shift+P
    /// wins over Ctrl+P.
    fn hotkey_for(&self, usage: u16) -> Option<usize> {
        self.hotkeys
            .iter()
            .enumerate()
            .filter(|(_, h)| h.split_last().is_some_and(|(last, mods)| *last == usage && mods.iter().all(|m| self.held.contains(m))))
            .max_by_key(|(_, h)| h.len())
            .map(|(i, _)| i)
    }

    /// Keys already down when the hook starts were delivered; their releases must pass too.
    pub fn seed_held(&mut self, usage: u16) {
        self.routes.insert(usage, Route::Passed);
        self.held.insert(usage);
    }

    /// Forget which keys are held (the hook was away and missed their releases).
    pub fn reset_held(&mut self) {
        self.held.clear();
        self.routes.clear();
    }

    pub fn route(&mut self, usage: u16, down: bool, own_foreground: bool) -> Decision {
        let (mut toggled, mut fired) = (None, None);
        let mut route = self.routes.get(&usage).copied();
        if down {
            if route.is_none() {
                let r = if let Some(i) = self.hotkey_for(usage) {
                    if i == 0 {
                        self.paused = !self.paused;
                        toggled = Some(self.paused);
                    } else {
                        fired = Some(i);
                    }
                    Route::Hotkey
                } else if self.block
                    && !self.paused
                    && (self.block_all || self.bound.contains(&usage))
                    && !(self.allow_alt_tab && ALT_TAB.contains(&usage))
                    && !own_foreground
                {
                    Route::Blocked
                } else {
                    Route::Passed
                };
                self.routes.insert(usage, r);
                route = Some(r);
            }
            self.held.insert(usage);
        } else {
            self.held.remove(&usage);
            if route.is_none() {
                return Decision { swallow: false, report: false, toggled: None, fired: None };
            }
            self.routes.remove(&usage);
        }
        let r = route.unwrap();
        Decision { swallow: r != Route::Passed, report: r == Route::Blocked, toggled, fired }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u16 = 26;
    const TAB: u16 = 43;
    const RCTRL: u16 = 228;
    const P: u16 = 19;
    const X: u16 = 27;

    fn router(block: bool) -> Router {
        Router { bound: [W, TAB].into(), block, allow_alt_tab: true, hotkeys: vec![vec![RCTRL, P]], ..Default::default() }
    }

    #[test]
    fn bound_key_blocked_and_reported() {
        let mut r = router(true);
        for down in [true, true, false] {
            let d = r.route(W, down, false);
            assert!(d.swallow && d.report);
        }
        assert!(!r.route(X, true, false).swallow);
        assert!(!r.route(X, false, false).swallow);
    }

    #[test]
    fn alt_tab_exempt() {
        let mut r = router(true);
        assert!(!r.route(TAB, true, false).swallow);
        assert!(!r.route(TAB, false, false).swallow);
    }

    #[test]
    fn down_decides_whole_press() {
        let mut r = router(true);
        r.route(W, true, false);
        r.route(RCTRL, true, false);
        let d = r.route(P, true, false);
        assert!(d.swallow && d.toggled == Some(true));
        assert!(r.route(W, false, false).report, "the up follows the blocked route");
        assert!(r.route(P, false, false).swallow, "hotkey up swallowed too");
        assert!(!r.route(W, true, false).swallow, "paused: new presses pass");
    }

    #[test]
    fn unknown_up_and_own_foreground_pass() {
        let mut r = router(true);
        assert!(!r.route(W, false, false).swallow);
        assert!(!r.route(W, true, true).swallow);
    }

    #[test]
    fn block_off_only_hotkey() {
        let mut r = router(false);
        assert!(!r.route(W, true, false).swallow);
        r.route(W, false, false);
        r.route(RCTRL, true, false);
        assert_eq!(r.route(P, true, false).toggled, Some(true));
    }

    #[test]
    fn other_hotkeys_fire_by_index_and_longest_wins() {
        const O: u16 = 18;
        const RSHIFT: u16 = 229;
        let mut r = Router { hotkeys: vec![vec![RCTRL, P], vec![RCTRL, O], vec![RCTRL, RSHIFT, P]], ..Default::default() };
        r.route(RCTRL, true, false);
        let d = r.route(O, true, false);
        assert_eq!((d.fired, d.toggled, d.swallow), (Some(1), None, true));
        r.route(O, false, false);
        r.route(RSHIFT, true, false);
        let d = r.route(P, true, false);
        assert_eq!((d.fired, d.toggled), (Some(2), None), "Ctrl+Shift+P beats Ctrl+P");
        assert!(!r.paused);
    }

    #[test]
    fn no_pause_hotkey() {
        let mut r = Router { hotkeys: vec![vec![], vec![RCTRL, P]], ..Default::default() };
        r.route(RCTRL, true, false);
        assert_eq!(r.route(P, true, false).fired, Some(1));
    }

    #[test]
    fn block_all_blocks_unbound_keys_but_not_hotkeys() {
        let mut r = Router { block: true, block_all: true, allow_alt_tab: true, hotkeys: vec![vec![RCTRL, P]], ..Default::default() };
        assert!(r.route(X, true, false).swallow, "unbound key blocked too");
        r.route(X, false, false);
        assert!(!r.route(TAB, true, false).swallow, "Alt+Tab still exempt");
        r.route(TAB, false, false);
        r.route(RCTRL, true, false);
        assert_eq!(r.route(P, true, false).toggled, Some(true), "the pause hotkey still works");
        assert!(!r.route(X, true, true).swallow, "Depthpad's own window gets keys");
    }
}
