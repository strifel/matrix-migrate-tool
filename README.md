# Matrix Migrate Tool
Migrate all* Chats to a new Matrix Account.

(* excluding those chats where you do not have invite access)

This is an alternative to the Element Tool for that, which does not seem to be open source and does not seem to be working with alle Homeservers all the time.

This tool does allow Migrating only single chats

## Running
You need Rust.

Than `cargo run https://old.homeserver old_username old_password @newUser:newHomeserver.tld`

## Recommendations
You should afterwards export the keys on your old account and import them into the new account
