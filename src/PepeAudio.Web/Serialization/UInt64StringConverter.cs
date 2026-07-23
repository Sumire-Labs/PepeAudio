// SPDX-License-Identifier: Apache-2.0
using System.Text.Json;
using System.Text.Json.Serialization;

namespace PepeAudio.Web.Serialization;

// Discord snowflakes are ulong; emit them as JSON strings so JavaScript does not
// lose precision (numbers above 2^53 are unsafe).
public sealed class UInt64StringConverter : JsonConverter<ulong>
{
    public override ulong Read(ref Utf8JsonReader reader, Type type, JsonSerializerOptions options)
        => reader.TokenType == JsonTokenType.String
            ? ulong.Parse(reader.GetString()!)
            : reader.GetUInt64();

    public override void Write(Utf8JsonWriter writer, ulong value, JsonSerializerOptions options)
        => writer.WriteStringValue(value.ToString());
}
