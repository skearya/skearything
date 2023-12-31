#[path = "../commands/mod.rs"]
mod commands;

use std::env;

use serenity::all::Command;
use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::id::GuildId;
use serenity::prelude::*;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, data_about_bot: Ready) {
        println!("logged in as {}", data_about_bot.user.name);

        let guild_id = GuildId::new(
            env::var("GUILD_ID")
                .expect("Expected GUILD_ID in environment")
                .parse()
                .expect("GUILD_ID must be an integer"),
        );

        let slash_commands = vec![
            commands::set_msg_log_channel::register(),
            commands::set_vc_log_channel::register(),
            commands::disable_daily_log::register(),
            commands::disable_vc_logs::register(),
        ];

        if env::args().nth(1).unwrap_or("".to_string()) == "global" {
            println!(
                "global commands: {:#?}",
                Command::set_global_commands(&ctx.http, slash_commands).await
            );
        } else {
            println!(
                "testing commands: {:#?}",
                guild_id.set_commands(&ctx.http, slash_commands).await
            );
        }

        std::process::exit(0);
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().unwrap();

    let token = env::var("BOT_TOKEN").expect("token");

    let mut client = Client::builder(token, GatewayIntents::non_privileged())
        .event_handler(Handler)
        .await
        .expect("Error creating client");

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
