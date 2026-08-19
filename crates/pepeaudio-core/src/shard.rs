use std::num::NonZeroU32;

use crate::GuildId;

/// Returns the Discord Gateway shard responsible for `guild_id`.
///
/// This is Discord's specified `(guild_id >> 22) % shard_count` mapping. The
/// non-zero shard count makes division-by-zero unrepresentable.
#[must_use]
pub fn shard_id(guild_id: GuildId, shard_count: NonZeroU32) -> u32 {
    let shifted = guild_id.get() >> 22;
    let shard_count = u64::from(shard_count.get());
    let remainder = shifted % shard_count;

    // `remainder < shard_count <= u32::MAX`, so this conversion cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    {
        remainder as u32
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::shard_id;
    use crate::GuildId;

    #[test]
    fn calculates_discord_shard_from_guild_snowflake() {
        // The lower 22 bits are timestamp-independent payload and must not
        // influence sharding. 17 * 16 + 5 makes the expected remainder five.
        let guild_value = ((17_u64 * 16) + 5) << 22 | 0x3f_ffff;
        let guild_id = GuildId::new(guild_value).expect("valid guild");
        let shard_count = NonZeroU32::new(16).expect("non-zero shard count");

        assert_eq!(shard_id(guild_id, shard_count), 5);
    }

    #[test]
    fn one_shard_always_maps_to_zero() {
        let guild_id = GuildId::new(u64::MAX).expect("valid guild");
        let shard_count = NonZeroU32::new(1).expect("non-zero shard count");

        assert_eq!(shard_id(guild_id, shard_count), 0);
    }
}
