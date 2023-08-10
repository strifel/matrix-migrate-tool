use std::{env, process::exit};

use matrix_sdk::{
    config::SyncSettings, Client, ruma::{events::room::power_levels::RoomPowerLevels, UserId},
};

async fn login_and_sync(
    homeserver_url: String,
    username: &str,
    password: &str,
) -> anyhow::Result<Client> {
    let client = Client::builder().homeserver_url(homeserver_url).build().await?;

    client
        .matrix_auth()
        .login_username(username, password)
        .initial_device_display_name("migration")
        .await?;

    println!("logged in as {username}");

    client.sync_once(SyncSettings::default()).await?;

    Ok(client)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let (homeserver_url, username, password, new_user) =
        match (env::args().nth(1), env::args().nth(2), env::args().nth(3), env::args().nth(4)) {
            (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
            _ => {
                eprintln!(
                    "Usage: {} <homeserver_url> <username> <password> <newUser>",
                    env::args().next().unwrap()
                );
                exit(1)
            }
        };

    let client = login_and_sync(homeserver_url, &username, &password).await.unwrap();

    let new_user_id = <&UserId>::try_from(&*new_user).unwrap();

    println!("Synced");

    let mut i = 0;
    for r in &client.joined_rooms() {
        let name = r.display_name().await.unwrap();
        println!("{}: {} ({})", i, name, r.room_id());

        let power_levels: RoomPowerLevels = 
          r.get_state_event_static()
           .await.expect("every room has power_level")
           .unwrap().deserialize().unwrap()
           .power_levels();

        let power_level = power_levels.for_user(client.user_id().unwrap());

        r.invite_user_by_id(new_user_id).await.unwrap();
        r.update_power_levels(vec![(new_user_id, power_level)]).await.unwrap();

        r.leave().await.unwrap();

        
        i += 1;
    }

    // TODO let user choose chat

    Ok(())
}