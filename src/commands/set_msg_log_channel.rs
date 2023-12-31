#![allow(dead_code)]
use serenity::all::{ChannelType, CommandInteraction};
use serenity::{
    all::{CommandOptionType, ResolvedOption, ResolvedValue},
    builder::{CreateCommand, CreateCommandOption, CreateInteractionResponseMessage},
};
use sqlx::SqlitePool;

pub async fn run(
    command: &CommandInteraction,
    pool: &SqlitePool,
) -> CreateInteractionResponseMessage {
    let Some(guild) = command.guild_id else {
        return CreateInteractionResponseMessage::new()
            .content("Please run this command in a guild");
    };

    let guild_id = guild.get() as i64;

    let content = if let Some(ResolvedOption {
        value: ResolvedValue::Channel(channel),
        ..
    }) = command.data.options().first()
    {
        if let ChannelType::Text = channel.kind {
            let channel_id = channel.id.get() as i64;
            let sql_result = sqlx::query!(
                "INSERT
                    OR IGNORE INTO servers(server_id)
                VALUES
                    (?);

                UPDATE
                    servers
                SET
                    daily_log_channel = ?
                WHERE
                    server_id = ?;",
                guild_id,
                channel_id,
                guild_id,
            )
            .execute(pool)
            .await;

            match sql_result {
                Ok(_) => format!("<#{}> successfully set as daily log channel", channel_id),
                Err(_) => "Failed to set given channel as daily log channel".to_string(),
            }
        } else {
            "Please provide a text channel".to_string()
        }
    } else {
        "Please provide a text channel".to_string()
    };

    CreateInteractionResponseMessage::new().content(content)
}

pub fn register() -> CreateCommand {
    CreateCommand::new("set-daily-log-channel")
        .description("set channel to be read from for daily message stats")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Channel,
                "channel",
                "channel to recieve/be sent daily logs about messages",
            )
            .required(true),
        )
}
