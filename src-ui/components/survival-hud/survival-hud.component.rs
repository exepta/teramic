use bevy::prelude::*;
use tilt_ui::{HtmlEvent, html_fn};

#[html_fn("survival_respawn")]
fn survival_respawn(_event: In<HtmlEvent>, mut requests: MessageWriter<terra_network::client::RespawnRequest>) {
    requests.write(terra_network::client::RespawnRequest);
}
