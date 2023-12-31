#![allow(dead_code)]
use serenity::all::CommandInteraction;
use serenity::builder::{CreateCommand, CreateInteractionResponseMessage};
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

    let update = sqlx::query!(
        "UPDATE
            servers
        SET
            vc_logs_channel = NULL
        WHERE
            server_id = ?;",
        guild_id
    )
    .execute(pool)
    .await;

    let content = match update {
        Ok(_) => "Disabled vc session logging, re-enable logs with /set-vc-session-log-channel",
        Err(_) => "Failed to disable vc session logging (db error, maybe try again?)",
    };

    CreateInteractionResponseMessage::new().content(content)
}

pub fn register() -> CreateCommand {
    CreateCommand::new("disable-vc-session-logs")
        .description("re-enable vc session logs with /set-vc-session-log-channel")
}
