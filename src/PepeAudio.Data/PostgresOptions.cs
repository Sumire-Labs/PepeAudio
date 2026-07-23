// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Data;

public sealed class PostgresOptions
{
    public const string Section = "ConnectionStrings";

    // Bound from ConnectionStrings:Postgres
    public string? Postgres { get; set; }
}
