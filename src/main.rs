mod commands;
mod stats;
mod utils;

use stats::send_message_stats;

use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serenity::all::{ChannelId, Guild, GuildId, Interaction, UserId, VoiceState};
use serenity::async_trait;
use serenity::builder::{
    CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage,
};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use tokio_cron_scheduler::{Job, JobScheduler};

struct VoiceChannelState;

impl TypeMapKey for VoiceChannelState {
    type Value = HashMap<ChannelId, VoiceChannelData>;
}

#[derive(Debug, Clone)]
struct VoiceChannelData {
    guild: GuildId,
    members: HashSet<UserId>,
    start: Instant,
}

struct Handler {
    is_loop_running: AtomicBool,
    db: sqlx::Pool<Sqlite>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, data_about_bot: Ready) {
        println!("logged in as {}", data_about_bot.user.name);
    }

    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        if self.is_loop_running.load(Ordering::Relaxed) {
            return;
        }

        let ctx = Arc::new(ctx);
        let db = self.db.clone();
        let sched = JobScheduler::new().await.unwrap();

        sched
            .add(
                Job::new_async("0 0 5 * * *", move |_, _| {
                    let ctx = ctx.clone();
                    let db = db.clone();

                    Box::pin(async move {
                        let timestamp = utils::get_timestamp();

                        let channel_ids = sqlx::query!(
                            "SELECT
                                daily_log_channel,
                                vc_seconds_elapsed
                            FROM
                                servers
                                LEFT JOIN days ON servers.server_id = days.server_id
                                AND days.day = ?
                            WHERE
                                daily_log_channel IS NOT NULL;",
                            timestamp
                        )
                        .fetch_all(&db)
                        .await
                        .unwrap();

                        for row in channel_ids {
                            let channel_id = row.daily_log_channel.unwrap();

                            send_message_stats(
                                &ctx,
                                ChannelId::new(channel_id.try_into().unwrap()),
                                row.vc_seconds_elapsed,
                            )
                            .await;
                        }
                    })
                })
                .unwrap(),
            )
            .await
            .unwrap();

        sched.start().await.unwrap();
        self.is_loop_running.swap(true, Ordering::Relaxed);
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: Option<bool>) {
        if is_new.unwrap_or(false) {
            let welcome_message = guild
                .channels
                .values()
                .find(|channel| channel.name() == "general")
                .unwrap_or(guild.default_channel_guaranteed().unwrap())
                .send_message(
                    &ctx.http,
                    CreateMessage::new().content(
                        "gm, use /set-daily-log-channel and or /set-vc-session-log-channel to set up logging",
                    )
                ).await;

            if let Err(why) = welcome_message {
                println!("Failed to send welcome message: {why}")
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let can_manage_guild = command.member.as_ref().map_or(false, |member| {
                member.permissions(&ctx).map_or(false, |p| p.manage_guild())
            });

            let data = match command.data.name.as_str() {
                "set-daily-log-channel" if can_manage_guild => {
                    commands::set_msg_log_channel::run(&command, &self.db).await
                }
                "set-vc-session-log-channel" if can_manage_guild => {
                    commands::set_vc_log_channel::run(&command, &self.db).await
                }
                "disable-daily-message-logs" if can_manage_guild => {
                    commands::disable_daily_log::run(&command, &self.db).await
                }
                "disable-vc-session-logs" if can_manage_guild => {
                    commands::disable_vc_logs::run(&command, &self.db).await
                }
                "set-daily-log-channel"
                | "set-vc-session-log-channel"
                | "disable-daily-message-logs"
                | "disable-vc-session-logs" => CreateInteractionResponseMessage::new().content(
                    "You need to have the [Manage Server] permission to execute this command",
                ),
                _ => CreateInteractionResponseMessage::new().content("Unimplemented?!"),
            };

            let builder = CreateInteractionResponse::Message(data);
            if let Err(why) = command.create_response(&ctx.http, builder).await {
                println!("Cannot respond to slash command: {why}");
            }
        }
    }

    // fix eventually: if theres an ongoing vc when the bot starts, the bot will give out wrong data for that session
    async fn voice_state_update(&self, ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        let mut data = ctx.data.write().await;
        let channels = data.get_mut::<VoiceChannelState>().unwrap();
        let guild = ctx.cache.guild(new.guild_id.unwrap()).unwrap().clone();

        for user_voice_state in guild.voice_states.values() {
            let channel_id = user_voice_state.channel_id.unwrap();

            channels
                .entry(channel_id)
                .or_insert(VoiceChannelData {
                    guild: guild.id,
                    members: HashSet::new(),
                    start: Instant::now(),
                })
                .members
                .insert(user_voice_state.user_id);
        }

        let dead_channels: HashMap<ChannelId, VoiceChannelData> = channels
            .iter()
            .filter(|(_, vc_data)| vc_data.guild == guild.id)
            .filter(|(&channel_id, _)| {
                !guild
                    .voice_states
                    .values()
                    .any(|voice_state| voice_state.channel_id.unwrap() == channel_id)
            })
            .map(|(channel_id, vc_data)| (*channel_id, vc_data.clone()))
            .collect();

        if dead_channels.is_empty() {
            return;
        }

        let mut time_elapsed = 0.0;
        let mut embeds: Vec<CreateEmbed> = Vec::new();

        for (channel_id, vc_data) in dead_channels {
            let embed = CreateEmbed::new()
                .title("VC session ended")
                .color(0xe190de)
                .field("Channel", format!("<#{}>", channel_id), false)
                .field(
                    "Participants",
                    vc_data
                        .members
                        .into_iter()
                        .map(|user| format!("<@{}>", user))
                        .collect::<Vec<_>>()
                        .join(", "),
                    false,
                )
                .field(
                    "Time elapsed",
                    utils::format_from_seconds(
                        Instant::now().duration_since(vc_data.start).as_secs_f32(),
                    ),
                    false,
                );

            time_elapsed += Instant::now().duration_since(vc_data.start).as_secs_f32();
            embeds.push(embed);
            channels.remove(&channel_id);
        }

        let timestamp = utils::get_timestamp();
        let guild_id = guild.id.get() as i64;

        let Some(vc_logs_channel) = sqlx::query!(
            "SELECT
                vc_logs_channel
            FROM
                servers
            WHERE
                server_id = ?;",
            guild_id
        )
        .fetch_optional(&self.db)
        .await
        .map_or(None, |record| {
            record.and_then(|record| record.vc_logs_channel)
        }) else {
            return;
        };

        let update = sqlx::query!(
            "INSERT
                OR IGNORE INTO servers(server_id)
            VALUES
                (?);

            INSERT
                OR IGNORE INTO days(day, server_id)
            VALUES
                (?, ?);

            UPDATE
                days
            SET
                vc_seconds_elapsed = vc_seconds_elapsed + ?
            WHERE
                server_id = ?
                AND day = ?;",
            guild_id,
            timestamp,
            guild_id,
            time_elapsed,
            guild_id,
            timestamp
        )
        .execute(&self.db)
        .await;

        let vc_seconds_elapsed = sqlx::query!(
            "SELECT
                vc_seconds_elapsed
            FROM
                days
            WHERE
                server_id = ?
                AND day = ?;",
            guild_id,
            timestamp
        )
        .fetch_one(&self.db)
        .await;

        let operations = update.and(vc_seconds_elapsed);

        if let Ok(record) = operations {
            let vc_seconds_elapsed = record.vc_seconds_elapsed.unwrap();

            for embed in embeds {
                if let Err(why) = ChannelId::new(vc_logs_channel.try_into().unwrap())
                    .send_message(
                        &ctx.http,
                        CreateMessage::new().add_embed(embed.field(
                            "Total time in vc today",
                            utils::format_from_seconds(vc_seconds_elapsed as f32),
                            false,
                        )),
                    )
                    .await
                {
                    println!("Failed to send vc session log: {why}");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let db_url = dotenvy::var("DATABASE_URL").unwrap_or("sqlite:main.db".to_string());

    if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        Sqlite::create_database(&db_url).await.unwrap()
    }

    let db = SqlitePool::connect(&db_url).await.unwrap();

    sqlx::query!(
        "CREATE TABLE IF NOT EXISTS servers(
            server_id INTEGER PRIMARY KEY,
            daily_log_channel INTEGER,
            vc_logs_channel INTEGER
        );

        CREATE TABLE IF NOT EXISTS days(
            day TEXT NOT NULL,
            server_id INTEGER NOT NULL,
            messages_sent INTEGER,
            unique_chatters INTEGER,
            vc_seconds_elapsed REAL DEFAULT 0,
            FOREIGN KEY (server_id) REFERENCES servers(server_id),
            PRIMARY KEY (day, server_id)
        );"
    )
    .execute(&db)
    .await
    .unwrap();

    let token = env::var("BOT_TOKEN").expect("token");
    let intents =
        GatewayIntents::privileged() | GatewayIntents::GUILD_VOICE_STATES | GatewayIntents::GUILDS;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler {
            is_loop_running: AtomicBool::new(false),
            db,
        })
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<VoiceChannelState>(HashMap::new());
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {why}");
    }
}
