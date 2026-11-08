//! Registering the text service with the real Windows and taking it away again.
//! It writes to `HKLM` and to the language settings, so it needs administrator
//! rights and changes the machine: it runs on the CI's Windows runner (and on a
//! throwaway Windows machine), and cleans up after itself.
#![cfg(windows)]

use frontend_tsf::{profiles, registration};

#[test]
fn the_service_registers_is_listed_and_unregisters() {
    // Any file will do as "the DLL": nothing loads it in this test.
    let dll = std::env::current_exe().unwrap();
    // A clean start, in case an earlier run left something behind.
    registration::unregister().unwrap();
    assert!(!registration::class_registered());

    registration::register(&dll).expect("registering needs administrator rights");
    assert!(
        registration::class_registered(),
        "the COM class is in the registry"
    );
    let wanted = profiles::registered();
    assert!(!wanted.is_empty());
    for profile in &wanted {
        assert!(
            registration::profile_registered(profile),
            "TSF has no registration of {}",
            profile.name()
        );
        // What the TSF interface itself says about it. Informational: whether
        // it lists a profile also depends on the languages the user has set up,
        // which a CI runner does not have.
        eprintln!(
            "TSF GetProfile for {}: {:?}",
            profile.name(),
            registration::profile_lookup(profile)
        );
    }
    // Profiles without an engine are not offered.
    for profile in profiles::ALL.iter().filter(|p| !wanted.contains(p)) {
        assert!(
            !registration::profile_registered(profile),
            "Windows must not list {}",
            profile.name()
        );
    }

    // In every category it claims: a keyboard, usable in Store apps.
    let categories = registration::categories().expect("TSF lists the categories");
    for category in registration::CATEGORIES {
        assert!(
            categories.contains(&category),
            "not in category {category:?}: {categories:?}"
        );
    }

    // Registering again (a repair, an upgrade over an old install) is fine.
    registration::register(&dll).expect("a second registration works");
    assert!(wanted.iter().all(registration::profile_registered));

    registration::unregister().unwrap();
    assert!(!registration::class_registered(), "the COM class is gone");
    for profile in &wanted {
        assert!(
            !registration::profile_registered(profile),
            "{} is gone from the language settings",
            profile.name()
        );
    }
    assert!(
        registration::categories().unwrap_or_default().is_empty(),
        "no category is left"
    );
    // Removing what is not there is fine too.
    registration::unregister().unwrap();
}

#[test]
fn the_exports_windows_calls_are_in_the_library() {
    // The crate builds as a DLL; the entry points regsvr32 and COM look for
    // are plain functions of the library, so a test can call them.
    let _ = (profiles::CLSID, registration::guid(profiles::CLSID));
    let guid = registration::guid("{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}");
    assert_eq!(guid.data1, 0xCFC0_9673);
    assert_eq!(guid.data2, 0xDDBA);
    assert_eq!(guid.data3, 0x4C84);
    assert_eq!(guid.data4, [0xB5, 0x7F, 0xCF, 0x0B, 0x0D, 0xDC, 0x26, 0x2C]);
}
