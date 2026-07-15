use std::{env, net::SocketAddr, path::PathBuf};

use ebirforms_web::{app, Store};

#[tokio::main]
async fn main() {
    let mut args = env::args().skip(1);
    let command = args.next();
    let db =
        PathBuf::from(env::var("EBIRFORMS_WEB_DB").unwrap_or_else(|_| "data/web.sqlite3".into()));
    let store = Store::open(&db).expect("open web database");

    match command.as_deref() {
        Some("create-user") => {
            let email = args
                .next()
                .expect("usage: ebirforms-web create-user EMAIL ROLE [PASSWORD]");
            let role = args.next().expect("role must be customer or operator");
            let password = args
                .next()
                .or_else(|| env::var("EBIRFORMS_NEW_USER_PASSWORD").ok())
                .expect("password argument or EBIRFORMS_NEW_USER_PASSWORD is required");
            store
                .create_user(&email, &password, &role)
                .expect("create user");
            println!("created {role} account for {email}");
            return;
        }
        Some("list-users") => {
            for (id, email, role, disabled) in store.list_users().expect("list users") {
                println!(
                    "{id}\t{email}\t{role}\t{}",
                    if disabled { "disabled" } else { "enabled" }
                );
            }
            return;
        }
        Some("reset-password") => {
            let email = args
                .next()
                .expect("usage: ebirforms-web reset-password EMAIL [PASSWORD]");
            let password = args
                .next()
                .or_else(|| env::var("EBIRFORMS_NEW_USER_PASSWORD").ok())
                .expect("password argument or EBIRFORMS_NEW_USER_PASSWORD is required");
            store
                .reset_password(&email, &password)
                .expect("reset password");
            println!("reset password and revoked sessions for {email}");
            return;
        }
        Some("disable-user") | Some("enable-user") => {
            let email = args
                .next()
                .expect("usage: ebirforms-web disable-user|enable-user EMAIL");
            let disabled = command.as_deref() == Some("disable-user");
            store
                .set_user_disabled(&email, disabled)
                .expect("update user");
            println!(
                "{} {email} and revoked sessions",
                if disabled { "disabled" } else { "enabled" }
            );
            return;
        }
        Some(other) => panic!("unknown command: {other}"),
        None => {}
    }

    let bind: SocketAddr = env::var("EBIRFORMS_WEB_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()
        .expect("valid EBIRFORMS_WEB_BIND");
    let static_dir =
        env::var("EBIRFORMS_WEB_STATIC_DIR").unwrap_or_else(|_| "apps/web/frontend/dist".into());
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .expect("bind web server");
    println!("ebirforms web intake listening on http://{bind}");
    axum::serve(listener, app(store, static_dir))
        .await
        .expect("serve web app");
}
