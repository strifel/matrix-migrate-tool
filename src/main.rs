use std::{env, process::exit, collections::HashMap};

use std::fs::File;
use std::io::Write;

use matrix_sdk::{
    config::SyncSettings, room, ruma::{events::room::{history_visibility::{self, HistoryVisibility}, power_levels::RoomPowerLevels}, RoomId, UserId}, Client
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

    let is_already_in = r.get_member(new_user_id).await.unwrap().is_some();

    if !is_already_in {
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


    let mut joined_rooms = client.joined_rooms();
    joined_rooms.sort_by_key(|j| j.name());
    let mut i = 0;
    for r in &joined_rooms {
        let name = r.display_name().await.unwrap();
        let encrypted = r.is_encrypted().await.unwrap_or(true);
        let history_visibility = (r.history_visibility() == HistoryVisibility::Shared || r.history_visibility() == HistoryVisibility::WorldReadable);
        let is_already_in = r.get_member(new_user_id).await.unwrap().is_some();
        println!("{}: {} ({}), encrypted: {}, history visible: {}, new account is in room: {}", i, name, r.room_id(), encrypted, history_visibility, is_already_in);

        i += 1;
    }

    println!("Selection (comma separated): ");

    let mut line: String = String::new();
    std::io::stdin().read_line(&mut line)?;
    line = line.replace("\n", "");
    line = line.replace(" ", "");
    let selection_list = line.split(",");

    for rid in selection_list {
        if rid.starts_with("!") {
            let selected: String = rid.replace("#", "");
            let room_id = RoomId::parse(selected.to_string());
            if !room_id.is_ok() || room_id.is_err() {
                println!("Found broken Room_ID");
                room_state.insert(selected.to_string(), String::from("ROOMID_INVALID"));
                continue;
            }
            let maybe_room = client.get_joined_room(&room_id.unwrap());
            if maybe_room.is_none() {
                room_state.insert(selected.to_string(), String::from("ROOM_NOT_FOUND"));
                continue;
            }
            let room = maybe_room.unwrap();
            println!("Processing room {}: {}", rid, room.display_name().await.unwrap());
            let state = process_room(&room, &client, new_user_id, rid.ends_with("#")).await;
            room_state.insert(selected.to_string(), state);
        } else {
            let selected: usize = rid.replace("#", "").parse().unwrap();
            let room = &joined_rooms[selected];
            println!("Processing room {}: {}", rid, room.display_name().await.unwrap());
            let state = process_room(&room, &client, new_user_id, rid.ends_with("#")).await;
            room_state.insert(room.room_id().as_str().to_string(), state);
        }

        let mut file = File::create("current_state.json")
        .expect("Unable to create file");

        file.write_all(json::stringify_pretty(json::object! {
            "rooms": room_state.clone()
        }, 4).as_bytes())
        .expect("Unable to write data");
    }

    println!("{}", json::stringify_pretty(json::object! {
        "rooms": room_state
    }, 4));

    Ok(())
}