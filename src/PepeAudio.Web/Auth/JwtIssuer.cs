// SPDX-License-Identifier: Apache-2.0
using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Text;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

namespace PepeAudio.Web.Auth;

// Issues and describes validation for the app session JWT (HttpOnly cookie).
public sealed class JwtIssuer
{
    private const string Issuer = "pepeaudio";
    private static readonly TimeSpan Lifetime = TimeSpan.FromDays(7);

    private readonly WebOptions _opt;

    public JwtIssuer(IOptions<WebOptions> opt) => _opt = opt.Value;

    public string Issue(string userId, string username, string? avatar)
    {
        var key = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(_opt.JwtSigningKey));
        var token = new JwtSecurityToken(
            issuer: Issuer, audience: Issuer,
            claims: new[]
            {
                new Claim("sub", userId),
                new Claim("name", username),
                new Claim("avatar", avatar ?? ""),
            },
            expires: DateTime.UtcNow.Add(Lifetime),
            signingCredentials: new SigningCredentials(key, SecurityAlgorithms.HmacSha256));
        return new JwtSecurityTokenHandler().WriteToken(token);
    }

    public static TokenValidationParameters ValidationParameters(WebOptions opt) => new()
    {
        ValidateIssuer = true,
        ValidIssuer = Issuer,
        ValidateAudience = true,
        ValidAudience = Issuer,
        ValidateIssuerSigningKey = true,
        IssuerSigningKey = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(opt.JwtSigningKey)),
        ValidateLifetime = true,
        ClockSkew = TimeSpan.FromMinutes(1),
    };
}
