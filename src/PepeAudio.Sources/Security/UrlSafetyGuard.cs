// SPDX-License-Identifier: Apache-2.0
using System.Net;
using System.Net.Sockets;

namespace PepeAudio.Sources.Security;

// Basic SSRF guard: https-only and block private/loopback/link-local literals.
// A connect-time IP re-check (DNS-rebinding) is added with the direct-URL fetch path.
public static class UrlSafetyGuard
{
    public static bool IsSafeHttpUrl(string? url, out Uri? uri)
    {
        uri = null;
        if (!Uri.TryCreate(url, UriKind.Absolute, out var parsed))
            return false;
        if (parsed.Scheme != Uri.UriSchemeHttps)
            return false;
        if (IPAddress.TryParse(parsed.Host, out var ip) && IsPrivate(ip))
            return false;
        uri = parsed;
        return true;
    }

    public static bool IsPrivate(IPAddress ip)
    {
        if (IPAddress.IsLoopback(ip)) return true;
        if (ip.AddressFamily == AddressFamily.InterNetwork)
        {
            var b = ip.GetAddressBytes();
            if (b[0] == 10) return true;
            if (b[0] == 172 && b[1] >= 16 && b[1] <= 31) return true;
            if (b[0] == 192 && b[1] == 168) return true;
            if (b[0] == 169 && b[1] == 254) return true;   // link-local incl. metadata
            if (b[0] == 100 && b[1] >= 64 && b[1] <= 127) return true; // CGNAT
        }
        else if (ip.AddressFamily == AddressFamily.InterNetworkV6)
        {
            if (ip.IsIPv6LinkLocal || ip.IsIPv6SiteLocal) return true;
            var b = ip.GetAddressBytes();
            if ((b[0] & 0xFE) == 0xFC) return true;         // fc00::/7 unique-local
        }
        return false;
    }
}
