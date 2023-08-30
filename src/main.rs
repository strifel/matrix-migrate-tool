use std::{env, process::exit, collections::HashMap};

use matrix_sdk::{
    Client, ruma::{events::room::power_levels::RoomPowerLevels, UserId}, room, config::SyncSettings,
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

    let settings = SyncSettings::new().timeout(std::time::Duration::from_secs(300));
    client.sync_once(settings).await.unwrap();

    Ok(client)
}

async fn process_room(r: &room::Joined, client: &Client, new_user_id: &UserId, leave: bool) -> String {
    let power_levels: RoomPowerLevels = 
        r.get_state_event_static()
          .await.expect("every room has power_level")
          .unwrap().deserialize().unwrap()
          .power_levels();

    let power_level = power_levels.for_user(client.user_id().unwrap());



    if let Err (e) = r.invite_user_by_id(new_user_id).await {
        println!("Error inviting user: {}", e);
        if r.is_public() {
            if r.canonical_alias().is_some() {
                println!("Room is public: Join via https://matrix.to/#/{}", r.canonical_alias().unwrap().as_str());
                return String::from("INVITE_ERROR_PUBLIC_ROOM_ALIAS_KNOWN");
            }
            return String::from("INVITE_ERROR_PUBLIC_ROOM_ALIAS_UNKNOWN");
        }
        return String::from("INVITE_ERROR");
    }

    if power_level.is_positive() {
        if let Err (e) = r.update_power_levels(vec![(new_user_id, power_level)]).await {
            println!("Error updating power levels: {}", e);
            return String::from("POWER_LEVEL_ERROR");
        }
    }

    if leave {
        if let Err (e) = r.leave().await {
            println!("Error leaving room: {}", e);
            return String::from("LEAVE_ERROR");
        }
        return String::from("LEFT");
    } else {
        return String::from("CAN_LEAVE");
    }
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
    let mut room_state = HashMap::new();


    let joined_rooms = &client.joined_rooms();
    let mut i = 0;
    for r in joined_rooms {
        let name = r.display_name().await.unwrap();
        println!("{}: {} ({})", i, name, r.room_id());

        i += 1;
    }

    println!("Selection (comma separated): ");

    let mut line: String = String::new();
    std::io::stdin().read_line(&mut line)?;
    line = line.replace("\n", "");
    line = line.replace(" ", "");
    let selection_list = line.split(",");

    for rid in selection_list {
        let selected: usize = rid.replace("!", "").parse().unwrap();
        let room = &joined_rooms[selected];
        println!("Processing room {}: {}", rid, room.display_name().await.unwrap());
        let state = process_room(&room, &client, new_user_id, rid.ends_with("!")).await;
        room_state.insert(room.room_id(), state);
    }

    println!("{}", json::stringify_pretty(json::object! {
        "rooms": room_state
    }, 4));

    Ok(())
}