//! Zigbee coordinator service: join window, endpoint discovery, reporting
//! configuration, observation publishing, and presence.
//!
//! Migrated from the bring-up demo. All `Coordinator` calls originate in
//! this single task — the C component's serialization contract — and every
//! observation leaves as a typed [`GatewayEvent`] on the bus. Command
//! execution (read/write with TX-confirm verification) is the next
//! milestone; today the service only observes.
use crate::drivers::radio::zigbee::Coordinator;
use crate::gateway::bus::BusSender;
use crate::gateway::device::{GatewayEvent, Property, Value};
use crate::gateway::presence::PresenceTracker;
use embassy_time::{Duration, Instant, Timer};
use esp_idf_svc::sys;
use log::{info, warn};
use std::collections::HashMap;

/// Network bring-up grace (the coordinator restores its persistent network).
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Poll cadence, mirroring the bring-up demo's superloop.
const POLL_TICK: Duration = Duration::from_millis(20);
/// Presence rule from the bring-up docs: silent for 15 s means offline.
const PRESENCE_TIMEOUT: Duration = Duration::from_secs(15);
/// Join window opened once the network is ready.
const JOIN_WINDOW_SECONDS: u8 = 60;
/// Home Automation profile; only HA endpoints get configured.
const PROFILE_HA: u16 = 0x0104;
const CLUSTER_TEMPERATURE: u16 = 0x0402;
const CLUSTER_ONOFF: u16 = 0x0006;

#[derive(Default)]
struct Peer {
    temperature_endpoint: Option<u8>,
    onoff_endpoint: Option<u8>,
}

#[embassy_executor::task]
pub async fn coordinator_service(bus: BusSender<'static>) {
    let mut zb = match Coordinator::new() {
        Ok(zb) => zb,
        Err(err) => {
            warn!("Zigbee init: {err:#}");
            let _ = bus.try_send(GatewayEvent::RadioZigbee { ready: false });
            return;
        }
    };

    let start = Instant::now();
    let mut ready = false;
    while start.elapsed() < STARTUP_TIMEOUT {
        if let Ok(status) = zb.status() {
            if status.ready {
                ready = true;
                break;
            }
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    if !ready {
        warn!("Zigbee startup timeout after 30 s; coordinator service idle");
        let _ = bus.try_send(GatewayEvent::RadioZigbee { ready: false });
        return;
    }

    let status = match zb.status() {
        Ok(status) => status,
        Err(err) => {
            warn!("Zigbee status: {err:#}");
            Default::default()
        }
    };
    info!(
        "Zigbee ready PAN={:04X} channel={}; joining open for {JOIN_WINDOW_SECONDS} s",
        status.pan_id, status.channel
    );
    let _ = bus.try_send(GatewayEvent::RadioZigbee { ready: true });
    if let Err(err) = zb.permit_join(JOIN_WINDOW_SECONDS) {
        warn!("permit_join: {err:#}");
    }

    let mut presence = PresenceTracker::new(PRESENCE_TIMEOUT);
    let mut peers: HashMap<u16, Peer> = HashMap::new();
    let mut request_id = 0u32;

    loop {
        // Drain the C event queue; a poll error logs and falls back to the
        // tick instead of killing the service.
        loop {
            let event = match zb.poll() {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(err) => {
                    warn!("Zigbee poll: {err:#}");
                    break;
                }
            };
            handle(&mut zb, &mut peers, &mut presence, &mut request_id, event, &bus);
        }

        for address in presence.sweep(Instant::now()) {
            peers.remove(&address);
            let _ = bus.try_send(GatewayEvent::PeerPresence {
                address,
                online: false,
            });
        }

        Timer::after(POLL_TICK).await;
    }
}

/// Route one C event: liveness first, then kind-specific work.
fn handle(
    zb: &mut Coordinator,
    peers: &mut HashMap<u16, Peer>,
    presence: &mut PresenceTracker,
    request_id: &mut u32,
    event: sys::tg_zigbee_event_t,
    bus: &BusSender<'_>,
) {
    let now = Instant::now();
    let kind = u32::from(event.kind);

    // Any event from a valid short address counts as liveness, and a first
    // sighting (join, or a returning peer reporting before rejoin) starts
    // endpoint discovery — mirroring the bring-up demo's peer detection.
    if is_peer_address(event.short_address) && kind != sys::TG_ZB_LEAVE {
        if presence.seen(event.short_address, now) {
            info!(
                "ZB peer {:#06x} online (IEEE {:02X?})",
                event.short_address, event.ieee_address
            );
            peers.entry(event.short_address).or_default();
            let _ = zb.discover(event.short_address);
            let _ = bus.try_send(GatewayEvent::PeerPresence {
                address: event.short_address,
                online: true,
            });
        }
    }

    match kind {
        sys::TG_ZB_ENDPOINT => {
            if event.profile == PROFILE_HA && event.status == 0 {
                if let Some(peer) = peers.get_mut(&event.short_address) {
                    configure_endpoint(
                        zb,
                        peer,
                        event.short_address,
                        event.endpoint,
                        event.cluster,
                        request_id,
                    );
                }
            }
        }
        sys::TG_ZB_ATTRIBUTE => {
            if let Some(peer) = peers.get(&event.short_address) {
                if let Some((property, value)) = observation(peer, &event) {
                    let _ = bus.try_send(GatewayEvent::Property {
                        address: event.short_address,
                        endpoint: event.endpoint,
                        property,
                        value,
                    });
                }
            }
        }
        sys::TG_ZB_LEAVE => {
            if presence.remove(event.short_address) {
                peers.remove(&event.short_address);
                info!("ZB peer {:#06x} left", event.short_address);
                let _ = bus.try_send(GatewayEvent::PeerPresence {
                    address: event.short_address,
                    online: false,
                });
            }
        }
        sys::TG_ZB_ERROR => {
            warn!("Zigbee error status={}", event.status);
        }
        // TX confirms, default responses and report-config receipts feed
        // the command-verification milestone; the C driver keeps counting
        // dropped events either way.
        _ => {}
    }
}

/// First sight of a temperature or OnOff cluster endpoint: read the current
/// value and subscribe to reports (the bring-up demo's configuration flow).
fn configure_endpoint(
    zb: &mut Coordinator,
    peer: &mut Peer,
    address: u16,
    endpoint: u8,
    cluster: u16,
    request_id: &mut u32,
) {
    let claimed = match cluster {
        CLUSTER_TEMPERATURE => peer.temperature_endpoint.is_none(),
        CLUSTER_ONOFF => peer.onoff_endpoint.is_none(),
        _ => false,
    };
    if !claimed {
        return;
    }
    let id = *request_id;
    *request_id = request_id.wrapping_add(1);
    if let Err(err) = zb.read(address, endpoint, cluster, id) {
        warn!("read {cluster:#06x} on {address:#06x}: {err:#}");
        return;
    }
    let id = *request_id;
    *request_id = request_id.wrapping_add(1);
    if let Err(err) = zb.reporting(address, endpoint, cluster, id) {
        warn!("reporting {cluster:#06x} on {address:#06x}: {err:#}");
        return;
    }
    match cluster {
        CLUSTER_TEMPERATURE => peer.temperature_endpoint = Some(endpoint),
        CLUSTER_ONOFF => peer.onoff_endpoint = Some(endpoint),
        _ => {}
    }
    info!("ZB {address:#06x} endpoint {endpoint} cluster {cluster:#06x} configured");
}

/// Attribute value to typed observation, per the bring-up protocol units
/// (temperature attribute carries 0.01 degC; OnOff carries 0/1).
fn observation(peer: &Peer, event: &sys::tg_zigbee_event_t) -> Option<(Property, Value)> {
    if peer.temperature_endpoint == Some(event.endpoint) && event.cluster == CLUSTER_TEMPERATURE {
        Some((Property::Temperature, Value::Number(event.value as f32 / 100.0)))
    } else if peer.onoff_endpoint == Some(event.endpoint) && event.cluster == CLUSTER_ONOFF {
        Some((Property::Power, Value::Bool(event.value != 0)))
    } else {
        None
    }
}

/// Short addresses in the valid peer range (0 reserved, 0xfff8+ reserved).
fn is_peer_address(address: u16) -> bool {
    address > 0 && address < 0xfff8
}
