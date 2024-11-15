use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use serenity::{
    all::{ChannelId, UserId},
    builder::{CreateEmbed, CreateEmbedFooter, CreateMessage, GetMessages},
    http::CacheHttp,
};

use crate::utils;

const DAY: i64 = 60 * 60 * 24;

const SKIPPED_WORDS: &[&str] = &[
    "there", "by", "at", "and", "so", "if", "than", "but", "about", "in", "on", "the", "was",
    "for", "that", "said", "a", "or", "of", "to", "there", "will", "be", "what", "get", "go",
    "think", "just", "every", "are", "it", "were", "had", "i", "",
];

#[derive(Debug)]
struct UserInfo {
    username: String,
    messages: u32,
}

pub async fn send_message_stats(
    http: impl CacheHttp,
    channel: ChannelId,
    vc_seconds_elapsed: Option<f64>,
) -> anyhow::Result<()> {
    let mut messages = channel
        .messages(&http, GetMessages::new().limit(100))
        .await?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time has gone backwards")
        .as_secs();

    while (now as i64
        - messages
            .last()
            .context("No messages")?
            .timestamp
            .unix_timestamp())
    .abs()
        <= DAY
    {
        let new_messages = channel
            .messages(
                &http,
                GetMessages::new()
                    .before(messages.last().context("No messages")?.id)
                    .limit(100),
            )
            .await?;

        if new_messages.is_empty() {
            break;
        }

        messages.extend(new_messages);
    }

    messages.retain(|message| i64::abs(now as i64 - message.timestamp.unix_timestamp()) <= DAY);

    let mut user_info: HashMap<UserId, UserInfo> = HashMap::new();
    let mut word_counts: HashMap<String, u32> = HashMap::new();

    for message in &messages {
        user_info
            .entry(message.author.id)
            .or_insert(UserInfo {
                username: message.author.name.clone(),
                messages: 0,
            })
            .messages += 1;

        message
            .content
            .split_whitespace()
            .filter(|word| !SKIPPED_WORDS.contains(word))
            .for_each(|word| {
                *word_counts.entry(word.to_lowercase()).or_insert(0) += 1;
            });
    }

    let mut word_counts: Vec<(String, u32)> = Vec::from_iter(word_counts.into_iter());
    word_counts.sort_by_key(|x| x.1);
    word_counts.reverse();

    let mut info: Vec<UserInfo> = user_info.into_values().collect();
    info.sort_by_key(|x| x.messages);

    let mut embed = CreateEmbed::new()
        .title("Active members")
        .color(0xe190de)
        .footer(CreateEmbedFooter::new(format!(
            "Total messages: {} | Unique chatters: {}",
            messages.len(),
            info.len()
        )));

    for (i, user) in info.iter().rev().take(8).enumerate() {
        embed = embed.field(
            format!("#{} {}", i + 1, user.username),
            format!("{} messages", user.messages),
            true,
        );
    }

    let words: String = word_counts
        .iter()
        .take(8)
        .map(|x| x.0.clone())
        .collect::<Vec<String>>()
        .join(", ");

    embed = embed.field("Most used words", words, false);

    embed = embed.field(
        "Total time in vc today",
        utils::format_from_seconds(vc_seconds_elapsed.unwrap_or(0.0) as f32),
        false,
    );

    channel
        .send_message(&http, CreateMessage::new().add_embed(embed))
        .await?;

    Ok(())
}
