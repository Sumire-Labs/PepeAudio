// SPDX-License-Identifier: Apache-2.0
namespace PepeAudio.Web;

public sealed class WebOptions
{
    public const string Section = "WebGui";

    public bool Enabled { get; set; }
    public string BaseUrl { get; set; } = "http://localhost:3000";
    public string SessionCookieName { get; set; } = "pepe_session";
    public string JwtSigningKey { get; set; } = "";

    // Discord user IDs allowed into the admin dashboard.
    public string[] AdminUserIds { get; set; } = Array.Empty<string>();

    public OAuthOptions OAuth { get; set; } = new();

    public sealed class OAuthOptions
    {
        public string ClientId { get; set; } = "";
        public string ClientSecret { get; set; } = "";
        public string RedirectUri { get; set; } = "http://localhost:3000/api/auth/callback";
    }
}
