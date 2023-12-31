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
            daily_log_channel = NULL
        WHERE
            server_id = ?;",
        guild_id
    )
    .execute(pool)
    .await;

    let content = match update {
        Ok(_) => "Disabled daily message stats, re-enable logs with /set-daily-log-channel",
        Err(_) => "Failed to disable daily message logging (db error, maybe try again?)",
    };

    CreateInteractionResponseMessage::new().content(content)
}

pub fn register() -> CreateCommand {
    CreateCommand::new("disable-daily-message-logs")
        .description("re-enable vc daily message stats with /set-daily-log-channel")
}
