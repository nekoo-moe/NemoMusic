//! Tests for the opt-in gate and the README egress list.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

const README: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md"));

fn info(version: u32, default_on: bool) -> OptInInfo {
    OptInInfo {
        id: "fake",
        title: "Fake feature",
        sends: "Test data.",
        hosts: &["fake.example"],
        risk: "Test risk.",
        version,
        default_on,
    }
}

fn stored(enabled: bool, confirmed_version: u32) -> BTreeMap<String, OptInChoice> {
    BTreeMap::from([(
        "fake".to_owned(),
        OptInChoice {
            enabled,
            confirmed_version,
        },
    )])
}

/// A service that counts every request its fake host receives.
struct FakeService {
    host: Arc<AtomicUsize>,
}

impl FakeService {
    fn request(&self) {
        self.host.fetch_add(1, Ordering::SeqCst);
    }
}

/// Build the fake service through the gate and, if it exists, send one request.
fn requests(choices: &BTreeMap<String, OptInChoice>, info: &OptInInfo) -> usize {
    let host = Arc::new(AtomicUsize::new(0));
    let service = build_if_open(choices, info, || FakeService {
        host: Arc::clone(&host),
    });
    if let Some(service) = service {
        service.request();
    }
    host.load(Ordering::SeqCst)
}

#[test]
fn no_request_while_the_switch_is_off_or_unconfirmed() {
    let v1 = info(1, false);
    assert_eq!(requests(&BTreeMap::new(), &v1), 0, "never touched");
    assert_eq!(requests(&stored(false, 0), &v1), 0, "off");
    assert_eq!(
        requests(&stored(true, 0), &v1),
        0,
        "on, panel not confirmed"
    );
    assert_eq!(requests(&stored(false, 1), &v1), 0, "confirmed, then off");
    assert_eq!(requests(&stored(true, 1), &v1), 1, "on and confirmed");
}

#[test]
fn raising_the_version_closes_the_gate_again() {
    let choices = stored(true, 1);
    assert!(is_open(&choices, &info(1, false)));
    assert!(!is_open(&choices, &info(2, false)));
    assert_eq!(requests(&choices, &info(2, false)), 0);
    assert!(is_open(&stored(true, 2), &info(2, false)));
}

#[test]
fn a_default_on_row_is_open_until_the_user_turns_it_off() {
    let row = info(3, true);
    assert!(is_open(&BTreeMap::new(), &row));
    assert!(!is_open(&stored(false, 3), &row));
}

/// Every host named in the first column of the README egress list.
fn egress_hosts() -> BTreeSet<String> {
    let section = README
        .split("### Network egress list")
        .nth(1)
        .expect("README has the egress list");
    let mut hosts = BTreeSet::new();
    for line in section.lines().take_while(|l| !l.starts_with("## ")) {
        let Some(cell) = line
            .strip_prefix('|')
            .and_then(|rest| rest.split('|').next())
        else {
            continue;
        };
        for host in cell.split('`').skip(1).step_by(2) {
            hosts.insert(host.trim().to_owned());
        }
    }
    hosts
}

/// The hosts of `info` that the egress list lacks. A host the user supplies ("the endpoint
/// you set") is not a host name and is shown in the panel instead.
fn missing_hosts(info: &OptInInfo, listed: &BTreeSet<String>) -> Vec<&'static str> {
    info.hosts
        .iter()
        .copied()
        .filter(|host| !host.contains(' ') && !listed.contains(*host))
        .collect()
}

#[test]
fn every_opt_in_host_is_on_the_readme_egress_list() {
    let listed = egress_hosts();
    for info in OPT_INS {
        assert_eq!(
            missing_hosts(info, &listed),
            Vec::<&str>::new(),
            "{}",
            info.id
        );
    }
}

#[test]
fn the_egress_check_reads_the_list_and_finds_a_missing_host() {
    let listed = egress_hosts();
    for host in [
        "music.youtube.com",
        "i.ytimg.com",
        "*.ggpht.com",
        "sponsor.ajay.app",
    ] {
        assert!(listed.contains(host), "{host}");
    }
    assert_eq!(missing_hosts(&info(1, false), &listed), ["fake.example"]);
    let user_host = OptInInfo {
        hosts: &["the endpoint you set"],
        ..info(1, false)
    };
    assert!(missing_hosts(&user_host, &listed).is_empty());
}
