//! End-to-end: Engine + session + client + simulated M4.

use std::time::{Duration, Instant};

use engine::{Action, AppState, Engine, EngineOptions, LinkState, NoiseMode};
use protocol::{AntiWind, EqGen};

fn wait(engine: &Engine, what: &str, cond: impl Fn(&AppState) -> bool) -> AppState {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let s = engine.snapshot();
        if cond(&s) {
            return s;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}: {s:#?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn connects_syncs_and_applies_actions() {
    let engine = Engine::start(
        EngineOptions {
            simulate: true,
            ..Default::default()
        },
        || {},
    );
    let s = wait(&engine, "connected + synced", |s| {
        s.link == LinkState::Connected
            && s.live().is_some_and(|h| {
                h.eq.known && h.paired_hosts.len() == 3 && h.auto_off_seconds.is_some()
            })
    });
    let h = s.live().unwrap();
    assert_eq!(h.model_id.as_deref(), Some("M4AEBT Black"));
    assert_eq!(h.caps.eq, EqGen::Gen2);
    assert_eq!(h.battery, Some(76));
    assert_eq!(h.noise_mode(), Some(NoiseMode::Adaptive));
    assert_eq!(h.anti_wind, Some(AntiWind::Auto));
    assert_eq!(h.serial.as_deref(), Some("SIM000000001"));
    assert_eq!(h.paired_own_index, Some(1));
    assert_eq!(h.hi_res(), Some(true));
    assert!(h.personalization_ready());

    // Custom leaves adaptive and keeps the headset's own level; the slider then moves it
    engine.dispatch(Action::SetNoiseMode(NoiseMode::Custom));
    wait(&engine, "custom at the stored level", |s| {
        s.live().is_some_and(|h| {
            h.noise_mode() == Some(NoiseMode::Custom) && h.transparency == Some(30)
        })
    });
    engine.dispatch(Action::SetTransparency(80));
    wait(&engine, "custom 80", |s| {
        s.live().is_some_and(|h| h.transparency == Some(80))
    });

    engine.dispatch(Action::SetAntiWind(AntiWind::Max));
    wait(&engine, "anti-wind", |s| {
        s.live().is_some_and(|h| h.anti_wind == Some(AntiWind::Max))
    });

    // a burst of slider drags is coalesced; the final values win
    for i in 0..=30 {
        engine.dispatch(Action::SetEqBands([i as f32 / 10.0, 0.0, -1.5, 0.0, 6.0]));
    }
    engine.dispatch(Action::SetBassBoost(true));
    wait(&engine, "eq", |s| {
        s.live().is_some_and(|h| {
            h.eq.bands == [3.0, 0.0, -1.5, 0.0, 6.0] && h.eq.bass_boost && h.eq_active()
        })
    });

    engine.dispatch(Action::SetSidetone(4));
    engine.dispatch(Action::SetPodcast(true));
    engine.dispatch(Action::SetAutoOff(1800));
    engine.dispatch(Action::SetTouchLock(true));
    wait(&engine, "settings", |s| {
        s.live().is_some_and(|h| {
            h.sidetone == Some(4)
                && h.podcast()
                && h.auto_off_seconds == Some(1800)
                && h.touch_lock == Some(true)
        })
    });

    // multipoint: connect the idle host, then drop it again
    engine.dispatch(Action::ConnectHost(2));
    wait(&engine, "host connected", |s| {
        s.live()
            .is_some_and(|h| h.paired_hosts.get(2).is_some_and(|x| x.connected))
    });
    engine.dispatch(Action::DisconnectHost(2));
    engine.dispatch(Action::SetHiRes(false));
    engine.dispatch(Action::SetPersonalizedSound(true));
    wait(
        &engine,
        "host disconnected, hi-res off, personalization on",
        |s| {
            s.live().is_some_and(|h| {
                h.paired_hosts.get(2).is_some_and(|x| !x.connected)
                    && h.hi_res() == Some(false)
                    && h.personalized_sound == Some(protocol::PersonalizedSoundMode::Enabled)
            })
        },
    );

    // forgetting the idle tablet shrinks the list; this PC keeps index 1
    engine.dispatch(Action::ForgetHost(2));
    wait(&engine, "tablet forgotten", |s| {
        s.live().is_some_and(|h| {
            h.paired_hosts.len() == 2 && h.paired_hosts.iter().all(|x| x.name != "iPad")
        })
    });

    // ANC master switch: Off, then Adaptive turns ANC back on
    engine.dispatch(Action::SetNoiseMode(NoiseMode::Off));
    wait(&engine, "anc off", |s| {
        s.live()
            .is_some_and(|h| h.noise_mode() == Some(NoiseMode::Off))
    });
    engine.dispatch(Action::SetNoiseMode(NoiseMode::Adaptive));
    wait(&engine, "adaptive", |s| {
        s.live().is_some_and(|h| {
            h.anc_status == Some(true) && h.noise_mode() == Some(NoiseMode::Adaptive)
        })
    });

    // reconnect drops and re-establishes the session
    let v = engine.version();
    engine.dispatch(Action::Reconnect);
    let end = Instant::now() + Duration::from_secs(5);
    while engine.version() <= v {
        assert!(Instant::now() < end, "reconnect did not change state");
        std::thread::sleep(Duration::from_millis(5));
    }
    wait(&engine, "reconnected", |s| {
        s.link == LinkState::Connected && s.live().is_some_and(|h| h.eq.known)
    });
    engine.shutdown();
}
